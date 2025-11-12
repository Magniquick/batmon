use crate::label_for_path;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryState {
    Charging,
    Discharging,
    Other(u32),
}

impl BatteryState {
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => BatteryState::Charging,
            2 => BatteryState::Discharging,
            other => BatteryState::Other(other),
        }
    }

    fn matches_mode(self, mode: ActionMode) -> bool {
        matches!(
            (self, mode),
            (BatteryState::Charging, ActionMode::Charging)
                | (BatteryState::Discharging, ActionMode::Discharging)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionMode {
    Charging,
    Discharging,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionTrigger {
    StateEnter,
    Threshold(u8),
    Always,
}

#[derive(Debug)]
pub struct ActionSpec {
    pub mode: ActionMode,
    pub trigger: ActionTrigger,
    pub command: String,
}

pub struct StatusEvent {
    pub device_path: String,
    pub percentage: u8,
    pub previous_percentage: Option<u8>,
    pub state: BatteryState,
    pub previous_state: Option<BatteryState>,
}

pub struct ActionManager {
    actions: Vec<Action>,
    runtime: Mutex<HashMap<usize, ActionRuntime>>,
}

impl ActionManager {
    pub fn from_specs(specs: Vec<ActionSpec>, resolver: ScriptResolver) -> Self {
        let actions: Vec<Action> = specs
            .into_iter()
            .enumerate()
            .map(|(id, spec)| Action {
                id,
                mode: spec.mode,
                trigger: spec.trigger,
                script: resolver.resolve(&spec.command),
            })
            .collect();
        let runtime = Mutex::new(
            actions
                .iter()
                .map(|action| (action.id, ActionRuntime::default()))
                .collect(),
        );
        Self { actions, runtime }
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn handle_event(&self, event: StatusEvent) {
        if self.actions.is_empty() {
            return;
        }

        let mut runtime = self.runtime.lock().expect("poisoned mutex");
        for action in &self.actions {
            let entry = runtime
                .get_mut(&action.id)
                .expect("missing runtime entry for action");

            if !event.state.matches_mode(action.mode) {
                entry.triggered_in_state = false;
                continue;
            }

            if action.should_fire(&event, entry.triggered_in_state) {
                self.spawn_script(action, &event);
                if action.trigger.sticky_within_state() {
                    entry.triggered_in_state = true;
                }
            } else if matches!(action.trigger, ActionTrigger::StateEnter) {
                entry.triggered_in_state = false;
            }
        }
    }

    fn spawn_script(&self, action: &Action, event: &StatusEvent) {
        let command = action.script.clone();
        let mode = action.mode;
        let percentage = event.percentage;
        let path = event.device_path.clone();
        thread::spawn(move || {
            println!(
                "BatWatch: launching script {:?} for {} (state {:?}, {}%)",
                command,
                label_for_path(&path),
                mode,
                percentage
            );
            if let Err(err) = validate_script(&command) {
                eprintln!(
                    "BatWatch: script {:?} invalid for {} (state {:?}, {}%): {err}",
                    command,
                    label_for_path(&path),
                    mode,
                    percentage
                );
                std::process::exit(1);
            }
            let mut cmd = Command::new(&command);
            cmd.arg(format!("{}", percentage));
            cmd.arg(match mode {
                ActionMode::Charging => "charging",
                ActionMode::Discharging => "discharging",
            });
            match cmd.status() {
                Ok(status) if status.success() => {}
                Ok(status) => {
                    eprintln!(
                        "BatWatch: script {:?} exited with {} for {} (state {:?}, {}%)",
                        command,
                        status,
                        label_for_path(&path),
                        mode,
                        percentage
                    );
                    std::process::exit(1);
                }
                Err(err) => {
                    eprintln!(
                        "BatWatch: failed to run script {:?} for {} (state {:?}, {}%): {}",
                        command,
                        label_for_path(&path),
                        mode,
                        percentage,
                        err
                    );
                    std::process::exit(1);
                }
            }
        });
    }
}

impl ActionTrigger {
    fn sticky_within_state(self) -> bool {
        matches!(
            self,
            ActionTrigger::StateEnter | ActionTrigger::Threshold(_)
        )
    }
}

struct Action {
    id: usize,
    mode: ActionMode,
    trigger: ActionTrigger,
    script: PathBuf,
}

#[derive(Default)]
struct ActionRuntime {
    triggered_in_state: bool,
}

impl Action {
    fn should_fire(&self, event: &StatusEvent, already_triggered: bool) -> bool {
        match self.trigger {
            ActionTrigger::StateEnter => state_just_entered(event, self.mode),
            ActionTrigger::Threshold(level) => {
                if already_triggered {
                    return false;
                }
                match self.mode {
                    ActionMode::Charging => {
                        crossed_up(event.previous_percentage, event.percentage, level)
                    }
                    ActionMode::Discharging => {
                        crossed_down(event.previous_percentage, event.percentage, level)
                    }
                }
            }
            ActionTrigger::Always => true,
        }
    }
}

pub(crate) fn state_just_entered(event: &StatusEvent, mode: ActionMode) -> bool {
    match event.previous_state {
        Some(prev) => !prev.matches_mode(mode),
        None => true,
    }
}

pub(crate) fn crossed_up(previous: Option<u8>, current: u8, level: u8) -> bool {
    previous.map_or(current >= level, |prev| prev < level && current >= level)
}

pub(crate) fn crossed_down(previous: Option<u8>, current: u8, level: u8) -> bool {
    previous.map_or(current <= level, |prev| prev > level && current <= level)
}

pub(crate) fn validate_script(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            println!("BatWatch: script {:?} not found while validating", path);
            format!("script {} not found", path.display())
        } else {
            format!("metadata unavailable: {err}")
        }
    })?;
    if !metadata.is_file() {
        return Err("not a regular file".into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            println!("BatWatch: script {:?} missing execute bit", path);
            return Err("missing executable bit (chmod +x)".into());
        }
    }

    #[cfg(not(unix))]
    {
        if metadata.permissions().readonly() {
            return Err("script is read-only; mark it executable".into());
        }
    }

    let mut file = fs::File::open(path).map_err(|err| format!("failed to open: {err}"))?;
    let mut header = [0u8; 4];
    let read = file
        .read(&mut header)
        .map_err(|err| format!("failed to read header: {err}"))?;
    if read == 0 {
        return Err("file is empty".into());
    }

    if has_shebang(&header[..read]) || is_elf(&header[..read]) {
        Ok(())
    } else {
        Err("missing shebang and not an ELF binary".into())
    }
}

fn has_shebang(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == b'#' && bytes[1] == b'!'
}

fn is_elf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0x7F && bytes[1] == b'E' && bytes[2] == b'L' && bytes[3] == b'F'
}

