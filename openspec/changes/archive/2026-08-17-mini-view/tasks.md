## 1. State

- [x] 1.1 Add `MINI_VIEW_THRESHOLD: u16 = 80` constant in `src/app/mod.rs`,
      alongside the existing `TWO_COLUMN_THRESHOLD`.
- [x] 1.2 Add `mini_view_focus: PanelFocus` field to `App`
      (`app_struct.rs`), defaulted to `PanelFocus::Library` in
      `construct.rs`, not read from or written to prefs.
- [x] 1.3 Add `effective_panel_mode()` and `effective_panel_focus()`
      helper methods that branch on `terminal_width < MINI_VIEW_THRESHOLD`
      per design.md's Decisions section.

## 2. Input

- [x] 2.1 In `Command::CyclePanelMode` (`action.rs`), branch at the top on
      `terminal_width < MINI_VIEW_THRESHOLD`: narrow toggles
      `mini_view_focus` (Library ⇄ Queue) and calls
      `focus_queue_initial_item()` when moving to Queue; wide keeps the
      existing unchanged 3-state match.
- [x] 2.2 Update `handle_key_view_dispatch`'s routing match
      (`input.rs:171-174`) to read `effective_panel_focus()` instead of
      `self.panel_focus`.
- [x] 2.3 Audit `input_mouse_panels.rs` for direct reads of `panel_mode` /
      `panel_focus` in click-region hit-testing; route through the
      effective helpers wherever the read should reflect mini view.

## 3. Rendering

- [x] 3.1 Update `render/mod.rs`'s layout computation (`left_w`,
      `right_w`, `left_area`, `lib_area`/`queue_area`, etc.) to read
      `effective_panel_mode()` instead of `self.panel_mode`.
- [x] 3.2 Remove the `panel_mode != PanelMode::QueueOnly` guard from the
      `queue_focused` computation (`render/mod.rs:319-321`) so queue-only
      always reflects real focus; update the surrounding comment.

## 4. Tests

- [x] 4.1 Update/add tests in `input_resolver_handle_key_tests.rs`
      covering: narrow-width `x` toggles library-only ⇄ queue-only only
      (never `Both`); toggling moves focus with the panel; widening back
      to 80+ restores the prior `panel_mode`/`panel_focus` unchanged.
- [x] 4.2 Update `render/tests_queue.rs` (and any other test asserting the
      old unfocused queue-only styling) to expect focused styling when the
      queue holds focus, at both narrow and wide widths.
- [x] 4.3 Add a render test confirming mini view starts at library-only by
      default (fresh app, narrow terminal, no prior interaction).

## 5. Docs

- [x] 5.1 Update `openspec/specs/panel-mode/spec.md` is handled by
      archiving this change (do not hand-edit); confirm no other doc
      (README, help overlay text) references the old queue-only unfocused
      behavior or needs a mini-view mention.
