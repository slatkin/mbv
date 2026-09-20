# Design

## Context

See `proposal.md` (Why). Today the Wide Library panel splits the whole content
area into Hero + Browser panes (`wide_hero_split`), and the pill reserve is
carved *inside* the Browser pane by `wide_hero_browser_pane`, so the Selector row
is browser-pane-width. The same `wide_hero_browser_pane(area, area)` call does
the Narrow pill reserve over the single pane.

Constraints that shape the approach:

- **The breakpoint predicate is embedded in the split.**
  `wide_hero_presentation` (`wide_hero.rs:65`) internally calls
  `wide_hero_fits(content_area)` and returns `None` when it fails. Callers gate
  on `wide_hero_fits(raw_area)` *before* calling it: `panel_view.rs:61` and
  `images.rs:280`. If the pill band is carved off before the fits check runs, the
  outer gate (on the raw area) and the inner check (on the reduced area) disagree
  by the band height — see D1.
- **`wide_hero_hero_pane` recomputes from the raw `area`.** It does not receive
  the panes from `wide_library_panes`; `render_wide_skeleton` hands it the raw
  `area` (`wide.rs:338`). Any top-band reserve must reach it too (D2).
- **`wide_hero_browser_pane`'s pill reserve is shared with Narrow**, which is
  already full-width and must keep it (D3).
- **The reserve height consts are private to `wide_hero.rs`**
  (`WIDE_HERO_PILLS_ROW_HEIGHT`/`WIDE_HERO_PILLS_GAP_ROWS`, both `const`, line
  25/27) — `library.rs` cannot see them without a visibility change (D7).

## Goals / Non-Goals

Goals:
- One full-width Selector band above both Wide panes, reserved after the fits
  check passes, before the split.
- The split drag never touches the pill/spacer rows.
- Remove the List controls row and *all* its plumbing (slot, painter, geometry,
  the `ControlPicked` event and its dispatch arms, and the `home_video` push) with
  no residual dead code — except `is_home_video_view`, which has other callers.

Non-Goals:
- Narrow layout changes (already full-width).
- Touching the split gesture math (`split_gesture_msg`/`split_hit`) beyond the
  gap rect's vertical extent.
- Deltas to sibling specs (see proposal Impact / the N1 reconciliation there).
- Coordinating with `sticky-wide-hero-split`: that change adds a drag-end persist
  message and touches the boundary gesture/prefs, not the gap rect's vertical
  extent (`panel_view.rs`). The two edits are orthogonal and apply in either
  order.

## Decisions

### D1: Fit on the uncarved area, then carve, then split

`wide_library_panes` first checks the breakpoint on the **uncarved** `area`
(so the Wide/Narrow decision does not shift by the band height), then carves the
top `pill_band` rows off, then splits the reduced `content_area` via a
split-only helper. `wide_hero_presentation`'s embedded fits check must run on the
uncarved area, and the split must be reachable without re-running fits on the
reduced area (otherwise heights 7–8 pass the outer gate but fail the inner one,
painting nothing and stranding a stale frame). Callers that gate externally
(`panel_view.rs:61`, `images.rs:280`) keep gating on `wide_hero_fits(raw_area)`,
which stays consistent with the inner decision.

- **Alternative — carve before the fits check:** rejected. It moves the Wide
  breakpoint up by 2 rows and, because the outer gates test the raw area, leaves
  short panels (raw height 7–8) painting nothing while the split/overlay geometry
  goes stale. It also silently drops Wide image projection at those heights
  (`images.rs`).

### D2: Feed the reduced content area to the hero-pane painter

`wide_library_panes` exposes the `content_area` it split, and
`render_wide_skeleton` passes that (not the raw `area`) to `wide_hero_hero_pane`,
so the hero pane starts below the band.

- **Alternative — recompute the reserve inside `wide_hero_hero_pane`:** rejected;
  duplicates the band arithmetic and re-introduces the drift the shared primitive
  exists to prevent.

### D3: Wide pill placement moves out of `wide_hero_browser_pane`

