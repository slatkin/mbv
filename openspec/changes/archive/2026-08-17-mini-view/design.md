## Context

See proposal.md - Why. The existing three-state cycle (`Command::CyclePanelMode`
in `src/app/action.rs`) and its focus-follow logic already exist and are
unchanged at 80+ columns. `app.terminal_width` (`app_struct.rs`) is already
tracked and refreshed every render frame (`render/mod.rs`), and is already
used elsewhere for width-driven decisions (`queue_column_width.rs`,
`input_mouse_panels.rs`), so no new width-tracking plumbing is needed.

## Goals / Non-Goals

**Goals:**
- Below 80 columns, `x` toggles exactly two states (library-only ⇄
  queue-only), landing on library-only by default.
- The toggle carries focus with it, and the visible panel always renders
  with focused styling when it holds focus — closing the queue-only
  "looks unresponsive" gap.
- Zero interference with the existing wide-terminal behavior: crossing the
  80-column boundary in either direction must not mutate `panel_mode` or
  `panel_focus`.

**Non-Goals:**
- Changing the 80+ column three-state cycle itself.
- Persisting mini-view state across restarts.
- Touching the unrelated `TWO_COLUMN_THRESHOLD` (82), which governs the
  library panel's internal list-column layout.

## Decisions

**Derive effective mode/focus at read time, don't mutate stored state.**
`panel_mode` and `panel_focus` stay exactly what the user last set at 80+
columns. Add one new field, `mini_view_focus: PanelFocus` (defaults to
`Library`, not persisted), holding only the narrow-mode toggle state.
Everywhere layout or input routing currently reads `self.panel_mode` /
`self.panel_focus`, it instead reads through a small helper that branches
on `self.terminal_width < MINI_VIEW_THRESHOLD`:

```rust
fn effective_panel_mode(&self) -> PanelMode {
    if self.terminal_width < MINI_VIEW_THRESHOLD {
        match self.mini_view_focus {
            PanelFocus::Library => PanelMode::LibraryOnly,
            PanelFocus::Queue => PanelMode::QueueOnly,
        }
    } else {
        self.panel_mode
    }
}

fn effective_panel_focus(&self) -> PanelFocus {
    if self.terminal_width < MINI_VIEW_THRESHOLD {
        self.mini_view_focus
    } else {
        self.panel_focus
    }
}
```

This reuses the existing `LibraryOnly`/`QueueOnly` rendering paths in
`render/mod.rs` unchanged — mini view is just those two states picked by a
different, narrower selector. No new rendering branch needed. Rejected
alternative: writing mini-view choices directly into `panel_mode`/
`panel_focus` and restoring saved values on widen — more state to save and
restore, and a resize mid-interaction becomes a lossy write instead of a
no-op read.

**`Command::CyclePanelMode` branches once, at the top.** If narrow, toggle
`mini_view_focus` and call `focus_queue_initial_item()` when moving to
`Queue` (mirroring the existing queue-only entry in the wide-mode branch).
If wide, run the existing unchanged 3-state match. The context-menu guard
in `handle_key_panel_mode_cycle` (`input_lib_keys.rs`) already fires before
either branch.

**Drop the queue-only muted-styling special case entirely**, not just for
mini view. `render/mod.rs`'s `queue_focused` computation currently is
`self.panel_mode != PanelMode::QueueOnly && matches!(self.panel_focus,
PanelFocus::Queue)`. The `panel_mode != QueueOnly` guard is the only thing
producing the unfocused look; removing it makes queue-only always reflect
real focus, which happens to also be exactly right for mini view without
any extra casing. Confirmed as in-scope with the user rather than treating
it as a pre-existing, unrelated bug report.

**New constant `MINI_VIEW_THRESHOLD: u16 = 80`** in `src/app/mod.rs`,
alongside but independent of `TWO_COLUMN_THRESHOLD` (82). Kept as a
separate named constant per the user's explicit instruction that these are
unrelated thresholds serving different subsystems, even though the values
are close.

## Risks / Trade-offs

- [Mouse click regions in `input_mouse_panels.rs` may hit-test against
  `panel_mode` directly rather than an effective mode] → Verify during
  implementation that click regions derive from the same
  `effective_panel_mode()`/rendered layout areas, not the raw field; add
  narrow-width coverage if they don't.
- [Any other production call site reads `self.panel_mode`/`self.panel_focus`
  directly instead of the effective helpers, silently ignoring mini view] →
  grep for both fields as part of implementation; route every layout- and
  input-relevant read through the helpers (persistence/prefs code should
  keep reading the raw fields, since those must stay untouched by mini
  view).

## Migration Plan

No data migration. Purely in-memory UI state; existing prefs
(`panel_focus`/`panel_mode` in saved prefs) are unaffected in shape or
meaning.
