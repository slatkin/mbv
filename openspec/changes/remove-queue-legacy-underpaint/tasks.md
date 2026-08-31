## 1. Prove the defect and re-home the geometry

- [x] 1.1 Regression test, red now: with two or more queue items, one arrow
      press leaves exactly one highlighted row. Assert on the `TestBackend`
      buffer through `Model::draw_frame` — **not** `compose_base_frame` alone,
      since the ghost only exists when both painters run. Verify:
      `rtk cargo nextest run -p mbv`, test red, stated.
- [x] 1.2 Publish `layout.main.queue_area` from `render_main`
      (`render/screens/root.rs`) instead of from inside the painter, preserving
      the existing "no publish when `height < 1`" semantics. Verify: component
      placement (`shell_queue.rs:36/47/60/73`), page-size (`actions.rs:127`),
      and the context-menu anchor (`shell_overlays_menus.rs:90/112`) still see
      the same rect — assert it in a test rather than by inspection.
- [x] 1.3 Add `QueueComponent::selected_row_rect() -> Option<Rect>`, derived
      from its own `geometry.rows` and `cursor`, and publish it into
      `layout.main.queue_selected_item_rect` from `render_queue_component`
      (`shell_queue.rs:53`) after the `application.view(..)` call — mirroring
      `render_music_workspace_component` (`shell_music_workspace.rs:175-182`).
      Verify: the **keyboard** context menu on a queue row still anchors to the
      selected row (`ContextMenuAnchor::SelectedItem(PanelFocus::Queue)`); this
      path is not mouse and is not covered by D16.

## 2. Delete the duplicate painter

- [x] 2.1 Delete the legacy body call at `render/screens/root.rs:506`. Verify:
      1.1's regression test goes green; the queue still paints, via the
      component only.
- [x] 2.2 Delete `render_queue` (`render/screens/queue.rs:277-566`) and its
      test-only helpers `render_queue_rows` / `render_queue_cursor_row`, plus
      the three tests that exercise only the deleted painter
      (`render_queue_scroll_up_reaches_top_without_regressing`,
      `render_queue_page_up_from_bottom_reaches_top`,
      `render_queue_viewport_does_not_leak_between_app_instances`) — confirm
      first that `QueueComponent` has equivalent coverage for each, and add it
      where it does not. Verify: `rtk cargo nextest run -p mbv`;
      `screens/queue.rs` drops from 757 to roughly 430 lines.
- [ ] 2.3 Delete the orphans this creates, and only those: `queue_row_map`
      (`layout.rs:112`, write-only — confirm with `rtk grep` before removing),
      and `build_queue_rows` + the `QueueRow` enum + their tests
      (`ui_util.rs:236-327`). Verify: `rtk cargo clippy --workspace
      --all-targets` reports no dead code.

## 3. Record and confirm

- [ ] 3.1 Update `docs/architecture/interactive-surface-ledger.md:63` to state
      the Queue body has no legacy underpaint, matching the wording already used
      for Playback (row 62) and Feeds (row 72). Verify: the row names the owner
      and the painter, consistent with #625's per-breakpoint column.
- [ ] 3.2 Add a conformance test in the shape of the existing
      Feeds/Playback rows in `tests_conformance_matrix.rs`:
      `compose_base_frame` reserves `queue_area` and paints no slot rows, and
      `QueueComponent` is the sole row painter. Verify: it fails if the legacy
      call is reinstated.
- [ ] 3.3 Confirm #623's repro by hand: arrow through a multi-item queue and
      see no ghost row. Verify: stated, with the #623 comment updated.
- [ ] 3.4 Full gate: `rtk cargo check -p mbv`,
      `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk ast-grep scan`, `rtk cargo fmt`,
      `rtk make check-code-file-lines`.
