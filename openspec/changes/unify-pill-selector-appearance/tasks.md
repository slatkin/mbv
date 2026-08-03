## 1. Shared Pill-Selector Presentation

- [x] 1.1 Replace Home-specific pill palette names with context-neutral pill-selector row, selected, unselected, and overflow tokens in `src/app/palette.rs`.
- [x] 1.2 Make `render_pill_bar` own the canonical Home-style joined edges, spacing, row background, selection styles, overflow indicators, width calculations, and hitbox geometry.
- [x] 1.3 Remove `render_home_pill_bar`, the Home-only style callback, and caller-selectable appearance variants from the shared pill-bar API.

## 2. Selector Adoption

- [x] 2.1 Update Home sections, feed groups, music groups, and letter filters to use the sole shared pill-bar renderer without changing their selected positions or target IDs.
- [x] 2.2 Render the series `Series:` prefix separately and delegate season choices to the shared pill bar, preserving season selection and keeping the selected season visible on overflow.
- [x] 2.3 Leave DirectRemote Local/Remote queue choices on the status-pill path: they double as connection status (device name, icon, "Connected:" label) with no alternate display, so the scope pills are not treated as selectors and stay out of the shared pill bar.
- [x] 2.4 Keep primary navigation tabs, attached-session indicators, connection indicators, and other non-interactive status pills on their existing independent render paths.

## 3. Verification

- [x] 3.1 Update existing Home and scrolling-pill render assertions for the canonical appearance and geometry; series-detail and queue-scope assertions confirmed unchanged (they don't assert the shared pill shell).
- [x] 3.2 Verify narrow rows keep the selected pill visible, preserve caller-defined target IDs, and produce hitboxes aligned with all rendered edge glyphs.
- [x] 3.3 Run `cargo fmt --all -- --check` and targeted pill-selector render/input tests.
- [x] 3.4 Run `cargo test` and resolve regressions introduced by the presentation refactor.
