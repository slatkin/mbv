# Design

## Context

`App::panel_appearance_focus` (`src/app/state/panel_focus.rs`) is `!is_mini_view() && effective_panel_focus() == panel`, so mini view always paints the resting palette. pinwin reports GTK focus enter and leave to the pty as terminal focus reports; mbv enables them (`infra/terminal.rs`) and `shell.rs` routes them to `note_focus_gained` and `note_focus_lost`, which today only drive `refocus_at`, the first-click suppression timer. pinwin sends nothing at launch, so a launch with no change yields no report. `App.pinned_panel: Option<PinnedPanel>` marks a pinned process.

The existing `panel-mode` requirement "Queue-only renders the queue panel focused" says "at any terminal width", which the code already contradicts in mini view; the delta narrows it.

## Goals / Non-Goals

**Goals:**
- Pinned mini view shows focused or resting palette according to window focus.

**Non-Goals:**
- Any change to wide view or to non-pinned mini view.
- Any pinwin change; the accent stroke (`pinned-panel-focus-accent`) is separate.

## Decisions

**D1: A new `window_focused: bool` on `App`, default `true`.** Set by `note_focus_gained` and `note_focus_lost`. Alternative: derive from `refocus_at`; rejected because it is a click-suppression timer whose `None` also means "never reported", and reusing it would couple two jobs. Default `true` because the pinned app launches with focus and pinwin reports only changes.

**D2: Gate on `pinned_panel.is_some()`, in `panel_appearance_focus`.** The new rule is: in mini view the result is `pinned && window_focused && effective_panel_focus() == panel`; wide view is unchanged. `panel_appearance_focus` decides the shell-side appearance bit, and every displayed-panel component is responsible for composing that projected bit into its own row paint policy (queue: `QueueComponent` policy; library: `SkeletonPaintState` via `set_frame_focused`) — a component that re-derives suppression itself can drift from the shell bit, as the queue and library rows each did before their mini-view suppression was folded into the composed bit. Pinned-only is an explicit user decision, not an oversight; unpinned mini view is left as it was.

**D3: Surfaces repaint on the focus event.** Focus reports arrive as ordinary events, so the existing event-driven redraw covers the change. Confirm during implementation that a focus event schedules a draw; if it does not, mark the frame dirty in the two handlers.

## Risks / Trade-offs

- [A pinned panel launched unfocused paints focused until the first focus report] → accepted; the user states pinned launches take focus.
- [Terminal focus reports require mode 1004] → mbv already enables it unconditionally.
