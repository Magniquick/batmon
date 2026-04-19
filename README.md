# BatWatch 🦇

BatWatch keeps an ear on UPower events and pings your scripts whenever your laptop’s cells change mood. Think of it as a polite Alfred reporting on the Batmobile’s charge levels while you hack away.

![Batman and Catwoman under a bat-shaped umbrella](assets/batman.png)
<sub>Cartoon by [Khalid Birdsong](https://khalidbirdsong.substack.com/p/are-these-batman-cartoons-funny)</sub>

## Utility Belt
- Rust binary (`cargo run`) that listens to DBus and prints status updates in real time.
- Configurable polling + DBus timeouts via `batwatch.toml`.
- Optional `[charging]` and `[discharging]` hooks so your custom gadgets (scripts) fire when the battery plugs in, crosses a `when` threshold, or drains. Define multiple hooks with subtables such as `[charging.toast]` / `[discharging.backup]`, each providing its own `script`/`when`. Scripts run directly, so mark them executable and include a shebang or ship an ELF binary; bare names are resolved under `<config_dir>/scripts/` before falling back to `$PATH`.
- `--debug` flag prints the current config before standing watch.

## Quick Start
```
cargo run -- --debug
```
Adjust `batwatch.toml`, then let BatWatch alert your scripts like a trusty Bat-Signal.

_Config search order:_ `$BATWATCH_CONFIG` → `~/.config/batwatch/batwatch.toml` → `~/.config/batwatch.toml` → `./batwatch.toml`.

### Install Icon

BatWatch ships full-color and symbolic icons under `assets/`. Install them into your icon theme so notifications pick up the adaptive glyph:

```
scripts/install-icons.sh
```

The script copies `batwatch.svg` and `batwatch-symbolic.svg` into `~/.local/share/icons/hicolor/scalable/apps/` and refreshes the cache. Notifications default to the symbolic name `batwatch-symbolic`; override via `BATWATCH_ICON` if you prefer another icon.

### Install via Cargo

To install the BatWatch binary into your Cargo bin directory (`$HOME/.cargo/bin` by default):

```
cargo install --path .
```

Ensure that directory is on your `PATH`, then bootstrap the bundled config, hook scripts, icons, and user service:

```
batwatch --init-config
```

Run `batwatch --debug` to confirm the binary works. The installed binary embeds the default `batwatch.toml`, hook scripts, notification icons, and `batwatch.service`, so `cargo install --path .` and `cargo install --git ...` do not need the source tree at runtime. Re-run `batwatch --init-config --force` after upgrading if you want to overwrite your local copies with the bundled defaults.

### Default Hooks

The bundled `batwatch.toml` ships with sensible power-management hooks:

- When you plug in, BatWatch switches `powerprofilesctl` to `performance` once per charging session and notifies you when the battery reaches 100%.
- When you unplug, it reverts to the balanced profile (once per discharging session).
- Discharging warnings at 20% and 15% enable the `power-saver` profile and emit low-priority notifications.
- A critical warning at 10% sets power-saver, sends a critical notification, and—if still discharging—issues a hybrid sleep after 15 seconds.

All hooks are ordinary shell scripts under `scripts/`; tweak or extend them to match your workflow.


<sub>Notify icon credit: [bat.svg](assets/bat.svg) from [OnlineWebFonts](https://www.onlinewebfonts.com/icon/509803), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).</sub>
