# Design

## Context

See `proposal.md` (Why). Today the Wide Library panel splits the whole content
area into Hero + Browser panes (`wide_hero_split`), and the pill reserve is
carved *inside* the Browser pane by `wide_hero_browser_pane`, so the Selector row
is browser-pane-width. The same `wide_hero_browser_pane(area, area)` call does
the Narrow pill reserve over the single pane.

Two facts constrain the approach:

- `wide_hero_hero_pane` (`wide_hero.rs`) recomputes `wide_hero_presentation` from
  the **raw `area`** it is handed by `render_wide_skeleton` — it does not receive
  the panes from `wide_library_panes`. Any top-band reserve must reach it too, or
  the hero pane paints over the pill row.
- `wide_hero_browser_pane`'s pill reserve is shared with Narrow, which is already
  full-width and must keep it. So the Wide path must stop taking that internal
  reserve without changing the Narrow path.

## Goals / Non-Goals

Goals:
- One full-width Selector band above both Wide panes, reserved before the split.
- The split drag never touches the pill/spacer rows.
- Remove the List controls row and its plumbing with no residual dead code
  (except `is_home_video_view`, which has other callers).

Non-Goals:
- Narrow layout changes (already full-width).
- Touching the split gesture math itself (`split.rs` / `split_hit`) beyond the
  gap rect's vertical extent.
- Deltas to sibling specs' positional wording (see proposal Impact).
- Coordinating with `sticky-wide-hero-split` — independent per the handoff.

## Decisions

### D1: Reserve the top band in `wide_library_panes`, split the remainder

`wide_library_panes` carves the top `WIDE_HERO_PILLS_ROW_HEIGHT +
WIDE_HERO_PILLS_GAP_ROWS` rows off `area` (full width), returns that band, and
calls `wide_hero_presentation` on the reduced `content_area`. `wide_hero_split`
and `wide_hero_presentation` stay pure — they split whatever area they are given.

- **Alternative — bake the reserve into `wide_hero_presentation`:** rejected. It
  is shared with `images.rs` and is the generic two-pane primitive; the
  full-width pill band is a *library-panel* concern, so it belongs in the
  library-only wrapper.

### D2: Feed the reduced content area to the hero-pane painter

`render_wide_skeleton` must pass the reduced `content_area` (not raw `area`) to
`wide_hero_hero_pane`, so the hero pane starts below the band. Cleanest is to
have `wide_library_panes` expose the `content_area` it split, and thread that to
both the browser-pane placement and `wide_hero_hero_pane`.

- **Alternative — recompute the reserve inside `wide_hero_hero_pane`:** rejected;
  it would duplicate the band arithmetic in a second place and re-introduce the
  drift the shared primitive exists to prevent.

### D3: Wide pill placement moves out of `wide_hero_browser_pane`

In the Wide path, paint the full-width pill bar + spacer directly over the
reserved band, and give the Browser pane its list box with **no internal pill
reserve** — the browser pane's content now starts at its own top (which is
already below the band). Narrow keeps `wide_hero_browser_pane(area, area)`
unchanged.

- The Wide `paint_browser_pane` currently takes a `WideHeroBrowserPane` that
  bundles `pills_area`/`spacer_area`/`list_panel`. The Wide caller will supply
  the full-width `pills_area`/`spacer_area` (from the reserved band) and the
  browser pane's full rect as `list_panel`. This keeps one browser-pane painter;
  only the rects it is fed change.

### D4: Split-gap rect starts at the content band (corrects the handoff)

`panel_view.rs` builds the drag-handle gap as `y: area.y, height: area.height`.
That would extend the drag target up through the pill band in the gutter columns,
violating "the band is independent of the drag." Change it to the content band's
vertical extent: `y: geometry.hero.y, height: geometry.hero.height` (the hero and
browser panes share the same y/height post-split).

### D5: Left alignment and spacer surface (defaults, no user decision needed)

The full-width pills start at the panel's left content edge (the shared
`PANE_PAD_X` inset), so they read as one band aligned with the hero content. The
spacer row keeps the `PillRowGap` surface, now spanning the full width.

### D6: List controls row removal

Delete `ListControls`, the `controls` field on `LibraryPanelContent`,
`paint_list_controls_row`, `SkeletonHits.controls`, `BrowserPaneGeometry.controls`,
`WideSkeletonGeometry.controls`, and the Wide `list_panel`/`controls_area` split.
Unwind the `home_video` field/push in `emby_library_content.rs` and its
computation/push in `shell_emby_library_content.rs`. **Keep** `is_home_video_view`
(`lib_cursor_actions.rs`) — `music_actions.rs` and `tests_non_music.rs` still use
it. `home_video` (controls label) and `feed_home_video` (feed grouping) are
distinct; do not touch the latter.

## Risks / Trade-offs

- **Full-width search box is now the behavior** → intended (user-confirmed); it
  follows the "search box uses the Selector row's rect" rule with no
  special-casing.
- **`wide_hero_hero_pane` and `wide_library_panes` disagreeing on the band** →
  mitigated by D2: one source (`wide_library_panes`) produces the content area
  both consume.
- **Stale controls-row tests** → `slots.rs::list_controls_row_paints_its_label`
  and any controls assertions in `wide_tests.rs`/`panel_tests.rs`/
  `tests_library_characterization.rs` are deleted, not ported (per repo rule:
  write fresh tests for the new behavior, don't translate old ones).

## Migration Plan

Pure UI/render change, single binary, no persistence or protocol impact. No
rollback strategy beyond reverting the commit. Verify against the legacy Wide
reference: pills full-width, hero two rows down, drag gap below the band,
home-video count gone.
