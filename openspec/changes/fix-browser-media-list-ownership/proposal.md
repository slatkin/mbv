## Why

Movies and the Emby homevideos feed view still duplicate list position and hit geometry between the shell (`BrowseLevel` resting state), the `BrowserComponent` parent cursor/scroll mirror, and the two persistent embedded controls (`WideMediaList`, `InlineMediaBrowser`). This violates the existing canonical-media-list ownership contract and prevents umbrella rows 4.1-4.3 of `openspec/changes/finish-canonical-media-list-ownership` from closing.

## What Changes

- Make the persistent active `WideMediaList<usize>` or `InlineMediaBrowser<usize>` the sole live owner of Browser list position for Movies and the Emby homevideos feed view (including the feed-group picker) at Wide and Normal/Narrow.
- Delete the `BrowserComponent` parent cursor/scroll fields and every write-back into them (movement, `apply_position`, `set_content`, `view()` tail, `claim_list_point`).
- Keep shell resting/restore state only, written from component-resolved values per the AGENTS.md resolved-value rule; never a render seed except explicit re-anchor. Identity-gated push seeds become explicit re-anchor.
- Switch `resolve_row_target` to point-only `resolve_current_point` / `current_selected_target` plus `current_detail_rect` against retained control geometry; remove `resolve_left_cursor` map reads on canonical paths. Canonical paints stop publishing `left_row_map` / `left_item_rows` / `left_sorted_indices`; compatibility shims stay for destinations still using them.
- Drop parent-field anchor fallbacks: `ViewportAnchor` transfer is control-to-control only.
- Align wheel/click with the Home repair pattern (control moves, shell keeps focus plus nav-effect side calls).
- Keep non-hero two-column Emby catalogs isolated: screen-owned grid interaction with arrangement-owned placement; canonical controls neither read nor overwrite grid state, with tests recording that evidence (row 4.2).
- Add Wide AND Normal/Narrow `Application::tick()` integration coverage plus focused component/media-list tests proving control-owned movement, one-painter ownership, two-column isolation, retained-geometry hits, and `ViewportAnchor` round-trips.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is an ownership refactor that conforms to the existing canonical-media-list and interactive-component-framework requirements without changing observable requirements.

## Impact

- Affected area: Browser interactive component (`BrowserComponent`, its navigation/keyboard/paint modules), its shell projection (`shell_browser.rs`, click-gesture arms, `shell_run.rs` cursor read), and its Wide/Inline render seam (`paint.rs`, `list_narrow.rs`) plus tests.
- No API, dependency, persistence-format, provider-behavior, or unrelated-destination change. TV Series waits on this family's ownership decisions; its Normal presentation builds on `BrowserComponent`.
