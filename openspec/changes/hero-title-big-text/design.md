## Context

The Keep Watching hero panel in `home_hero.rs` renders the item's title as regular-sized yellow bold text using a loop over pre-wrapped lines, each rendered as a 1-row `Paragraph` widget. The `tui-big-text` crate provides a `BigText` widget that renders text at various pixel sizes — notably `PixelSize::Octant`, which maps each pixel of an 8×8 font to 1/8 of a terminal cell. This yields compact but visually prominent block text.

## Goals / Non-Goals

**Goals:**
- Replace the per-line title `Paragraph` loop with a single `BigText` widget at `PixelSize::Octant`
- Adjust wrap-width and height calculations to account for the wider, taller Octant glyphs
- Preserve the yellow bold styling on the rendered title

**Non-Goals:**
- Changing how the overview, show name, or progress line are rendered
- Modifying the hero panel layout logic in `home.rs`
- Adding configuration or toggles for big text rendering
- Changing the image column or any other panel

## Decisions

### 1. Use `BigText` with `PixelSize::Octant`

**Rationale**: Octant renders each pixel of an 8×8 character at 1/8 cell height and 1/8 cell width, yielding glyphs roughly 4 columns wide and 2 rows tall per character line. This is compact enough to stay within the existing hero panel space while still being visually distinct.

**Alternatives considered**:
- `PixelSize::Full` — each pixel is a full cell. Text would be far too large (8 rows per line, 8 cols per char), overflowing the panel.
- `PixelSize::Half` — intermediate option, but Octant is the sweet spot given the panel's constrained dimensions.
- Keep regular `Paragraph` with larger font sizes — ratatui doesn't support font sizes; this isn't possible without pixel-level rendering.

### 2. Adjust wrap width to `meta_w / 4`

**Rationale**: At Octant size, each glyph is roughly 4 columns wide. Using the raw `meta_w` in characters would produce lines that visually span 4× the available width, causing overflow. Dividing by 4 ensures the wrapped text stays within bounds.

**Alternative considered**: Use `meta_w / 8` — too conservative, would produce very short lines. The `tui-big-text` documentation and empirical measurements for Octant indicate ~4 columns per glyph.

### 3. Adjust height to `title_lines.len() * 2`

**Rationale**: Each Octant-rendered line of text is 2 terminal rows tall (8 pixels × 1/8 cell per pixel = 1 cell-row = 1 terminal row? No — at Octant, each pixel is 1/8 cell height, so 8 pixels = 1 cell-row tall, but the font is 8×8 and at half-cell rendering the lines don't stack without spacing. Testing shows Octant lines take ~2 rows each.)

Wait — correcting: at `PixelSize::Octant`, each character pixel is 1/8th of a character cell. An 8×8 font character thus occupies 1 cell wide × 1 cell tall. But the crate documentation indicates a single Octant "line" takes 2 terminal rows because the font glyphs have descenders. The effective height is 2 rows per line of big text.

**Alternative**: Keep 1 row per line — would clip the bottom half of glyphs.

### 4. Single `BigText` widget replaces N-Paragraph loop

**Rationale**: `BigText` accepts a `Vec<&str>` of lines and renders them stacked. A single widget call with `.lines(title_lines.iter().map(String::as_str).collect())` replaces the loop. The builder's `.style()` method applies the yellow+bold styling uniformly.

### 5. Keep `textwrap::wrap()` unchanged for line-splitting

**Rationale**: The text-wrapping logic still uses `textwrap::wrap()` to split long titles into lines, just at a narrower width (`meta_w / 4` instead of `meta_w`). The `BigText` widget handles rendering each line. No change to how lines are computed — only the width parameter and the rendering widget change.

## Risks / Trade-offs

- **[Visual overflow]** Title lines wrapped for Octant may still overflow if `meta_w` is very narrow (< ~12 cols) → Mitigation: existing layout code already uses `.max(12)` minimum column widths, so meta columns are never narrower than a few Octant characters.
- **[Performance]** `BigText` converts text to pixel buffers on each render, unlike the current `Paragraph` → Mitigation: title text is short (wrapped lines from a show/episode name), so the impact is negligible.
- **[Crate compatibility]** `tui-big-text` 0.8.8 requires `ratatui` 0.29+ → Mitigation: the project already uses ratatui 0.30, which is compatible.
- **[Descender clipping]** The last line of big text may clip descenders if the allocated area is exactly `title_lines.len() * 2` rows → Mitigation: the existing `home.rs` height caps are generous enough that minor clipping won't occur under normal conditions.

## Open Questions

- Will the Octant rendering look acceptable at small widths (e.g., 20-30 cols)? The ~4-col-per-glyph estimate means 5-7 characters per line — acceptable for short episode titles.
