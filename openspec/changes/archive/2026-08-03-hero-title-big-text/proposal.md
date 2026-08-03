## Why

The hero/"Keep Watching" panel title is currently rendered as regular-sized yellow bold text, which looks generic and underuses the panel's visual potential. Rendering it as large Octant-sized block text will give the hero panel more visual impact and presence, making the featured item feel more prominent.

## What Changes

- Add `tui-big-text = "0.8.8"` as a dependency
- Replace the per-line `Paragraph` loop that renders the title with a single `BigText` widget using `PixelSize::Octant`
- Adjust the title-wrap width calculation in `keep_watching_hero_layout` to account for wider Octant glyphs (divide by 4 instead of using raw character width)
- Adjust the title height calculation from `title_lines.len()` (1 row per line) to `title_lines.len() * 2` (2 rows per Octant line)

## Capabilities

### New Capabilities
- `hero-big-text-title`: The hero panel's item title is rendered as large Octant-sized text via the `tui-big-text` crate instead of regular ratatui `Paragraph` text.

### Modified Capabilities
<!-- None: no existing specs are being modified -->

## Impact

- **Dependencies**: New `tui-big-text = "0.8.8"` in `Cargo.toml`
- **Code**: `src/app/render/home_hero.rs` — `keep_watching_hero_layout` (wrap width and height) and `render_keep_watching_hero_meta` (title rendering loop)
- **Behavior**: Title appears as large block text; title area takes roughly twice the vertical space (2 rows per Octant line); no changes to layout in `home.rs` — existing height caps handle overflow
