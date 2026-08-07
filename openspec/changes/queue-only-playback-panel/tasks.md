## 1. Extend card image return type and alignment

- [ ] 1.1 Add `last_card_width: u16` field to `App` alongside `last_card_height`. Reset it in every place `last_card_height` is reset (grep for `last_card_height = 0`).
- [ ] 1.2 Add `left_align: bool` parameter to `render_card_image` in `src/app/render/card.rs`. When `true`, set `img_x = area.x`; when `false`, keep the existing centering formula. Change return type from `(u16, bool)` to `(u16, u16, bool)` — `(height, width, image_loading)`. Return `actual.width` on successful render, `area.width` for placeholder/loading. Store width in `self.last_card_width` alongside height.
- [ ] 1.3 Update `render_power_card` return type to `(u16, u16, bool)` and propagate the width from `render_card_image` and `render_power_card_placeholder`. Placeholder returns `area.width` as its width.
- [ ] 1.4 Update all call sites of `render_power_card` to destructure the new 3-tuple. The existing call in `render_main` line ~409 currently does `let (card_h, _) = self.render_power_card(...)` — change to `let (card_h, card_w, _) = ...`. Pass `left_align: false` at existing call sites.
- [ ] 1.5 Update card.rs tests (`render_power_card` test helper at line ~249) to match the new return type.
- [ ] 1.6 Verify: `cargo check -p mbv-core && cargo check -p mbv` compiles. `cargo test -p mbv` passes (card tests use the new tuple).

## 2. Queue-only playback panel rendering

- [ ] 2.1 In `render_main` in `src/app/render/mod.rs`, inside the queue-only branch (the `else` arm starting around line 406 where `panel_mode != LibraryOnly`), after calling `render_power_card`, add the queue-only playback panel logic. Determine `is_wide = left_area.width >= 100`. In wide mode, pass `left_align: true` to `render_power_card`; in narrow mode pass `false`.
- [ ] 2.2 **Narrow path** (`!is_wide`): after the card renders, compute a playback panel area at `y = left_content.y + card_h`, full `left_content.width`, height `player_h`. Call `self.render_player_panel(f, panel_area, playback, player_h, show_controls, now_playing_title, palette::DARK_BG)`. Subtract `player_h` from `left_remaining` before computing the queue area.
- [ ] 2.3 **Wide path** (`is_wide`): after the card renders, compute a playback panel area at `x = card_area.x + card_w + 2`, `y = card_area.y`, width `left_content.width.saturating_sub(card_w + 2)`, height `card_h`. Paint a `DARK_BG` block over the full panel area first, then call `self.render_player_panel` into it. `left_remaining` subtracts `card_h` as before (no extra rows consumed).
- [ ] 2.4 Gate the queue-only playback panel render on `self.panel_mode == PanelMode::QueueOnly` so it only runs in that mode. The existing `right_visible && player_h > 0` path for the right-column playback panel stays unchanged.
- [ ] 2.5 Verify: `cargo check -p mbv` compiles. `cargo clippy --workspace --all-targets` clean.

## 3. Validation

- [ ] 3.1 Run `cargo test -p mbv` — all existing tests pass with the new return types and layout changes.
- [ ] 3.2 Run `make check-code-file-lines` — no file exceeds the 800-line cap.
