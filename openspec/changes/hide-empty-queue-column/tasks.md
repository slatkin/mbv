# Tasks

## 1. Derive the hidden column

- [x] 1.1 Add `App::queue_column_hidden_empty()` in `src/app/state/panel_focus.rs`. It is true when `!is_mini_view()`, `panel_mode == PanelMode::Both`, and `displayed_queue().slots().is_empty()`. In the non-mini branch of `effective_panel_mode`, return `PanelMode::LibraryOnly` when it holds, and return `PanelFocus::Library` from `effective_panel_focus` likewise (design D1, D2). Do not write `panel_mode`. Verify with `cargo check -p mbv`.
- [x] 1.2 In `Model::sync_queue` (`src/app/shell/queue.rs`), before `mount_and_focus_queue`, set `self.app.panel_focus = PanelFocus::Library` when `queue_column_hidden_empty()` holds and `panel_focus == PanelFocus::Queue`. Assign the field directly: `set_panel_focus` would run the queue initial-item path for no reason. Verify with `cargo check -p mbv`.
- [x] 1.3 Add app tests in `src/app/tests/` next to the existing panel-mode tests, one behavior each. Contract: spec *Two-panel layout hides an empty queue column* and *Hidden empty queue moves focus to the library*. Cases: (a) `Both` + empty displayed queue -> `effective_panel_mode() == LibraryOnly` while `panel_mode` stays `Both`; (b) `x` from that state -> stored `panel_mode == QueueOnly`; (c) the queue had focus, then emptied and synced -> `panel_focus == Library`, and it stays `Library` after a slot is adopted. Verify with `cargo nextest run -p mbv`.

## 2. Fix existing tests and verify

- [ ] 2.1 Run `cargo nextest run -p mbv`. A failing test that sets up `Both` with an empty queue and needs the queue column gets one seeded queue item. A test that only asserts raw pane geometry is deleted. Verify that the full `mbv` nextest run passes.
- [ ] 2.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`, and verify that both are clean.

## Workflow follow-up

- Manual check by the user: in two-panel view, clear the queue; the library goes full width with the playback strip; add a track and the queue column returns with focus left on the library.
- Sync the `panel-mode` delta and archive the change.
