## 1. Config plumbing (mbv-core)

- [x] 1.1 Add `pub mouse_support: bool` to `Config` (crates/mbv-core/src/config_types_paths.rs) with `Default` value `true`, doc comment noting it gates terminal mouse capture and is editable via F2. Verify: `cargo check -p mbv-core`.
- [x] 1.2 Parse `mouse_support` from the `[display]` section in `config_parse.rs` (absent → `true`, mirroring the `show_systray_icon`/`system_notifications` pattern) and add the save entry in `config_save.rs` beside `system_notifications`. Verify: unit test in the existing config tests files covering round-trip save→parse with `false`, and default-on when the key is absent; `cargo nextest run -p mbv-core`.

## 2. Capture gate (mbv TUI)

- [x] 2.1 Extract `set_mouse_capture(writer, enabled)` helper in src/app/mod.rs emitting `EnableMouseCapture`/`DisableMouseCapture`; rewire `init_terminal` and `restore_terminal` onto it. `init_terminal` takes a `mouse_support: bool` parameter from `Model::run` (caller reads the config handle); the enable call is gated on it, the disable stays unconditional. Verify: `cargo check -p mbv`; existing suite green (`cargo nextest run -p mbv`).
- [ ] 2.2 Update the `init_terminal` call in `Model::run` (src/app/shell_run.rs) to pass `self.app`'s config `mouse_support` value. Verify: launch with `mouse_support = false` in a manual run shows no mouse capture (tui does not consume mouse events); default launch unchanged.

## 3. Settings panel toggle (mbv TUI)

- [x] 3.1 Add `SettingKey::MouseSupport` to the Display section in `SETTING_SECTIONS` (src/app/types_settings.rs), add the label ("Mouse support") and value mapping (`bool_val(cfg.mouse_support)`) beside the `SystemNotifications` entries in src/app/settings.rs, and render its current value in the Display section painter (src/app/render/components/settings.rs screen content — follow the `SystemNotifications` bool-row shape). Verify: settings panel shows the row with correct on/off text; `cargo nextest run -p mbv`.
- [ ] 3.2 Add the toggle arm in `handle_settings_activate`: flip `config.mouse_support` under the lock, then apply live capture through the `set_mouse_capture` helper executed on the session stdout. No component-local mirror is needed — `settings_snapshot` rebuilds rows from config on every sync pass, so the row repaints from the flipped value. Persistence rides the existing `settings_save_at` debounce; no new save path. Verify: unit test covering the toggle arm flips the config value; manual check that toggling off stops mouse-event delivery and toggling on resumes it mid-session.
- [x] 3.3 Add a routing/keyboard test for the new row per repo convention: mount the Settings destination through the tick harness, navigate to the MouseSupport row, send Enter, assert the config value flipped (mounting/focus changes flow through shell sync — direct `Component::on` tests are not sufficient). Verify: `cargo nextest run -p mbv`.

## 4. Spec sync and gates

- [x] 4.1 Check whether the toggle's mount/focus/routing surface crosses into Wide/Narrow presentations or panel painting (it should not — the Display section gains a row, not a surface); if the row rendering shifts any painted geometry, add/adjust buffer tests for the affected painter. Verify: `cargo nextest run -p mbv` and existing settings buffer tests untouched.
- [x] 4.2 Full gates: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`, `cargo nextest run -p mbv`, and `cargo test --release -- --test-threads=4` for the mbv package (CI parity). Verify: all green, no new warnings.
