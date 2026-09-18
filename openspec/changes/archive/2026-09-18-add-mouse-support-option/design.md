## Context

Mouse support (ADR 0024) is unconditional at exactly one point:
`init_terminal` (src/app/mod.rs:355) always emits
`crossterm::event::EnableMouseCapture`; `restore_terminal` always emits the
disable. Everything downstream — per-frame mouse subscriptions
(`sync_mouse_subscriptions`), the mouse fold, hit resolution — is already
conditional on "was this surface painted this frame", never on "is mouse
wanted at all". All mouse integration tests inject events through the tick
harness and never touch a real terminal, so a capture-level gate cannot
affect them.

Config plumbing for a persisted bool already exists end to end:
`[display]` section (`system_notifications` is the precedent), with the
parse-default-save trio in `config_parse.rs` / `config_save.rs` /
`Config::default`, plus a Settings panel row pattern
(`SettingKey` → `SETTING_SECTIONS` → `handle_settings_activate` toggle arm
→ debounced `settings_save_at` persistence).

## Goals / Non-Goals

**Goals:**

- One gate, at the capture boundary: no capture → no mouse bytes → every
  downstream mouse layer idles untouched.
- Live toggle from F2 without restart, applied via the same code path as
  launch-time setup.
- Capture sequences exist in exactly one place.

**Non-Goals:**

- Any change to ADR 0024's subscription/reconciliation model, the mouse
  fold, or hit-resolution semantics.
- Suppressing `mouse_sub()` subscriptions when mouse is off (subscriptions
  may remain registered; with capture off no events arrive to fire them).
- Live-toggle support for any other terminal setup (focus-change,
  keyboard-enhancement flags, shift-escape mode).
- Clearing stale hover state on toggle-off (accepted cosmetic edge, see
  Risks).

## Decisions

- **Gate at `EnableMouseCapture` only; leave every downstream layer
  alone.** With capture disabled the terminal never sends mouse bytes, so
  subscriptions never fire and the fold never sees a mouse tick. The
  alternative — also suppressing `mouse_sub()` in
  `sync_mouse_subscriptions` — adds reconciler coupling and a way for the
  test harness to disagree with production, for zero observable gain.
  The fold's structural mouse-tick detection is unaffected in both
  directions: no events means no mouse ticks; re-enabling mid-session
  means events arrive through the existing, already-armed subscriptions.

- **One shared `set_mouse_capture` helper, three callers.** `init_terminal`
  (gated on the config value), `restore_terminal` (still unconditional —
  disabling when never enabled is a no-op, and symmetry keeps teardown
  simple), and the settings toggle arm (applies the live flip). Without the
  helper, the toggle arm would be a second site inventing capture
  sequences that must then be kept in sync with launch setup.

- **`init_terminal` takes the flag as a parameter.** It is called from
  `Model::run`, which holds the config handle. Reading the config inside
  `init_terminal` would hide the dependency for no benefit.

- **Live toggle applies immediately, persists via the existing debounce.**
  `handle_settings_activate` runs on `Model` inside the run loop, so the
  toggle arm flips the config value under the lock and records the flip in
  `mouse_capture_pending`; the run loop consumes the pending flag in the
  same iteration and executes the helper against the session stdout (the
  arm itself holds no terminal handle, so direct execution from the arm
  would be a second capture site with the wrong I/O context).
  `settings_save_at` continues to own persistence, exactly as the
  `SystemNotifications` arm separates apply-now from save-later.

- **Setting lives in `[display]` as `mouse_support`, default `true`.**
  Matches the section the user-facing row renders under (Display), and
  follows the `show_systray_icon`-style naming for terminal-facing
  presentation toggles. Absent value parses to `true`, so existing configs
  change nothing on upgrade.

## Risks / Trade-offs

- [Hover appearance can stick when mouse is toggled off mid-hover] →
  Accepted cosmetic edge: no further mouse event arrives to clear it, and
  the next keyboard-driven repaint resets local presentation state. Not
  worth synthetic-clear machinery.
- [Mid-session enable starts event delivery into a running session] →
  No re-arming is needed because subscriptions are refreshed per painted
  frame; the first mouse tick flows through the ordinary path.
- [Toggling rapidly could interleave capture enables/disables] →
  The helper emits idempotent DECSET sequences; a stray enable/disable pair
  converges to the last-written state. No debouncing added unless it
  proves noisy.

## Migration Plan

None needed. Default `true` preserves current behavior; the new config key
is simply absent from existing files until first settings save.

## Open Questions

None.
