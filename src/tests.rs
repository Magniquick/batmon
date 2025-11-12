use crate::actions::{ActionMode, ActionTrigger, crossed_down, crossed_up, validate_script};
use crate::config::{
    self, Config, DEFAULT_POLL_INTERVAL_SECS, DEFAULT_PROXY_TIMEOUT_SECS, MIN_POLL_INTERVAL_SECS,
    MIN_PROXY_TIMEOUT_SECS,
};
use crate::describe_state;
use std::path::Path;

#[test]
fn describe_state_reports_known_values() {
    assert_eq!("charging", describe_state(1));
    assert_eq!("discharging", describe_state(2));
    assert_eq!("unknown", describe_state(999));
}

#[test]
fn config_clamps_intervals() {
    let config = Config::default();
    assert_eq!(DEFAULT_POLL_INTERVAL_SECS, config.poll_interval_secs());
    assert_eq!(DEFAULT_PROXY_TIMEOUT_SECS, config.proxy_timeout_secs());

    let config = Config::with_intervals_for_test(0, 0);
    assert_eq!(MIN_POLL_INTERVAL_SECS, config.poll_interval_secs());
    assert_eq!(MIN_PROXY_TIMEOUT_SECS, config.proxy_timeout_secs());

    let config = Config::with_intervals_for_test(10, 15);
    assert_eq!(10, config.poll_interval_secs());
    assert_eq!(15, config.proxy_timeout_secs());
}

#[test]
fn missing_config_defaults_are_used() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    path.push(format!("batwatch-nonexistent-{unique}.toml"));
    let _ = std::fs::remove_file(&path);

    let config = config::load_config_from_path(Path::new(
        path.to_str()
            .expect("temp path should be valid UTF-8 for tests"),
    ))
    .unwrap_or_default();
    assert_eq!(5, config.poll_interval_secs());
    assert_eq!(5, config.proxy_timeout_secs());
}

#[test]
fn action_config_parses_triggers() {
    let cfg: Config = toml::from_str(
        r#"
[charging.toast]
script = "echo toast"
when = "always"

[charging.log]
script = "echo log"

[charging.once]
script = "echo once"
when = "once"

[discharging.warn]
script = "echo warn"
when = 42
"#,
    )
    .expect("valid config");

    let mut charging = Vec::new();
    let mut discharging = Vec::new();
    for spec in cfg.action_specs_checked().expect("valid hooks") {
        match spec.mode {
            ActionMode::Charging => charging.push(spec.trigger),
            ActionMode::Discharging => discharging.push(spec.trigger),
        }
    }

    charging.sort_by_key(|t| match t {
        ActionTrigger::Always => 0,
        ActionTrigger::StateEnter => 1,
        ActionTrigger::Threshold(_) => 2,
    });

    assert_eq!(
        vec![
            ActionTrigger::Always,
            ActionTrigger::StateEnter,
            ActionTrigger::StateEnter
        ],
        charging
    );
    assert_eq!(vec![ActionTrigger::Threshold(42)], discharging);
}

#[test]
fn invalid_when_reports_error() {
    let cfg: Result<Config, _> = toml::from_str(
        r#"
[charging.bad]
script = "echo hi"
when = "later"
"#,
    );
    let cfg = cfg.expect("valid toml");
    let err = cfg.action_specs_checked().expect_err("should error");
    assert!(err.contains("unsupported `when"));
}

#[test]
fn threshold_crossing_detects_skipped_percent() {
    assert!(crossed_down(Some(60), 40, 50));
    assert!(crossed_up(Some(40), 60, 50));
    assert!(!crossed_down(Some(40), 60, 50));
    assert!(!crossed_up(Some(60), 40, 50));
}

#[test]
fn validate_script_accepts_shebang_or_elf() {
    use std::io::Write;
    let tmp_dir = tempfile::tempdir().expect("tmp dir");

    let script_path = tmp_dir.path().join("script.sh");
    {
        let mut file = std::fs::File::create(&script_path).expect("create script");
        writeln!(file, "#!/bin/sh\necho ok").expect("write script");
    }
    mark_executable(&script_path);
    assert!(validate_script(&script_path).is_ok());

    let elf_path = tmp_dir.path().join("binary");
    {
        let mut file = std::fs::File::create(&elf_path).expect("create elf");
        file.write_all(b"\x7FELF\0\0\0\0").expect("write elf");
    }
    mark_executable(&elf_path);
    assert!(validate_script(&elf_path).is_ok());

    let txt_path = tmp_dir.path().join("plain.txt");
    std::fs::write(&txt_path, "echo missing shebang").expect("write txt");
    mark_executable(&txt_path);
    assert!(validate_script(&txt_path).is_err());
}

#[test]
fn validate_script_errors_on_missing_path() {
    let missing = tempfile::tempdir()
        .expect("tmp dir")
        .path()
        .join("missing.sh");
    let err = validate_script(&missing).expect_err("should error");
    assert!(
        err.contains("not found"),
        "expected not found error, got {err}"
    );
}

fn mark_executable(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}
