# Tasks

## 1. Client: route switching and notifications ignore Stay-alive

- [x] 1.1 In `src/app/dispatch/session/switch.rs` `attach_remote_owner`, delete the
  `if !self.config.lock().unwrap().stay_alive { self.player.stop(); }` block. The home link is
  still suspended, just never stopped first (design D1). In `src/app/tests/route_state.rs`,
  replace `switching_away_with_stay_alive_disabled_sends_stop_to_home_link` with
  `switching_away_from_home_never_stops_local_playback`. It keeps `stay_alive = false` (the case
  that used to stop) and asserts `suspended_local.is_some()` and that `home_commands` received
  **no** `PlaybackIntent` Stop. Contract: `daemon-lifecycle` "Switching away from the home link
  keeps local playback". Verify: `cargo nextest run -p mbv route_state` passes.
- [x] 1.2 In `src/app/state/construct/remote.rs` `new_remote`, set
  `let system_notifications = app_config.system_notifications;` (design D2). In the same file's
  tests:
  - delete
    `daemon_lifecycle_local_daemon_is_independent_of_emby_setup_uses_system_notifications_when_stay_alive_is_off`;
  - change `..._suppresses_notifications_when_stay_alive_is_on` into
    `system_notifications_follow_config_when_stay_alive_is_on`, keeping `stay_alive: true` and
    asserting `app.system_notifications`.

  Contract: `toast-notification-semantics` "Stay-alive does not suppress notifications". Verify:
  `cargo nextest run -p mbv construct::remote` passes.
