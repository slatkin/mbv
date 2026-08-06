## 1. State

- [x] 1.1 Add `PanelMode { Both, LibraryOnly, QueueOnly }` next to `PanelFocus` in `src/app/types_settings.rs`, re-exported from the app module.
- [x] 1.2 Replace `queue_column_collapsed: bool` with `panel_mode: PanelMode` in `src/app/app_struct.rs`.
- [x] 1.3 Construct `panel_mode: PanelMode::Both` in `src/app/construct.rs` and the test constructor in `src/app/tests.rs`.

## 2. Command and dispatch

- [x] 2.1 Rename `Command::TogglePowerSidebar` to `Command::CyclePanelMode` in `src/app/action.rs`.
- [x] 2.2 Dispatch: advance `Both -> LibraryOnly -> QueueOnly -> Both`, then apply the focus rule (LibraryOnly forces Library, QueueOnly forces Queue, Both leaves focus alone).
- [x] 2.3 Rename the resolver binding and handler (`input_resolver.rs`, `input_lib_power_keys.rs`) to `panel_mode_cycle_x` / `handle_key_panel_mode_cycle`, keeping the `x` key and the context-menu guard.

## 3. Layout

- [x] 3.1 Update the six `queue_column_collapsed` branches in `src/app/render/mod.rs` to match on `panel_mode` (lines 275, 286, 303, 394, 429, 470).
- [x] 3.2 Build the `QueueOnly` queue area from a full-window left-content rect, reusing the card/queue/visualizer path; the right column becomes `Rect::default()`.
- [x] 3.3 Guard the right-column renderers (tabs, player, status) against zero-dimension areas in `QueueOnly`, and the left renderers against zero-dimension areas in `LibraryOnly`.

## 4. Input guards

- [x] 4.1 `src/app/input_queue_keys.rs:23`: deactivate column resize when `panel_mode != PanelMode::Both`.
- [x] 4.2 `src/app/input_queue_keys.rs:71`: gate Alt+Left return-to-queue on `panel_mode == PanelMode::Both`.

## 5. Tests

- [x] 5.1 Update `input_resolver_handle_key_tests.rs`: cycle test through all three states asserting `panel_mode` and `panel_focus`; focus tests for each state; keep the context-menu and `h`-does-not-toggle guards.
- [x] 5.2 Update `input_power_movie_detail_tests.rs` field writes to `panel_mode`.
- [x] 5.3 Update `render/tests_queue.rs` and any other constructor/field sites.
- [x] 5.4 Add a render test for `QueueOnly`: queue area spans the full width, right column is defaulted.
- [x] 5.5 Add tests that resize keys and Alt+Left are inert in `LibraryOnly` and `QueueOnly`.

## 6. Verify

- [x] 6.1 `cargo check -p mbv-core` and `cargo test -p mbv-core` pass.
- [x] 6.2 `cargo clippy --workspace --all-targets` passes.
- [x] 6.3 Manual check in a terminal: cycle `x` through all three states from both Home and a library tab, resize and reset the queue column in `Both`, confirm nothing renders in the hidden column.
