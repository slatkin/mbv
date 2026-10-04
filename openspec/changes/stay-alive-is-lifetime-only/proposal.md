# Proposal

## Why

ADR 0030 made the Owner process the only local Player owner whatever the Stay-alive setting, so
Stay-alive should only decide when that process exits. `queue-owner-process` (#857) kept several
Bare-era behaviours for stay-alive-off users, and those now hang off the lifetime policy: route
switching stops local playback, system notifications are silently suppressed while Stay-alive is
on, and the Tray ignores its own "Show systray icon" setting. These branches box users into
behaviour they didn't choose.

## What Changes

- Switching the playback route away from the home link no longer stops local playback, whatever
  Stay-alive is set to. The local queue keeps playing on the suspended home link.
- `system_notifications` alone decides whether toasts are mirrored to desktop notifications. The
  Stay-alive gate at Client construction is removed, so the setting works in stay-alive sessions
  again.
- The Tray is shown when `stay_alive || show_systray_icon`:
  - While Stay-alive is on, the Tray is forced on. The settings row shows it as on, and a toggle
    is refused with a toast. The stored `show_systray_icon` preference is never overwritten.
  - **BREAKING (default):** `show_systray_icon` now defaults to `false`. A config that already
    stores `true` keeps it.
  - The Owner process starts and stops the Tray live when the effective value changes, with no
    restart needed.
- Stay-alive still decides three things: the Owner process lifetime, exclusive admission and the
  forced Tray. Exclusive admission and the unconditional coordinated shutdown are unchanged. The
  status-chrome indicator still shows the policy.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `daemon-lifecycle`: Stay-alive decides only lifetime, admission count and whether the Tray is
  forced on. Route switching never stops local playback.
- `local-daemon-tray`:
  - The Tray belongs to the local Owner process, not only a stay-alive one.
  - It is shown when Stay-alive is on or "Show systray icon" is on, and follows that value live.
  - The configuration-disabled case applies only while Stay-alive is off.
- `toast-notification-semantics`: desktop notifications follow `system_notifications`
  independently of Stay-alive.

## Impact

- **Client (TUI):**
  - `src/app/dispatch/session/switch.rs`: the route-switch Stop.
  - `src/app/state/construct/remote.rs`: the notification gate.
  - `src/app/dispatch/settings.rs`: the locked Tray toggle.
- **Settings model:** `crates/mbv-ui-model/src/settings.rs` displays the effective Tray value.
- **Config:** the default in `crates/mbv-config/src/parse.rs` and `types_paths.rs`.
- **Owner process:**
  - `crates/mbv-daemon/src/owner_settings.rs` gains `show_systray_icon`.
  - `run.rs` and the event loop handle live Tray start and stop.
- **Desktop:** `crates/mbv-desktop/src/tray.rs` gets a stoppable Tray handle (ksni needs an
  explicit `Handle::shutdown`).
- **Local bootstrap:** `src/local_daemon.rs` makes the tray hook callable more than once.
- **Docs:** CONTEXT.md (*Tray*), plus a status note on ADR 0030.