- [x] 1.3 Add a status note to `docs/adr/0030-owner-process-is-the-only-local-player-owner.md`
  saying that stay-alive-off no longer keeps Bare behaviour for route switching or system
  notifications, and that Stay-alive decides only lifetime, exclusive admission and the forced
  Tray (change `stay-alive-is-lifetime-only`). Verify: `rg -n "stay-alive-is-lifetime-only"`
  shows it in the ADR. Group gate: `cargo clippy -p mbv --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean.

## 2. The Tray setting is derived from both settings

- [x] 2.1 Change the `show_systray_icon` default from `true` to `false` in
  `crates/mbv-config/src/parse.rs` (`unwrap_or(true)` in `PlaybackSettings`) and in
  `crates/mbv-config/src/types_paths.rs` (`Config` default). Don't add a test for the default
  value. Verify: `cargo nextest run -p mbv-config` passes; update any existing assertion that
  pinned `true`.
- [x] 2.2 In `crates/mbv-daemon/src/owner_settings.rs`:
  - add `show_systray_icon: bool` to `OwnerSettings`, filled from config in `From<&Config>`;
  - add `pub(crate) fn tray_enabled(&self) -> bool { self.stay_alive || self.show_systray_icon }`;
  - update `fixed_reader` and the existing reader test's struct literals so they compile.

  No new test: the method is one expression, and 3.3's tests exercise it. Verify:
  `cargo check -p mbv-daemon --all-targets`.
- [x] 2.3 In `crates/mbv-ui-model/src/settings.rs` `setting_boolean_value`, show the
  `ShowSysTrayIcon` row as `cfg.stay_alive || cfg.show_systray_icon` (design D3). Add one test:
  with `stay_alive = true` and `show_systray_icon = false`, `setting_value(ShowSysTrayIcon, ..)`
  returns `bool_val(true)`. Contract: `local-daemon-tray` "Row while stay-alive is on". Verify:
  `cargo nextest run -p mbv-ui-model` passes.
- [x] 2.4 In `src/app/dispatch/settings.rs`, refuse the `ShowSysTrayIcon` toggle while
  `stay_alive` is on:
  - leave config unchanged and call
    `self.flash("Tray stays on while Stay alive is on".into(), ToastSeverity::Neutral)`;
  - otherwise toggle as today.

  `toggle_config_setting` holds the config lock and can't flash, so handle `ShowSysTrayIcon` in
  the caller before it reaches that function, then remove its arm there. Add one test:
  `stay_alive = true`, `show_systray_icon = false`, toggle ShowSysTrayIcon, assert the config
  value stays `false`. Contract: `local-daemon-tray` "Toggle refused while stay-alive is on".
  Verify: `cargo nextest run -p mbv settings` passes. Group gate: clippy (`-p mbv-config -p
  mbv-daemon -p mbv-ui-model -p mbv`) and `cargo fmt --all -- --check` are clean.

## 3. The Owner process starts and stops the Tray live

- [x] 3.1 In `crates/mbv-desktop/src/tray.rs`, make `spawn` box an owning wrapper (for example
  `struct RunningTray(ksni::blocking::Handle<MbvTray>)`) whose `Drop` calls
  `self.0.shutdown().wait()`. Dropping a raw ksni handle does not stop the service (design D5).
  Keep the `Option<Box<dyn Send>>` return type. Update the doc comment, which currently says
  "stay-alive tray", to say the Tray is enabled by Stay-alive or "Show systray icon". No unit
  test (it needs D-Bus); the manual check is in 4.1. Verify: `cargo check -p mbv-desktop`.
- [x] 3.2 Make the tray hook callable more than once (design D6):
  - in `crates/mbv-daemon/src/core.rs`, change `OnTrayReady` to
    `Box<dyn FnMut(mpsc::SyncSender<()>) -> Option<Box<dyn Send>>>`, and derive `Clone` for
    `DaemonPlayerHandle`;
  - in `src/local_daemon.rs`, drop the spawn-time `show_systray_icon` check. In the hook, clone
    the stored handle (`.lock().unwrap().clone()?`) instead of `take()`;
  - in `crates/mbvd/src/main.rs`, the `Box::new(|_| None)` hook needs no change beyond
    compiling.

  Verify: `cargo check -p mbv-daemon -p mbv -p mbvd --all-targets`.
- [x] 3.3 In `crates/mbv-daemon`, move Tray ownership into the daemon loop (design D4):
  1. Create a new module `src/event_loop/tray.rs` holding a `TrayState` with these fields:
     - the hook;
     - a `shutdown_signal_tx` clone;
     - `tray: Option<Box<dyn Send>>`;
     - `enabled: bool`, the last value acted on;
     - `last_check: Instant`.
  2. Give it `reconcile(&mut self, enabled: bool)`:
     - it calls the hook only when `enabled` turns true;
     - it sets `tray = None` when `enabled` turns false;
     - it does nothing when the value is unchanged, even if `tray` is `None`.
  3. Give it `poll(&mut self, now: Instant, settings: &OwnerSettingsReader)`, which calls
     `reconcile(settings().tray_enabled())` at most once per second.
  4. In `run.rs`:
     - replace `start_tray` and the `_tray` fields with a `TrayState` that runs one
       `reconcile` at startup, in the same place `start_tray` ran;
     - store the `TrayState` on `DaemonLoop`;
     - call `poll` from `DaemonLoop::tick`.

  Tests in `event_loop/tray.rs`, using a counting hook that returns a box whose `Drop` sets a
  flag, and no sleeps (pass `now` explicitly):
  - turning on calls the hook once;
  - turning off drops the Tray;
  - an unchanged `true`, with the hook returning `None`, does not call the hook again;
  - `poll` within 1 s of the last check doesn't read settings.

  Contracts: `local-daemon-tray` "Stay-alive turned on/off during the session" and "A missing
  tray is not an error". Verify: `cargo nextest run -p mbv-daemon` passes.
- [x] 3.4 Update CONTEXT.md *Tray*: replace "For the local Owner process, it is present only when
  Stay-alive is enabled." with "For the local Owner process, it is present while Stay-alive is
  enabled or Show systray icon is on, and follows those settings live." Verify: `rg -n "follows
  those settings live" CONTEXT.md`. Group gate: `cargo clippy --workspace --all-targets -- -D
  warnings` and `cargo fmt --all -- --check` are clean.

## 4. Integration

- [ ] 4.1 Hand the user these manual live checks (do not run mbv yourself):
  - (a) With Stay-alive off and "Show systray icon" off: no Tray. Toggle "Show systray icon" on
    and the Tray appears within about 1 s of the settings save; toggle it off and it disappears.
  - (b) Toggle Stay-alive on: the Tray appears and the row reads on. Toggling the row shows the
    toast and changes nothing. Toggle Stay-alive off: the Tray disappears if the preference was
    off.
  - (c) With Stay-alive off, play locally, then switch to a Library route or direct remote:
    local playback continues. Route back home and the queue is still playing.
  - (d) With Stay-alive on and system notifications on, an Error toast raises a desktop
    notification.

  Verify: the user reports the results.
