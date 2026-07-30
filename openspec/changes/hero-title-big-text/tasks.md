## 1. Add Dependency

- [ ] 1.1 Add `tui-big-text = "0.8.8"` to `Cargo.toml` dependencies

## 2. Adjust Title Layout Calculations

- [ ] 2.1 In `keep_watching_hero_layout`, change wrap width from `text_w` to `text_w / 4` for `title_lines`
- [ ] 2.2 Change title height contribution from `title_lines.len() as u16` to `title_lines.len() as u16 * 2`

## 3. Replace Title Rendering with BigText

- [ ] 3.1 Add `use tui_big_text::{BigText, PixelSize};` import to `home_hero.rs`
- [ ] 3.2 Replace the for-loop over `layout.title_lines` (lines ~155-174) with a single `BigText::builder().pixel_size(PixelSize::Octant).lines(...).style(...).build()` rendered via `f.render_widget(big_text, area)`
- [ ] 3.3 Remove all `title_lines` references from the loop-based rendering code

## 4. Verify

- [ ] 4.1 Run `cargo check` to confirm compilation
- [ ] 4.2 Run the app and verify the hero title renders as large Octant text with yellow bold styling
