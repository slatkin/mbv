# Divider status indicators

The tab-bar divider line (`gap_area` in `render()`) renders right-aligned bracketed indicators: `[字]` subtitle, `[↯]` remote-control, `[>]/[||]/[ ]` playback. Code is in `render/mod.rs` just below the `// Thin underline below tab row` comment.

To add a new indicator:

1. Compute `(text: &str, color: Color)` from whatever app state you need.
2. Add it to the `ind_w` sum in `dash_count`:
   ```rust
   let dash_count = gap_area.width.saturating_sub(ind_w(new_text) + ind_w(rc_text) + ...) as usize;
   ```
3. Insert the bracket/glyph/bracket spans in order (left-to-right = left-of-existing to right):
   ```rust
   Span::styled("[", bracket),
   Span::styled(new_text, Style::default().fg(new_color).add_modifier(Modifier::BOLD)),
   Span::styled("]", bracket),
   Span::styled("─", dash_style),
   ```

Rules:
- `ind_w(text)` = `1 + text.width() + 1 + 1` (`[` + display-width + `]` + `─`). Use `.width()` (`UnicodeWidthStr`), not `.chars().count()` — CJK and some nerd font glyphs are double-width.
- Brackets use the `bracket` style (white); the trailing `─` uses `dash_style` (muted). Never combine into one span or the dash turns white.
- Nerd font glyphs go behind `if self.use_nerd_fonts { ... } else { ascii_fallback }`.
- `dash_count` dashes fill remaining width; total of all `ind_w` values plus `dash_count` must equal `gap_area.width`.
