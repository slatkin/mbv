# Proposal

## Why

In Wide geometry the Selector row (the pill bar) is reserved only inside the
Browser (list) pane's width, so it sits above the list but stops short of the
Hero pane. Widening it to span the whole Library panel gives the pills the full
panel width and reads as one chrome band over both panes. At the same time the
List controls row — the right-aligned `"{N} items"` label on Emby home-video
libraries — is not visible in practice and the user wants it gone rather than
repaired.

## What Changes

- In Wide geometry the Selector row becomes a **full-width band across the top
  of the whole Library panel**, spanning both the Hero and Browser panes, with
  its one-row parent-background spacer below it. The Hero/Browser split is
  computed over the content area *below* that reserved band, so both panes start
  two rows down.
- The full-width Selector band is **independent of the Hero/Browser split
  drag-resize**: the drag-handle gap covers only the content band below the
  pills, never the pill or spacer rows.
- While Inline Search is active in Wide, the search box occupies the full-width
  Selector row (it reuses the Selector row's exact rect), so the search bar also
  spans the full panel width.
- **BREAKING (UI):** The List controls row is removed entirely — the
  `ListControls` slot, its painter, its geometry, and the Emby home-video
  `home_video` plumbing that fed it. Destinations no longer supply a List
  controls row.
- Narrow geometry is unchanged: it is already a single full-width pane whose
  Selector row spans the whole panel.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-panel`: the Selector row's Wide placement changes from "inside the
  list pane" to a full-width band above both panes that is independent of the
  split drag; the "one optional List controls row" is removed from the list
  pane's row set and from the non-Wide composition.

## Impact

- **Render/arrangement**: `src/app/render/arrangements/library.rs`
  (`wide_library_panes` reserves the top band before splitting),
  `src/app/render/arrangements/wide_hero.rs` (`wide_hero_hero_pane` must paint
  over the reduced content area, not the raw area).
- **Panel skeleton**: `src/app/components/library_panel/wide.rs`
  (`render_wide_skeleton`, `paint_browser_pane` — full-width pill placement, no
  internal pill reserve in the Wide path, controls-row removal),
  `src/app/components/library_panel/panel_view.rs` (split-gap rect starts at the
  content band, not `area.y`), `src/app/components/library_panel/narrow.rs`
  (keeps its own full-width pill reserve; drops the controls field).
- **Content/plumbing**: `src/app/components/library_panel/content.rs`
  (`ListControls`, `controls` field), `.../slots.rs` (`paint_list_controls_row`),
  `src/app/components/emby_library_content.rs` (`home_video` field/push/use),
  `src/app/shell_emby_library_content.rs` (`home_video` computation/push).
  `is_home_video_view` (`src/app/lib_cursor_actions.rs`) is retained — it still
  has non-controls callers (`music_actions.rs`).
- **Sibling specs (no delta this change)**: `library-list-hero`,
  `tv-letter-filtering`, `right-panel-arrangements`,
  `audiobookshelf-podcast-library-ui` describe the Selector row's position
  relative to the list ("top of the right-hand list rail", "above the browser").
  `library-panel` is the geometry authority; those descriptions remain accurate
  as to *which pills appear* and are not re-litigated here.
- **Reconciling the "no full-width area above the browser" prohibitions**:
  `library-list-hero/spec.md` ("No presentation SHALL reserve a separate
  full-width area above the browser") and `right-panel-arrangements/spec.md`
  ("no separate hero area is reserved above the browser") sit in the paragraphs
  governing the **non-Wide inline hero / detail block**: they forbid placing the
  *selected item's hero/detail* in a reserved band above the browser (it must be
  a row replacement in non-Wide, beside the list in Wide). This change reserves a
  full-width band for the **Selector row (pills)**, not a hero/detail area, and
  only in Wide — the hero stays beside the browser. The prohibitions are not
  contradicted, so those specs get no delta. (The Narrow Selector row is already
  full-width above the browser today, confirming the clauses target the hero, not
  the selector.)
- **Optional-slot example scenario**: the `library-panel` "optional slot renders
  absent identically" scenario used List-controls absence as its example; with
  that row removed it is re-anchored to Workspace absence, which is the same
  behavior on a slot that still exists.
