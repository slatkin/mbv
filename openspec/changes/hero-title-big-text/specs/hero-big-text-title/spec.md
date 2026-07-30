## ADDED Requirements

### Requirement: Hero title renders as large Octant text
The hero panel's item title SHALL be rendered using the `tui-big-text` crate's `BigText` widget with `PixelSize::Octant` instead of per-line `Paragraph` widgets.

#### Scenario: Title renders with BigText widget
- **WHEN** the Keep Watching hero panel is rendered for an item with a name
- **THEN** the title SHALL be rendered as a single `BigText` widget built with `PixelSize::Octant`, the pre-wrapped title lines, and a style with `palette::YELLOW` foreground and `Modifier::BOLD`

### Requirement: Title wrap width accounts for Octant glyph width
The wrap width used to split the title into lines SHALL be `meta_w / 4` instead of `meta_w` to accommodate the wider Octant glyphs (~4 columns per character).

#### Scenario: Wrap width divides by 4
- **WHEN** computing `title_lines` for a meta column width of `meta_w` columns
- **THEN** `textwrap::wrap()` SHALL be called with `meta_w / 4` as the width parameter

### Requirement: Title height accounts for Octant line height
The height contribution of the title lines SHALL be `title_lines.len() * 2` rows instead of `title_lines.len()` rows, because each Octant line occupies approximately 2 terminal rows.

#### Scenario: Height is 2 rows per line
- **WHEN** computing `height` in `KeepWatchingHeroLayout`
- **THEN** the title portion SHALL contribute `title_lines.len() as u16 * 2` to the total height

### Requirement: Title style preserves yellow bold appearance
The `BigText` widget's style SHALL use `Color::Yellow` foreground (or the `palette::YELLOW` equivalent) with `Modifier::BOLD`.

#### Scenario: Yellow bold style applied
- **WHEN** the `BigText` widget is built
- **THEN** `.style()` SHALL be called with a `Style` that has `fg: palette::YELLOW` and `add_modifier(Modifier::BOLD)`
