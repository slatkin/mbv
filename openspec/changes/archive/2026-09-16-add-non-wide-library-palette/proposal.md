## Why

The non-Wide (Narrow/Mini) library panel had no body surface of its own. The shell filled the whole
library placement with the library column's fixed backdrop (`#2d353b`) and the inline list drew no
fill at all, so the list's zebra stripe was the only thing painting a second tone. A focused Narrow
library therefore looked like a list floating on a gutter rather than a panel with a body, and three
sites had each invented their own fill for the same region — the panel's Selector spacer row, the
status band's padding rows, and the list's scrollbar column — with three different identities.

The first attempt at this fix (`0b0023c0`) changed the shared surface-table value for
`MainContentBox`'s focused fill. Because that identity is shared by every geometry, the change leaked
the library's focused tone into Wide, Music, the Queue and the Workspace; it was reverted
(`e127177b`). The palette needed a geometry-scoped route, not a table edit.

Mini is part of the same story: Mini takes the non-Wide skeleton, and the narrow view is the focused
one there, so Mini follows the non-Wide palette rather than holding a resting-only appearance.

## What Changes

- **One body authority for the non-Wide library panel.** The library column's own body fill resolves
  a named non-Wide surface identity, guarded twice — by the shared breakpoint predicate (Wide keeps
  the column's backdrop, byte-for-byte) and by the library panel-focus bit (a resting narrow library
  keeps the backdrop). The panel placement, the Selector row's spacer row, and the status band's
  padding rows all take it; the status row itself stays the status bar's.
- **The non-Wide list owns the surface under it.** The list's paint policy carries its own body fill;
  the painter fills its claim rect with it and the scrollbar column resolves that same fill. The row
  flow is inset by one spacer row above and below (`PANE_PAD_Y`) *inside* that claim, so the spacer
  rows are the inset's surface, not the panel's.
- **The non-Wide list stripes with the library-panel pair** — focused `#3c4841`, resting `#333c43` —
  while its body is the `MainContentBox` pair (`#48584e` focused, `#2d353b` resting). This satisfies
  the existing rule that a stripe never equals the fill of its own list box.
- **Mini follows the non-Wide presentation**, focused fills included.
- Out of scope: the Wide library palette (unchanged), the mini library *playback* panel (deferred,
  slatkin/mbv#726), and every other list's striping.

## Impact

- Affected specs: `library-panel` (body authority, inset), `canonical-media-lists` (body ownership,
  non-Wide stripe pair).
- Affected code: `src/app/shell_library_panel.rs`, `src/app/shell_chrome_panels.rs`,
  `src/app/components/library_panel/{narrow,wide,slots,content,panel_list}.rs`,
  `src/app/components/media_list/mod.rs`, `src/app/render/components/media_list/wide.rs`,
  `src/app/render/theme/{surface,surface_table,surface_resolve}.rs`.
- Wide appearance is unchanged; the change is provably unreachable from Wide geometry.
