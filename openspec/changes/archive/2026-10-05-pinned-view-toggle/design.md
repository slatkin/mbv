# Design

## Context

`x` is the `panel_mode_cycle_x` keybind action (registry default chord `x`, key-policy entry
`panel_mode_cycle_x` → `Command::CyclePanelMode` → `App::cycle_panel_mode` in
`src/app/dispatch/action.rs`). Below `MINI_VIEW_THRESHOLD` (80) it toggles `mini_view_focus`;
at 80+ it cycles `panel_mode` through both -> queue-only -> library-only. Separately,
`pinned_width_toggle` (`Ctrl+e`) flips the running pinned panel between `PinnedWidth::Collapsed`
(`cols`) and `Expanded` (`cols_expanded`) via `crate::pin::apply_layout`, tracked in
`App::pinned_width`; a rejected layout keeps the current width and flashes a warning toast.
A pinned launch always starts collapsed, and at the default `cols = 40` that lands in mini view,
which starts queue-only — the collapsed-queue state already exists at launch.

## Decisions

### D1: One action, dispatch-level branch — no new keybind action

The registry rejects duplicate effective chords, so a second `x`-bound action
(`pinned_view_toggle`) cannot ship alongside `panel_mode_cycle_x`'s default `x` without forcing a
rebind. The pinned behaviour therefore branches inside `App::cycle_panel_mode`: when
`self.pinned_panel.is_some()`, call the new `pinned_view_toggle()`; otherwise run today's cycle
unchanged. Precedent for context-dependent behaviour inside one action exists (the `Playback`
bucket routes different conditions per key). Rebinding `x` moves both behaviours together, which
is the intent — it is one key doing "the panel thing". No `RouterSnapshot` change is needed: the
branch needs pinned-ness, which `App` owns and the snapshot deliberately does not.

### D2: Target view is derived from the tracked width, applied width-first

`pinned_view_toggle` mirrors `toggle_pinned_width`'s structure:

1. `let width = self.pinned_width.toggled();`
2. Apply the layout (`crate::pin::apply_layout` with the config for that width).
3. On `Ok`, set the view pinned to the target width; on `Err`, flash the reason and touch
   nothing; on not-pinned, unreachable (D1 guards the call site).

The view for the target width, assigned as direct field writes — never via `set_panel_focus`.
At dispatch time `terminal_width` still holds the pre-toggle width (the new size is only observed a
frame later through `pinned_resize_pending`), so on expand `set_panel_focus` would take its
mini-view early-return branch and leave stored `panel_focus` stale at `Queue`: at the expanded
width the router, the focus-follows-mode rule, and the wide palette all read the stored
`panel_focus`, not the mini-view derivation.

- `Expanded` -> library-only: `panel_mode = LibraryOnly`, `mini_view_focus = Library`,
  `panel_focus = Library`.
- `Collapsed` -> queue-only: `panel_mode = QueueOnly`, `mini_view_focus = Queue`,
  `panel_focus = Queue`, then `focus_queue_initial_item()` (the mini-view path's call)..

### D3: Set `panel_mode`, `mini_view_focus`, and `panel_focus` explicitly

Which of the view fields the renderer and the router consult depends on the live terminal width
(`effective_panel_mode` / `effective_panel_focus` read `mini_view_focus` below 80 columns and the
stored `panel_mode` / `panel_focus` at 80+), and a `[panel]`
configuration can put either width on either side of 80 columns (`cols` and `cols_expanded` are
independent integers). Assigning all three fields unconditionally makes the behaviour identical
for every width
pair and does not rely on the "narrowing starts mini view at queue-only" resize rule. The values
are also the correct restore state if the user later resizes via F2 instead of `x`.

### D4: `Ctrl+e` stays a plain width toggle

`toggle_pinned_width` already touches no panel-mode state; the spec delta only states this
explicitly so the two keys' division of labour is specified, not accidental.

## Risks

- Expanding to library-only hides the queue column; the existing library-only render path
  (playback strip below the tab panel) already covers this at 80+, and pinned mini-view focus
  rules cover the resting/focused palette. No render changes expected.
- `x` while a blocking overlay is open stays gated by the existing `NoBlockingOverlay` policy
  gate; no eligibility change.
