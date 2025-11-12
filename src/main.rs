mod actions;
mod config;

use actions::{ActionManager, BatteryState, ScriptResolver, StatusEvent};
use dbus::Path as DBusPath;
use dbus::arg::PropMap;
use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::message::{MatchRule, MessageType};
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const UPOWER_SERVICE: &str = "org.freedesktop.UPower";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";
const DEVICE_INTERFACE: &str = "org.freedesktop.UPower.Device";
const UPOWER_INTERFACE: &str = "org.freedesktop.UPower";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const BATTERY_DEVICE_TYPE: u32 = 2;

fn main() {
    let debug = env::args().any(|arg| arg == "--debug");
    if let Err(err) = run(debug) {
        eprintln!("BatWatch: {err}");
        std::process::exit(1);
    }
}

fn run(debug: bool) -> Result<(), Box<dyn Error>> {
    let config::LoadedConfig { config, origin_dir } = config::load_config();
    if debug {
        println!(
            "Debug configuration: poll_interval={}s, proxy_timeout={}s",
            config.poll_interval_secs(),
            config.proxy_timeout_secs()
        );
    }
    let poll_interval = config.poll_interval();
    let proxy_timeout = config.proxy_timeout();
    let connection = Connection::new_system()?;
    let known_devices = Arc::new(Mutex::new(HashSet::<String>::new()));
    let device_states = Arc::new(Mutex::new(HashMap::<String, DeviceStatus>::new()));
    let resolver = ScriptResolver::new(origin_dir);
    let actions = Arc::new(ActionManager::from_specs(config.action_specs(), resolver));

    discover_batteries(
        &connection,
        Arc::clone(&known_devices),
        Arc::clone(&device_states),
        Arc::clone(&actions),
        proxy_timeout,
    )?;
    listen_for_device_updates(
        &connection,
        Arc::clone(&known_devices),
        Arc::clone(&device_states),
        Arc::clone(&actions),
        proxy_timeout,
    )?;
    listen_for_root_updates(&connection, proxy_timeout)?;

    loop {
        connection.process(poll_interval)?;
    }
}

