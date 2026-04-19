use dirs::data_local_dir;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{DEFAULT_CONFIG_FILENAME, default_config_dir};

const DEFAULT_CONFIG: &str = include_str!("../batwatch.toml");

const SCRIPTS: &[BundledTextFile] = &[
    BundledTextFile::new("common.sh", include_str!("../scripts/common.sh")),
    BundledTextFile::new(
        "critical-low.sh",
        include_str!("../scripts/critical-low.sh"),
    ),
    BundledTextFile::new("full.sh", include_str!("../scripts/full.sh")),
    BundledTextFile::new(
        "hybrid-sleep-if-needed.sh",
        include_str!("../scripts/hybrid-sleep-if-needed.sh"),
    ),
    BundledTextFile::new(
        "install-icons.sh",
        include_str!("../scripts/install-icons.sh"),
    ),
    BundledTextFile::new("on-charging.sh", include_str!("../scripts/on-charging.sh")),
    BundledTextFile::new(
        "on-discharge.sh",
        include_str!("../scripts/on-discharge.sh"),
    ),
    BundledTextFile::new(
        "on-plugged-in.sh",
        include_str!("../scripts/on-plugged-in.sh"),
    ),
    BundledTextFile::new(
        "ppd-balanced.sh",
        include_str!("../scripts/ppd-balanced.sh"),
    ),
    BundledTextFile::new(
        "ppd-performance.sh",
        include_str!("../scripts/ppd-performance.sh"),
    ),
    BundledTextFile::new("warn-low.sh", include_str!("../scripts/warn-low.sh")),
];

const ASSETS: &[BundledBytesFile] = &[
    BundledBytesFile::new("bat.svg", include_bytes!("../assets/bat.svg")),
    BundledBytesFile::new("bat_white.svg", include_bytes!("../assets/bat_white.svg")),
];

const ICONS: &[BundledBytesFile] = &[
    BundledBytesFile::new("batwatch.svg", include_bytes!("../assets/bat.svg")),
    BundledBytesFile::new(
        "batwatch-symbolic.svg",
        include_bytes!("../assets/bat_white.svg"),
    ),
];

const SYSTEMD_UNIT: BundledTextFile = BundledTextFile::new(
    "batwatch.service",
    include_str!("../systemd/batwatch.service"),
);

pub fn default_config() -> &'static str {
    DEFAULT_CONFIG
}

pub fn install(force: bool) -> io::Result<InstallReport> {
    let config_dir = default_config_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not resolve a user config directory",
        )
    })?;

    fs::create_dir_all(&config_dir)?;
    let mut report = InstallReport {
        config_dir: config_dir.clone(),
        written: Vec::new(),
        skipped: Vec::new(),
    };

    write_text(
        &config_dir.join(DEFAULT_CONFIG_FILENAME),
        DEFAULT_CONFIG,
        force,
        false,
        &mut report,
    )?;

    let scripts_dir = config_dir.join("scripts");
    fs::create_dir_all(&scripts_dir)?;
    for script in SCRIPTS {
        write_text(
            &scripts_dir.join(script.name),
            script.contents,
            force,
            true,
            &mut report,
        )?;
    }

    let assets_dir = config_dir.join("assets");
    fs::create_dir_all(&assets_dir)?;
    for asset in ASSETS {
        write_bytes(
            &assets_dir.join(asset.name),
            asset.contents,
            force,
            &mut report,
        )?;
    }

    if let Some(icon_dir) = icon_dir() {
        fs::create_dir_all(&icon_dir)?;
        for icon in ICONS {
            write_bytes(&icon_dir.join(icon.name), icon.contents, force, &mut report)?;
        }
    }

    if let Some(systemd_user_dir) = systemd_user_dir() {
        fs::create_dir_all(&systemd_user_dir)?;
        write_text(
            &systemd_user_dir.join(SYSTEMD_UNIT.name),
            SYSTEMD_UNIT.contents,
            force,
            false,
            &mut report,
        )?;
    }

    Ok(report)
}

fn icon_dir() -> Option<PathBuf> {
    let mut dir = data_local_dir()?;
    dir.push("icons");
    dir.push("hicolor");
    dir.push("scalable");
    dir.push("apps");
    Some(dir)
}

fn systemd_user_dir() -> Option<PathBuf> {
    let mut dir = default_config_dir()?;
    dir.pop();
    dir.push("systemd");
    dir.push("user");
    Some(dir)
}

fn write_text(
    path: &Path,
    contents: &str,
    force: bool,
    executable: bool,
    report: &mut InstallReport,
) -> io::Result<()> {
    write_file(path, contents.as_bytes(), force, report)?;
    if executable {
        make_executable(path)?;
    }
    Ok(())
}

fn write_bytes(
    path: &Path,
    contents: &[u8],
    force: bool,
    report: &mut InstallReport,
) -> io::Result<()> {
    write_file(path, contents, force, report)
}

fn write_file(
    path: &Path,
    contents: &[u8],
    force: bool,
    report: &mut InstallReport,
) -> io::Result<()> {
    if path.exists() && !force {
        report.skipped.push(path.to_path_buf());
        return Ok(());
    }

    fs::write(path, contents)?;
    report.written.push(path.to_path_buf());
    Ok(())
}

fn make_executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

pub struct InstallReport {
    pub config_dir: PathBuf,
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

struct BundledTextFile {
    name: &'static str,
    contents: &'static str,
}

impl BundledTextFile {
    const fn new(name: &'static str, contents: &'static str) -> Self {
        Self { name, contents }
    }
}

struct BundledBytesFile {
    name: &'static str,
    contents: &'static [u8],
}

impl BundledBytesFile {
    const fn new(name: &'static str, contents: &'static [u8]) -> Self {
        Self { name, contents }
    }
}
