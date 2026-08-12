## 1. Helper function

- [x] 1.1 In `src/app/render/overlays/search_modal.rs`, add a private function `fn body_bg(mode: SearchMode) -> Color` that returns `palette::LIBRARY_SIDE_BG` for `SearchMode::Global` and `palette::BG_GREEN` for `SearchMode::Fuzzy`.

## 2. Replace call sites

- [x] 2.1 Replace the `palette::LIBRARY_SIDE_BG` argument passed to `render_modal_frame` with `body_bg(modal.mode)`.
- [x] 2.2 Replace every `palette::LIBRARY_SIDE_BG` passed as the `bg` argument to `render_state_message` with `body_bg(modal.mode)`.
- [x] 2.3 Replace `palette::LIBRARY_SIDE_BG` in the results-area background `Block` (the one that fills `body_area`) with `body_bg(modal.mode)`.
- [x] 2.4 Replace `palette::LIBRARY_SIDE_BG` in the per-row background (`line_bg` for unselected rows) with `body_bg(modal.mode)`.
- [x] 2.5 Replace `palette::LIBRARY_SIDE_BG` in the type-filter gap spans and unselected-chip background with `body_bg(modal.mode)`.

## 3. Verification

- [x] 3.1 Confirm no remaining `palette::LIBRARY_SIDE_BG` reference in the file is a body-fill site (only the helper's own definition should reference it).
- [x] 3.2 Run `rtk cargo check -p mbv-core` and `rtk cargo clippy --workspace --all-targets`.
