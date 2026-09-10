## Why

The library column's surface is a single constant (`SURFACE_BACKDROP`, `#2d353b`) that never follows
panel focus, so the only thing telling you the right panel holds focus is the panel's own fill. The
queue column already does this correctly — its column surface is focus-resolved while its panel is a
separate, lighter green — so the same screen shows two different answers to "how do I show focus".
The screens are not individually wrong: the theme has no role that means "the surface a panel sits
on", so `SURFACE_BACKDROP` is overloaded across six unrelated meanings and three different screens
resolved it three different ways by hand.

## What Changes

Adopt one nesting model for every bounded content area, and express it as roles rather than as a
constant reused by coincidence:

```
column / pane surface   focused #3c4841   resting #2d353b
panel on that surface   focused #48584e   resting #333c43
selected row            = the containing surface   (punch-through)
```

- **The library column surface becomes focus-resolved.** Today `#2d353b` always; it becomes
  `#3c4841` while the right panel holds focus. The queue column already behaves this way and does
  not change.
- **The focused panel colour becomes `#48584e`** (`BG_GREEN_SOFT`) for every panel that resolves
  through the shared panel lever — library rails, Home, Feeds, TV, Music, Audiobookshelf, the
  queue's own panel, the now-playing strip, and the TV/Music workspace boxes. The queue panel, the
  TV episode box, and the Music track box already render `#48584e`; they lose their separate
  `SURFACE_ACCENT_SOFT` role and join the shared panel role.
- **The selected row follows the surface that contains its panel** instead of a fixed backdrop, in
  both the canonical media lists and the remaining legacy list/letter-group renderers. Nothing
  visually changes for an unfocused list, because a list only paints a selected-row background while
  it is focused.
- **The wide hero pane is classified as a pane/column surface, not a panel.** It keeps its current
  focus-green (`#3c4841`) and resting (`#333c43`) values, but stops sharing the panel lever so the
  retarget cannot swallow the recessed content box inside it.
- **Modal frames get their own role** (`SURFACE_DIALOG`, `#3c4841`, unchanged) so the nine modal
  frames (10 direct-name sites across nine dialog modules) stop riding on the panel role and cannot
  be repainted by a panel-colour change.
- **The deep recess keeps its own role.** The inset content box inside a hero pane, the player
  panel's blank row and status pills, and the visualizer background stay `#2d353b` under
  `SURFACE_BACKDROP`, which stops meaning "column" and starts meaning "recess inside a surface".

Not changing: the unfocused panel colour (`#333c43`), the unfocused column colour (`#2d353b`), the
unfocused queue panel (`#2d353b`, already equal to the unfocused column), the tab bar, the status
bar, and every dialog frame's colour.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `ui-design-language`: the capability must name the nested surface levels (column/pane, panel,
  recess, dialog overlay) as roles resolved from the caller's focus state, must require the
  selected-row highlight to derive from the surface containing its panel, and must require a
  panel-colour change to leave dialog frames untouched.

## Impact

- `src/app/render/theme/primitives.rs`, `theme/mod.rs` — role taxonomy and resolvers; `SURFACE_FOCUSED`
  stops meaning "the panel fill", `SURFACE_ACCENT_SOFT` is retired into the panel role,
  `SURFACE_DIALOG` is added, `list_selected_row_bg()` becomes focus-resolved.
- `src/app/palette.rs`, `src/app/render/mod.rs` — re-exports.
- `src/app/render/arrangements/chrome.rs` — `FrameChromeGeometry` gains the right column's focus bit
  (derived from the same `PanelFocus` it already receives); `chrome.rs` column fills.
- `src/app/render/arrangements/wide_hero.rs` — `LeftPaneFocus` moves onto the pane surface role;
  `WideHeroContentBoxSurface::FocusedTrackList` onto the panel role.
- `src/app/render/components/` — `chrome.rs` (column fills), `home.rs` (spacer), `music_wide.rs`
  (browser panel), `tv_wide.rs` and `widgets.rs` (focused boxes), `chrome_player.rs` (pill row),
  `media_list/wide.rs` (`SelectedRowSurface` resolution), `list_rows.rs` (legacy row backgrounds).
- Shell: `src/app/shell_playback.rs` (now-playing strip follows the panel role).
- The nine modal frames' 10 direct-name call sites move to `SURFACE_DIALOG` with no rendered change.
- Tests: ~42 assertions in 12 files reference these roles by name and follow the retarget; the three
  characterization tests that pin a selected row against its panel
  (`tests_library_characterization`, `tests_home_characterization`, `tests_feeds`) and
  `tv_wide_tests` need their expected relationship restated; `wide_hero_pane_characterization`
  proves the pane's values are unchanged.