#[derive(Clone)]
pub struct ScriptResolver {
    config_dir: Option<PathBuf>,
    config_scripts_dir: Option<PathBuf>,
}

impl ScriptResolver {
    pub fn new(config_dir: Option<PathBuf>) -> Self {
        let config_scripts_dir = config_dir.as_ref().map(|dir| dir.join("scripts"));
        Self {
            config_dir,
            config_scripts_dir,
        }
    }

    pub fn resolve(&self, command: &str) -> PathBuf {
        let candidate = PathBuf::from(command);
        if candidate.is_absolute() {
            println!("BatWatch: using explicit script path {:?}", candidate);
            return candidate;
        }

        if let Some(dir) = &self.config_dir {
            let config_relative = dir.join(&candidate);
            if config_relative.exists() {
                println!(
                    "BatWatch: resolved script {:?} relative to config dir {:?}",
                    command, dir
                );
                return config_relative;
            }
        }

        if let Some(dir) = &self.config_scripts_dir {
            let scripts_path = dir.join(command);
            if scripts_path.exists() {
                println!("BatWatch: resolved script {:?} under {:?}", command, dir);
                return scripts_path;
            }
        }

        if let Some(path) = find_in_path(command) {
            println!(
                "BatWatch: resolved script {:?} via $PATH at {:?}",
                command, path
            );
            return path;
        }

        println!(
            "BatWatch: falling back to literal script {:?} (may fail if missing)",
            candidate
        );
        candidate
    }
}

fn find_in_path(command: &str) -> Option<PathBuf> {
    let path_env = env::var_os("PATH")?;
    env::split_paths(&path_env)
        .map(|dir| dir.join(command))
        .find(|path| {
            let exists = path.exists();
            if exists {
                println!("BatWatch: found {:?} in PATH", path);
            }
            exists
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn always_trigger_fires_for_every_update() {
        let action = Action {
            id: 0,
            mode: ActionMode::Discharging,
            trigger: ActionTrigger::Always,
            script: PathBuf::from("noop"),
        };
        let mut event = StatusEvent {
            device_path: "/dev".into(),
            percentage: 50,
            previous_percentage: Some(51),
            state: BatteryState::Discharging,
            previous_state: Some(BatteryState::Discharging),
        };
        assert!(action.should_fire(&event, false));
        event.percentage = 49;
        assert!(action.should_fire(&event, true));
    }

    #[test]
    fn resolves_relative_paths_against_config_dir() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join("batwatch");
        fs::create_dir_all(&config_dir).unwrap();
        let script_path = config_dir.join("scripts").join("ppd-performance.sh");
        write_dummy_script(&script_path);
        let resolver = ScriptResolver::new(Some(config_dir));

        let resolved = resolver.resolve("scripts/ppd-performance.sh");
        assert_eq!(resolved, script_path);
    }

    #[test]
    fn resolves_bare_names_under_scripts_dir() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join("batwatch");
        fs::create_dir_all(&config_dir).unwrap();
        let script_path = config_dir.join("scripts").join("warn-low.sh");
        write_dummy_script(&script_path);
        let resolver = ScriptResolver::new(Some(config_dir));

        let resolved = resolver.resolve("warn-low.sh");
        assert_eq!(resolved, script_path);
    }

    fn write_dummy_script(path: &Path) {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(path, b"#!/bin/sh\nexit 0\n").unwrap();
    }
}