In the Wide path, paint the full-width pill bar + spacer directly over the
reserved band, and give the Browser pane its list box with **no internal pill
reserve** (its content starts at its own top, already below the band). Narrow
keeps `wide_hero_browser_pane(area, area)` unchanged. One browser-pane painter
survives; only the rects it is fed change. The Wide caller supplies the
full-width band's `pills_area`/`spacer_area` and the **`browser_panel`** (the
full un-inset pane rect, so the list-box fill reaches the border exactly as
today) as `list_panel`.

### D4: Split-gap rect starts at the content band

`panel_view.rs` builds the drag-handle gap as `y: area.y, height: area.height`.
That extends the drag target up through the pill band in the gutter columns,
violating "the band is independent of the drag." Change it to the content band's
vertical extent — `y: geometry.hero.y, height: geometry.hero.height` (hero and
browser share the same post-split y/height).

### D5: Left alignment and spacer surface (defaults)

The full-width pills start at the panel's left content edge (the shared
`PANE_PAD_X` inset), reading as one band aligned with the hero content. The
spacer row keeps the `PillRowGap` surface, now full width.

### D6: List controls row removal — full enumeration

The row is removed with all its plumbing. Because every `ControlPicked` dispatch
arm is already a `=> None` no-op (verified across all seven owners), the event is
never acted on and is removed wholesale rather than left as a dead exhaustive arm
(the repo forbids wildcard-hidden or dead exhaustive arms):

- Content/paint: `ListControls` + `controls` field (`content.rs`),
  `paint_list_controls_row` (`slots.rs`), `SkeletonHits.controls`,
  `BrowserPaneGeometry.controls`, `WideSkeletonGeometry.controls`, and the Wide
  `list_panel`/`controls_area` split (`wide.rs:169-186`); drop `controls` from
  `narrow.rs`'s geometry build.
- Event: `LibrarySlotEvent::ControlPicked` (`owner.rs:107`), its resolution arm
  (`panel.rs:655-657`), and its dispatch arms in `home_content.rs:581`,
  `emby_library_content.rs:685`, `podcast_content.rs:600`, `feeds_content.rs:523`,
  `tv_content/interaction.rs:65`, `book_content.rs:483`,
  `music_interaction.rs:150`.
- Push: `home_video` field/push/use (`emby_library_content.rs`) and its
  computation/push (`shell_emby_library_content.rs`). **Keep**
  `is_home_video_view` (`lib_cursor_actions.rs`) — used by `music_actions.rs:164`
  and `tests_non_music.rs:13`. `home_video` (controls label) and `feed_home_video`
  (feed grouping) are distinct; do not touch the latter.
- Vocabulary: remove the "List controls row" definition from `CONTEXT.md` and
  strike it from the "Library panel" definition's slot list.

### D7: Expose the reserve height to `library.rs`

`library.rs` needs the pill-band height to carve it. Add a single
`pub(in crate::app)` accessor in `wide_hero.rs` (e.g. `pub const` or a small
`pill_band_height()` fn) rather than duplicating the `1 + 1` literal, keeping one
definition of the band height.

## Risks / Trade-offs

- **Full-width search box is now the behavior** → intended (user-confirmed);
  follows the "search box uses the Selector row's rect" rule with no
  special-casing.
- **`wide_hero_hero_pane` and `wide_library_panes` disagreeing on the band** →
  mitigated by D2: one source produces the content area both consume.
- **Breakpoint drift at short heights** → mitigated by D1 (fit on uncarved area);
  covered by the threshold-height rect test (task 1.2) at raw heights 7/8/9.
- **Stale controls tests** → deleted, not ported; each deleted assertion names
  its surviving owner test per the frontend skill (task 5.1).

## Migration Plan

Pure UI/render change, single binary, no persistence or protocol impact. No
rollback beyond reverting the commit. Verify by comparing the running Wide panel
against the pre-change build (git stash / prior commit `2dce938c^`): pills
full-width, hero two rows down, drag gap below the band, home-video count gone,
Inline Search bar full-width.

## Open Questions

None.
