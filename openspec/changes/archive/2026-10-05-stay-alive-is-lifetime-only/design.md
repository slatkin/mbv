# Design

## Context

See proposal.md for the motivation. The current state:

- **Route switch.** `App::attach_remote_owner` (`src/app/dispatch/session/switch.rs`) is the only
  place that suspends a current home link. Before suspending, it sends Stop when `stay_alive` is
  false. `switch_to_direct_remote` and `switch_to_library_route` both go through it.
- **Notifications.** `App::new_remote` (`src/app/state/construct/remote.rs`) computes
  `system_notifications = !stay_alive && config.system_notifications`. The settings-overlay toggle
  (`src/app/dispatch/settings.rs`) assigns the config value directly, so startup and a
  mid-session toggle disagree.
- **Tray start.** The Owner process decides the Tray once, in `start_tray` (`crates/mbv-daemon/src/run.rs`),
  gated on `owner_settings().stay_alive`.
  - The hook it calls (`on_tray_ready`, built in `src/local_daemon.rs`) is an `FnOnce`. It
    checks `show_systray_icon` from the spawn-time config and `take()`s the
    `DaemonPlayerHandle` out of a `Mutex<Option<_>>`.
  - The returned `Box<dyn Send>` lives for the life of the loop.
- **Tray stop.** `mbv_desktop::tray::spawn` returns a boxed ksni `blocking::Handle`. Dropping a
  ksni handle does **not** stop the tray: the service loop ignores a closed handle channel
  (ksni 0.3.6 `service.rs`), so only `Handle::shutdown().wait()` removes the icon.
- **Owner settings.** `OwnerSettingsReader` re-reads the config file each time it is called.
  The Owner process has no change notification, and ADR 0030 rejects Clients pushing settings.
- **Event loop.** The daemon loop calls `DaemonLoop::tick(now)` every 25 ms
  (`run_daemon_loop`).

## Goals / Non-Goals

**Goals:**

- No Client or Player-owner branch on `stay_alive` other than three: lifetime (teardown request,
  `LastClientGone`, `prepare_shutdown`), exclusive admission, and whether the Tray is forced on.
  The status-chrome indicator keeps showing the policy.
- The Tray's effective value is derived as `stay_alive || show_systray_icon` wherever it is
  needed, never stored.

**Non-Goals:**

- Changing exclusive admission or the unconditional coordinated shutdown. The user decided to
  keep both as specified.
- A dimmed or locked row style in the settings overlay. The row shows the effective value and
  refuses the toggle with a toast.
- Making `show_audio_window` or any other owner setting live. Only the Tray reconciles.

## Decisions

### D1. Delete the route-switch Stop outright

Remove the `if !stay_alive { self.player.stop() }` block in `attach_remote_owner`. The suspended
home link already drains owner events every tick (`drain_suspended_home_events`), so the Local
queue view stays current while the link is suspended. Nothing else depends on the Stop.

*Alternative:* always stop. Rejected because it would destroy local playback for stay-alive users
who cast while something plays locally.

### D2. Notifications follow the config alone

`App::new_remote` passes `app_config.system_notifications` through unchanged. The overlay toggle
already does the same, so startup and toggle now share one rule.

### D3. The effective Tray value is computed from both settings

The Tray is enabled when `stay_alive || show_systray_icon`. That expression is computed in two
places, each reading its own copy of config, and nothing new is stored:

- `OwnerSettings` (Owner process) gains `show_systray_icon` and a `tray_enabled()` method.
- The settings model (`setting_boolean_value`) displays `stay_alive || show_systray_icon` for the
  `ShowSysTrayIcon` row.

The overlay toggle checks `stay_alive`. If it is on, the toggle leaves config untouched and shows
a Neutral toast: "Tray stays on while Stay alive is on". Writing `show_systray_icon = true` when
Stay-alive turns on was rejected: it would lose the user's preference and store the same fact
twice.

The Packaged role's reader forces `stay_alive: true`, so `tray_enabled()` is true for mbvd too.
That is harmless: mbvd's tray hook is a no-op, and the reconcile in D4 only calls the hook when
the value changes.

### D4. The Owner process reconciles the Tray on a 1 s cadence

`DaemonLoop` owns the Tray state: the current `Option<Box<dyn Send>>`, the last `tray_enabled`
value it acted on, the tray hook, and a `shutdown_signal_tx` clone.

- In `tick(now)`, at most once per second, it calls `owner_settings()`. If `tray_enabled()`
  differs from the last value it acted on:
  - when it turned on, it calls the hook and stores the result;
  - when it turned off, it drops the stored Tray.
- The startup path runs the same reconcile once, in place of `start_tray`.

The hook is called only when the value changes, never whenever the Tray happens to be `None`.
A headless host (hook returns `None`) therefore isn't retried every second, and the "not
available" warning is logged once per enable, as today.

The cadence costs one config-file read and parse per second. The config file is small, and a
change shows up after the Client's debounced settings save plus at most 1 s.

*Alternatives:*
- **Watch the file with inotify:** a new dependency, or new libc code, for a single setting.
- **Have the Client push settings over ctrl:** rejected by ADR 0030, since multiple Clients
  could disagree.
- **Re-read only on Client events:** a Stay-alive toggle produces no ctrl event, so the Tray
  would lag until some other event arrived.

### D5. Dropping the Tray removes the icon

`mbv_desktop::tray::spawn` returns a small owning wrapper around the ksni handle whose `Drop`
calls `handle.shutdown().wait()`. The `Box<dyn Send>` boundary between `mbv-daemon` and
`mbv-desktop` stays as it is: `mbv-daemon` still can't name ksni, and dropping the box (D4, or
loop teardown) is the stop operation.

*Alternative:* a `stop()` trait object across the hook boundary. That adds a trait to
`mbv-daemon` for a single implementation, whereas `Drop` matches how the box is already released
at exit.

### D6. The tray hook becomes callable more than once

The hook type in `DaemonRuntimeHooks` changes from `FnOnce` to `FnMut`. In `src/local_daemon.rs`
it no longer checks the spawn-time `show_systray_icon`; the decision now lives in
`OwnerSettings`. It also no longer `take()`s the player handle. Instead it keeps the status `Arc`
and transport `Sender` and clones them on each call (derive or implement `Clone` for
`DaemonPlayerHandle`; both fields are cheaply cloneable).

## Risks / Trade-offs

- **`Drop` blocking on `shutdown().wait()` could stall the daemon loop if the D-Bus connection
  hangs.** ksni's shutdown closes the connection and signals straight away, so the wait is
  short. If it is ever seen to hang, it can move to a detached thread.
- **Toggling repeatedly spawns and stops ksni services.** Each enable creates a fresh service
  and D-Bus name. Toggles are user-paced and ksni supports respawning, so this is acceptable.
- **The default flip (`show_systray_icon`: true → false) is user-visible.** A config with no
  stored key loses nothing, because before this change the Tray only appeared with Stay-alive
  on, and Stay-alive now forces it. A config that stored `true` keeps it, and with Stay-alive off
  now gets a Tray it never had.
- **Route switching keeps local audio playing alongside a remote target.** MPRIS follows the
  current player, so the local queue is controlled by routing back home, or from the Tray when
  it is shown. This is intended (proposal).

## Migration Plan

No data migration is needed. The default change applies only to configs that don't store
`show_systray_icon`. To roll back, revert the commit.
