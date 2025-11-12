# Repository Guidelines

## Project Structure & Module Organization
This repo is a single Rust crate defined in `Cargo.toml`. Runtime logic currently lives in `src/main.rs`; factor reusable pieces into modules under `src/` and re-export them via `mod` statements so `main.rs` only wires dependencies. Add integration tests in `tests/` and keep DBus capture fixtures under `assets/`. Generated code lives in `target/` and should never be committed. Store machine-specific settings outside git (e.g., `.env.local` or your shell profile).

## Build, Test, and Development Commands
```
cargo fmt --all                              # enforce rustfmt across the tree
cargo clippy --workspace --all-targets -- -D warnings  # lint with Rust best-practice rules
cargo build --all-targets                    # ensure every binary/test compiles
cargo run --release                          # run the monitor with realistic perf/locking
cargo test --workspace --all-features        # execute unit + integration tests
```
Use `cargo doc --open` before large refactors to verify public APIs stay coherent.
- Use `cargo add <crate> [--features ...]` when introducing dependencies; never edit `Cargo.toml` by hand.

## Runtime Configuration
Edit the `[advanced]` section of `batwatch.toml` in the repo root (or place the file in `~/.config/batwatch/batwatch.toml`, fall back to `~/.config/batwatch.toml`, drop it alongside the binary, or point `BATWATCH_CONFIG` to a custom path). `poll_interval_secs` controls how often the DBus event loop calls `process`, and `proxy_timeout_secs` determines how long we wait on DBus property calls. Both default to `5` and clamp to at least `1` second to avoid busy loops or hung proxies. Under `[charging]` or `[discharging]` you can declare a default `script`/`when`, plus any number of named subtables such as `[charging.notify]` or `[discharging.log]`, each with its own `script` and optional `when`. Set `when = 80` to fire once when the percentage first crosses that threshold (>= for charging, <= for discharging), use `when = "always"` to run on every relevant event, or `when = "once"` for a single firing per state entry (charging maps that to `0`, discharging to `100`). Scripts execute directly (no shell), so they must be executable files with a shebang or be ELF binaries that handle their own arguments; bare names resolve first under `<config_dir>/scripts/` and then via `$PATH`. Run `cargo run -- --debug` to print the effective poll/timeout values before the monitor starts.

## Coding Style & Naming Conventions
Follow Rust 2024 defaults: four-space indent, `snake_case` functions/modules, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants (e.g., `UPOWER_SERVICE`). Propagate errors with `?` instead of `unwrap`/`expect`, and return `Result` from helpers so callers can bubble failures. Derive `Debug` and `Clone` on structs that cross threads, and gate expensive logging behind feature flags when possible. Always run `cargo fmt` and keep `clippy` clean; if a lint must be silenced, scope `#[allow(...)]` to the smallest block and document why.

## Testing Guidelines
Unit tests should accompany their modules under `#[cfg(test)] mod tests`, while DBus-heavy flows move to `tests/` and can use `cargo test -- --ignored` when they require system bus access. Name tests by behavior (`register_battery_watch_skips_non_batteries`) and assert on both output strings and state transitions. When fixing bugs, add regression tests first, then ensure the fix makes them pass. Prefer deterministic fixtures over random delays to keep CI reliable.

## Commit & Pull Request Guidelines
Use Conventional Commits with imperative, ≤72-character summaries (`feat: add upower watcher cache`). Describe the motivation and any user-facing changes in the body, reference issues (`Closes #7`), and include manual test notes or log snippets for reviewer context. A PR must show passing runs of `cargo fmt --all`, `cargo clippy --workspace --all-targets -D warnings`, and `cargo test --workspace`. When behavior changes, attach sample output or screenshots so reviewers can validate parity quickly.