fn discover_batteries(
    connection: &Connection,
    known_devices: Arc<Mutex<HashSet<String>>>,
    device_states: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    actions: Arc<ActionManager>,
    proxy_timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    let proxy = connection.with_proxy(UPOWER_SERVICE, UPOWER_PATH, proxy_timeout);
    let (devices,): (Vec<DBusPath<'static>>,) =
        proxy.method_call(UPOWER_SERVICE, "EnumerateDevices", ())?;

    for device_path in devices {
        register_battery_watch(
            connection,
            &device_path,
            Arc::clone(&known_devices),
            Arc::clone(&device_states),
            Arc::clone(&actions),
            proxy_timeout,
        )?;
    }

    Ok(())
}

fn listen_for_device_updates(
    connection: &Connection,
    known_devices: Arc<Mutex<HashSet<String>>>,
    device_states: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    actions: Arc<ActionManager>,
    proxy_timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    let mut added_rule = MatchRule::new();
    added_rule.msg_type = Some(MessageType::Signal);
    added_rule.member = Some("DeviceAdded".into());
    added_rule.interface = Some(UPOWER_SERVICE.into());
    added_rule.path = Some(DBusPath::new(UPOWER_PATH).expect("valid path"));

    let added_devices = Arc::clone(&known_devices);
    let added_states = Arc::clone(&device_states);
    let added_actions = Arc::clone(&actions);
    let _added_token = connection.add_match::<(DBusPath<'static>,), _>(
        added_rule,
        move |(path,), conn, _msg| {
            if let Err(err) = register_battery_watch(
                conn,
                &path,
                Arc::clone(&added_devices),
                Arc::clone(&added_states),
                Arc::clone(&added_actions),
                proxy_timeout,
            ) {
                eprintln!("BatWatch: failed to register device {path:?}: {err}");
            }
            true
        },
    )?;

    let mut removed_rule = MatchRule::new();
    removed_rule.msg_type = Some(MessageType::Signal);
    removed_rule.member = Some("DeviceRemoved".into());
    removed_rule.interface = Some(UPOWER_SERVICE.into());
    removed_rule.path = Some(DBusPath::new(UPOWER_PATH).expect("valid path"));

    let removed_devices = Arc::clone(&known_devices);
    let removed_states = Arc::clone(&device_states);
    let _removed_token = connection.add_match::<(DBusPath<'static>,), _>(
        removed_rule,
        move |(path,), _conn, _msg| {
            let mut guard = removed_devices.lock().expect("poisoned mutex");
            let path_str = path.to_string();
            if guard.remove(&path_str) {
                println!("{} disconnected", label_for_path(path_str.as_str()));
            }
            removed_states
                .lock()
                .expect("poisoned mutex")
                .remove(&path_str);
            true
        },
    )?;

    Ok(())
}

fn listen_for_root_updates(
    connection: &Connection,
    proxy_timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    let proxy = connection.with_proxy(UPOWER_SERVICE, UPOWER_PATH, proxy_timeout);
    if let Ok(on_battery) = proxy.get::<bool>(UPOWER_INTERFACE, "OnBattery") {
        print_power_source(on_battery);
    }

    let mut rule = MatchRule::new();
    rule.msg_type = Some(MessageType::Signal);
    rule.interface = Some(PROPERTIES_INTERFACE.into());
    rule.member = Some("PropertiesChanged".into());
    rule.path = Some(DBusPath::new(UPOWER_PATH).expect("valid path"));

    let _token = connection.add_match::<(String, PropMap, Vec<String>), _>(
        rule,
        move |(interface, changed, _), _conn, _msg| {
            if interface == UPOWER_INTERFACE
                && let Some(value) = changed.get("OnBattery")
                && let Some(on_battery) = value.0.as_i64().map(|v| v != 0)
            {
                print_power_source(on_battery);
            }
            true
        },
    )?;

    Ok(())
}

fn register_battery_watch(
    connection: &Connection,
    path: &DBusPath<'static>,
    known_devices: Arc<Mutex<HashSet<String>>>,
    device_states: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    actions: Arc<ActionManager>,
    proxy_timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    let path_string = path.to_string();
    {
        let mut guard = known_devices.lock().expect("poisoned mutex");
        if !guard.insert(path_string.clone()) {
            return Ok(());
        }
    }

    let proxy = connection.with_proxy(UPOWER_SERVICE, path.clone(), proxy_timeout);
    let device_type: u32 = proxy.get(DEVICE_INTERFACE, "Type")?;
    if device_type != BATTERY_DEVICE_TYPE {
        let mut guard = known_devices.lock().expect("poisoned mutex");
        guard.remove(&path_string);
        return Ok(());
    }

    print_status(
        &proxy,
        &path_string,
        Arc::clone(&device_states),
        Arc::clone(&actions),
    )?;

    let mut rule = MatchRule::new();
    rule.msg_type = Some(MessageType::Signal);
    rule.interface = Some(PROPERTIES_INTERFACE.into());
    rule.member = Some("PropertiesChanged".into());
    rule.path = Some(path.clone());

    let state_map = Arc::clone(&device_states);
    let action_map = Arc::clone(&actions);
    let _watch_token = connection.add_match::<(String, PropMap, Vec<String>), _>(
        rule,
        move |(interface, changed, _), conn, msg| {
            if interface == DEVICE_INTERFACE
                && (changed.contains_key("Percentage") || changed.contains_key("State"))
                && let Some(path) = msg.path()
            {
                let path_string = path.to_string();
                let proxy = conn.with_proxy(UPOWER_SERVICE, path_string.clone(), proxy_timeout);
                if let Err(err) = print_status(
                    &proxy,
                    &path_string,
                    Arc::clone(&state_map),
                    Arc::clone(&action_map),
                ) {
                    eprintln!("BatWatch: failed to read percentage for {path_string}: {err}");
                }
            }
            true
        },
    )?;

    Ok(())
}

fn print_status(
    proxy: &dbus::blocking::Proxy<&Connection>,
    path: &str,
    device_states: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    actions: Arc<ActionManager>,
) -> Result<(), dbus::Error> {
    let percentage: f64 = proxy.get(DEVICE_INTERFACE, "Percentage")?;
    let state: u32 = proxy.get(DEVICE_INTERFACE, "State")?;
    println!(
        "{}: {:.0}% ({})",
        label_for_path(path),
        percentage,
        describe_state(state)
    );

    let percent_u8 = percentage.round().clamp(0.0, 100.0) as u8;
    let battery_state = BatteryState::from_code(state);
    process_status_update(path, percent_u8, battery_state, device_states, actions);
    Ok(())
}

fn label_for_path(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn describe_state(state: u32) -> &'static str {
    match state {
        1 => "charging",
        2 => "discharging",
        3 => "empty",
        4 => "full",
        5 => "pending charge",
        6 => "pending discharge",
        _ => "unknown",
    }
}

fn print_power_source(on_battery: bool) {
    if on_battery {
        println!("Power: on battery");
    } else {
        println!("Power: plugged in");
    }
}

fn process_status_update(
    path: &str,
    percentage: u8,
    state: BatteryState,
    device_states: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    actions: Arc<ActionManager>,
) {
    if actions.is_empty() {
        return;
    }

    let mut map = device_states.lock().expect("poisoned mutex");
    let entry = map.entry(path.to_string()).or_default();
    let event = StatusEvent {
        device_path: path.to_string(),
        percentage,
        previous_percentage: entry.last_percentage,
        state,
        previous_state: entry.last_state,
    };
    entry.last_percentage = Some(percentage);
    entry.last_state = Some(state);
    drop(map);

    actions.handle_event(event);
}

#[derive(Default)]
struct DeviceStatus {
    last_state: Option<BatteryState>,
    last_percentage: Option<u8>,
}

#[cfg(test)]
mod tests;
