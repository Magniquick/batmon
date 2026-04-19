use crate::actions::{ActionMode, ActionSpec, ActionTrigger};
use dirs::config_dir;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_PROXY_TIMEOUT_SECS: u64 = 5;
pub const MIN_POLL_INTERVAL_SECS: u64 = 1;
pub const MIN_PROXY_TIMEOUT_SECS: u64 = 1;
pub const DEFAULT_CONFIG_FILENAME: &str = "batwatch.toml";
const CONFIG_ENV_VAR: &str = "BATWATCH_CONFIG";

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    advanced: AdvancedConfig,
    #[serde(flatten, default)]
    sections: ActionSections,
}

#[derive(Debug, Deserialize, Default)]
struct ActionSections {
    #[serde(default)]
    charging: HookGroup,
    #[serde(default)]
    discharging: HookGroup,
}

#[derive(Debug, Deserialize, Default)]
struct HookGroup {
    #[serde(default)]
    script: Option<String>,
    #[serde(flatten)]
    named: BTreeMap<String, HookEntry>,
    #[serde(default)]
    when: Option<ActionWhen>,
}

#[derive(Debug, Deserialize, Clone)]
struct HookEntry {
    script: String,
    #[serde(default)]
    when: Option<ActionWhen>,
}

#[derive(Debug, Deserialize)]
struct AdvancedConfig {
    #[serde(default = "default_poll_interval_secs")]
    poll_interval_secs: u64,
    #[serde(default = "default_proxy_timeout_secs")]
    proxy_timeout_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum ActionWhen {
    Number(u8),
    Text(String),
}

impl Config {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs())
    }

    pub fn proxy_timeout(&self) -> Duration {
        Duration::from_secs(self.proxy_timeout_secs())
    }

    pub fn poll_interval_secs(&self) -> u64 {
        self.advanced.poll_interval_secs.max(MIN_POLL_INTERVAL_SECS)
    }

    pub fn proxy_timeout_secs(&self) -> u64 {
        self.advanced.proxy_timeout_secs.max(MIN_PROXY_TIMEOUT_SECS)
    }

    pub fn action_specs(&self) -> Vec<ActionSpec> {
        self.action_specs_checked().unwrap_or_else(|err| {
            eprintln!("BatWatch: {err}");
            std::process::exit(1);
        })
    }

    pub fn action_specs_checked(&self) -> Result<Vec<ActionSpec>, String> {
        let mut specs = Vec::new();
        specs.extend(self.sections.charging.to_specs(ActionMode::Charging)?);
        specs.extend(
            self.sections
                .discharging
                .to_specs(ActionMode::Discharging)?,
        );
        Ok(specs)
    }
}

#[cfg(test)]
impl Config {
    pub(crate) fn with_intervals_for_test(poll_secs: u64, proxy_secs: u64) -> Self {
        Self {
            advanced: AdvancedConfig {
                poll_interval_secs: poll_secs,
                proxy_timeout_secs: proxy_secs,
            },
            ..Default::default()
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll_interval_secs(),
            proxy_timeout_secs: default_proxy_timeout_secs(),
        }
    }
}

impl HookGroup {
    fn to_specs(&self, mode: ActionMode) -> Result<Vec<ActionSpec>, String> {
        let mut specs = Vec::new();
        if let Some(cmd) = self.script.as_ref().and_then(|s| trimmed(s)) {
            specs.push(build_spec(cmd, self.when.as_ref(), mode)?);
        }
        for (name, entry) in self.named.iter() {
            if let Some(cmd) = trimmed(&entry.script) {
                specs.push(build_spec(
                    cmd,
                    entry.when.as_ref().or(self.when.as_ref()),
                    mode,
                )?);
            } else {
                return Err(format!("Hook `{name}` is missing a script"));
            }
        }
        Ok(specs)
    }
}

fn build_spec(
    command: &str,
    when: Option<&ActionWhen>,
    mode: ActionMode,
) -> Result<ActionSpec, String> {
    let trigger = when
        .map(|w| trigger_from_when(w, mode))
        .transpose()?
        .unwrap_or(ActionTrigger::StateEnter);
    Ok(ActionSpec {
        mode,
        trigger,
        command: command.to_string(),
    })
}

fn trigger_from_when(when: &ActionWhen, _mode: ActionMode) -> Result<ActionTrigger, String> {
    match when {
        ActionWhen::Text(value) if value.eq_ignore_ascii_case("always") => {
            Ok(ActionTrigger::Always)
        }
        ActionWhen::Text(value) if value.eq_ignore_ascii_case("once") => {
            Ok(ActionTrigger::StateEnter)
        }
        ActionWhen::Number(level) => Ok(ActionTrigger::Threshold(*level)),
        ActionWhen::Text(value) => Err(format!("unsupported `when = \"{value}\"`")),
    }
}

fn trimmed(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

const fn default_poll_interval_secs() -> u64 {
    DEFAULT_POLL_INTERVAL_SECS
}

const fn default_proxy_timeout_secs() -> u64 {
    DEFAULT_PROXY_TIMEOUT_SECS
}

pub struct LoadedConfig {
    pub config: Config,
    pub origin_dir: Option<PathBuf>,
}

pub fn load_config() -> LoadedConfig {
    for path in config_search_paths() {
        if let Some(config) = load_config_from_path(&path) {
            let origin_dir = path.parent().map(PathBuf::from);
            return LoadedConfig { config, origin_dir };
        }
    }
    LoadedConfig {
        config: Config::default(),
        origin_dir: None,
    }
}

pub(crate) fn load_config_from_path(path: &Path) -> Option<Config> {
    match fs::read_to_string(path) {
        Ok(raw) => match toml::from_str(&raw) {
            Ok(config) => Some(config),
            Err(err) => {
                eprintln!("BatWatch: failed to parse {}: {err}", path.display());
                None
            }
        },
        Err(err) => {
            if err.kind() != io::ErrorKind::NotFound {
                eprintln!("BatWatch: failed to read {}: {err}", path.display());
            }
            None
        }
    }
}

fn config_search_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(value) = env::var_os(CONFIG_ENV_VAR) {
        candidates.push(PathBuf::from(value));
    }

    if let Some(nested) = default_config_path() {
        candidates.push(nested);
    }

    if let Some(mut dir) = config_dir() {
        dir.push(DEFAULT_CONFIG_FILENAME);
        candidates.push(dir);
    }

    if let Ok(mut exe) = env::current_exe() {
        exe.pop();
        exe.push(DEFAULT_CONFIG_FILENAME);
        candidates.push(exe);
    }

    if let Ok(mut current) = env::current_dir() {
        current.push(DEFAULT_CONFIG_FILENAME);
        candidates.push(current);
    }

    candidates
}

pub fn default_config_dir() -> Option<PathBuf> {
    let mut dir = config_dir()?;
    dir.push("batwatch");
    Some(dir)
}

pub fn default_config_path() -> Option<PathBuf> {
    let mut path = default_config_dir()?;
    path.push(DEFAULT_CONFIG_FILENAME);
    Some(path)
}
