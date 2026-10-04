# Proposal

## Why

Mini view (terminal narrower than 80 columns) paints its single panel with the resting palette
whether or not the window has focus, because there is no sibling panel to contrast against. In the
pinned panel that leaves no sign inside mbv of whether the panel holds the keyboard. The pinned
panel already learns this from pinwin: GTK focus enter and leave arrive in the pty as terminal
focus reports, which mbv already enables and receives.

## What Changes

- `App` records whether the window holds focus, set by the existing focus-gained and focus-lost
  handlers. It starts as focused, since the pinned app launches with focus.
- When the process is pinned, mini view paints the focused palette while the window is focused and
  the resting palette while it is not.
- Wide view and non-pinned mini view are unchanged.
- The `panel_appearance_focus` unit test that fixes "mini view is never focused" is rewritten for
  the new rule.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `panel-mode`: `Queue-only renders the queue panel focused when it holds focus` is narrowed to
  exclude mini view, and a new requirement `Pinned mini view follows window focus` is added.

## Impact

- `src/app/state/panel_focus.rs`: `panel_appearance_focus`, the focus-gained and focus-lost
  handlers.
- `src/app/state/app_struct.rs`, `src/app/state/construct.rs`: the new field.
- `src/app/tests/panel_focus.rs`: the appearance-focus test.
- No pinwin, protocol, config or daemon change. Independent of `pinned-panel-focus-accent`.
