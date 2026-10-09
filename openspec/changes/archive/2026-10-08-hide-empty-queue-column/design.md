# Design

## Context

`App::effective_panel_mode` (`src/app/state/panel_focus.rs`) already turns the stored `panel_mode` into the mode in effect for each frame, for example mini view below 80 columns. Every layout, input-policy, hit-test, and painter reader goes through it, not through the raw `panel_mode`. Those readers are `shell/draw.rs` (`chrome_geometry`), `shell/arbitration.rs` (the key-policy snapshot, which gates `QueueColumnWidth` and Alt+Left), `shell/library.rs`, `shell/library_panel.rs`, `shell/queue.rs`, and `state/projection/tv_wide.rs`. Library-only rendering (the Library playback panel, full-width library, no boundary) already follows from an effective `LibraryOnly`.

## Goals / Non-Goals

**Goals:** reuse the existing library-only rendering with no new layout code and no new mode.

**Non-Goals:** changing the empty placeholder in queue-only or mini view; changing pinned layouts; animation or transition effects.

## Decisions

### D1: Derive the hide in `effective_panel_mode`

At 80+ columns, `effective_panel_mode` returns `LibraryOnly` when `panel_mode == Both` and `displayed_queue().slots().is_empty()`. The stored `panel_mode` is not written, so the `x` cycle (`cycle_wide_panel_mode`, which reads the stored mode) still advances `both -> queue-only`.

*Alternative rejected:* a new `PanelMode` variant or a new `chrome_geometry` input. Either one duplicates library-only geometry that already exists and adds a variant that every exhaustive match must handle.

"Displayed queue" means the viewed scope (`viewed_queue_scope`). An empty Remote queue hides the column the same way an empty Local queue does.

### D2: Focus follows from the derived mode, and the stored focus is written in one place

`effective_panel_focus` returns `Library` while D1's hide holds, so input routing never targets a queue that has no placement. The stored `panel_focus` is also moved to `Library` in the shell's queue sync (`Model::sync_queue`, `src/app/shell/queue.rs`), and only when the hide holds and `panel_focus == Queue`. Without that write, the stored `Queue` focus would come back when the queue refills, and focus would jump away from the library the user was browsing (spec: *Focus stays on the library on refill*). `sync_queue` already runs on every queue change, so the hide is never seen without that write.

*Alternative rejected:* deriving focus only. It is simpler, but it breaks the refill scenario.

## Risks / Trade-offs

- [Existing app tests set up `Both` with an empty queue and expect a queue placement or queue focus] → Seed such tests with one queue item. A test that only asserts raw geometry is deleted, not reworked (repo test policy).
- [The library width changes when the first item is added or the last is removed] → Accepted. This is the requested behavior. Library components stay mounted, so their cursor and scroll survive.
