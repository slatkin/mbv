## Why

The non-Wide library panel is a second skeleton with its own surface identity
(`NarrowLibraryBody`) and its own list paint policy (`PanelListPaintPolicy::Narrow`). Today's commits
already gave its list box the same `LibraryPanel`/`MainContentBox` pair as the Wide browser list
(`c7aff1d2`, `060b54cd`), so the only remaining difference is the body around it: the non-Wide body
follows focus (focused `#3c4841`) while the Wide column stays on the fixed backdrop (`#2d353b`). A
focused Narrow library therefore paints one flat green surface instead of a list box on the backdrop,
and the geometry-scoped palette has no reason left to exist.

## What Changes

- The non-Wide (Narrow/Mini) library renders through the **same browser-pane painter** as the Wide
  library's list column, with the Hero pane absent. The non-Wide list is not a second presentation
  of the browser pane; it *is* the browser pane at full width.
- **BREAKING (visible)**: the non-Wide library column body resolves the fixed app backdrop `#2d353b`
  in every focus state, matching the Wide library column. The panel placement, the Selector row's
  spacer row, the status band's padding rows and the list's scrollbar column all stop carrying a
  focus-driven tone.
- Retire `Surface::NarrowLibraryBody`, `PanelListPaintPolicy::Narrow`, and the `body`/`scrollbar`
  overrides on `WideMediaListPaintPolicy`; the non-Wide list uses the Wide policy verbatim (the
  skeleton, not the list, paints the list box).
- Preserve the Wide browser pane's saved geometry exactly: the non-Wide list keeps its two-column row
  indent, and its claim/row-flow/hit rects become the Wide pane's.
- Preserve the Library Hero overlay: opening it in non-Wide geometry still requires a "non-Wide
  painted this frame" signal; the overlay's own pixels and behaviour do not change.
- Wide appearance is unchanged. This supersedes the four requirements added by
  `add-non-wide-library-palette` (archived `2026-09-16`).

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `library-panel`: replaces "The non-Wide library panel body has one fill authority" and "The
  non-Wide list panel is inset inside its own surface" with one requirement that the non-Wide panel
  is the Wide browser pane without a hero, on the fixed column backdrop.
- `canonical-media-lists`: replaces "The non-Wide library list owns the surface under it" and
  "Non-Wide library lists use the Wide browser list's pair" with one requirement that the non-Wide
  library list uses the Wide browser list's policy and surfaces verbatim.

## Impact

- Affected specs: `library-panel`, `canonical-media-lists`.
- Affected code: `src/app/components/library_panel/{narrow,wide,panel_list,panel,panel_view,mod}.rs`,
  `src/app/components/media_list/mod.rs`, `src/app/render/components/media_list/wide.rs`,
  `src/app/render/theme/{surface,surface_table,surface_resolve}.rs`,
  `src/app/render/tests_surface_conformance.rs`, `src/app/shell_library_panel.rs`,
  `src/app/shell_chrome_panels.rs`.
- Affected docs: `docs/adr/0028-geometry-scoped-surface-identity.md` (Consequences: the non-Wide
  palette identity is retired).
- No Service, Player, queue-authority, persistence, or protocol change. The Library Hero overlay's
  appearance, focus and dismissal behaviour are unchanged.