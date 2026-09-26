# Interactive Surface Ledger

This ledger records interaction authority and the composing Panel for every
root-visible surface. Interaction state ownership is distinct from composition:
a row's state does not imply that a shell frame may paint beneath it. The root
composes Panels; destinations and content owners supply typed slot content.

## States

- `legacy`: interaction or rendering still depends on global shell authority.
- `component`: an Interactive Component owns the surface while migration work
  remains.
- `migrated`: local interaction authority is complete. This label records state
  ownership only; composition is recorded separately in the Panel column.

## Composition rules

The root composes Tab, Library, Library playback, Queue, Queue playback, Status
bar, pane boundaries, and overlays. Each Panel owns its placement, fill, slots,
painting, and retained hit geometry. No base frame paints or underpaints a
Panel. A destination supplies only typed slot content. A mounted embedded
`MediaList<Target>` is not a separate routed surface; Wide and Inline
presentations read its one owner. Grid is retired.

Mouse eligibility follows the Panel painted in the latest frame (ADR 0024).
Keyboard precedence remains in the single Keyboard Router (ADR 0023); the
router's outcome and the focused leaf's disposition are combined by the one
central arbitration fold (same ADR), which is not a second router.

## Panel ledger

| Root Panel | Owned surface and state | Painter / content source | Placement and input | Verification |
| --- | --- | --- | --- | --- |
| Tab panel | Tab selection and overflow state; local hovered tab identity | `TabPanel` | Library column only; owns tab hit regions and local hover painting | Local hover: `tab_panel::tests::moved_over_unselected_tab_sets_hover_without_selection_or_msg`; live delivery: `tick_integration::mouse::tick_mouse_hover_delivery_updates_only_the_pointed_surface_in_narrow_and_wide_modes` |
| Library panel | Library focus, destination owner map, selector/list/hero/workspace slots; local main Selector-row hovered pill identity | `LibraryPanel`; Home, Emby, TV, Music, Audiobookshelf and Feeds owners supply typed content | Wide or Narrow skeleton; owns all library geometry, slot hits, and main Selector-row hover painting | Local hover: `library_panel::panel_tests::selector_move_updates_only_private_hover_identity_and_returns_no_message`; live delivery: `tick_integration::mouse::tick_mouse_hover_delivery_updates_only_the_pointed_surface_in_narrow_and_wide_modes` (Narrow + Wide) |
| Library playback panel | Library-column playback transport state | `LibraryPlaybackPanel` | Queue column hidden; owns transport geometry | playback component and tick tests |
| Queue panel | Queue cursor, scroll, scope, and row-local state | `QueuePanel` / embedded canonical queue owner | Queue column visible; owns queue frame and row hits | queue component and tick tests |
| Queue playback panel | Queue-column status header, visual slot, and transport state | `QueuePlaybackPanel` | Queue column visible, including idle header; owns transport and visual geometry | queue playback buffer and tick tests |
| Status bar panel | Volume, mute, and remote controls | `StatusBarPanel` | Library column only; owns control hits | status component and tick tests |
| Pane boundaries | Live split gesture state | boundary Interactive Components | Root-placed only where the corresponding split is painted | resize tick tests |
| Overlay stack | Overlay presence, focus and z-order | `UiRootComponent` plus mounted overlay Components | Root-composed above Panels; each overlay owns its surface | overlay tick tests |

## Embedded content owners

Library content owners remain mounted while their Service library is in the
catalog and preserve cursor, scroll, local focus, drafts, and provider intent
translation. They are not root-composed surfaces and do not select a skeleton,
Hero header arm, style, or slot. The Library panel derives Wide/Narrow and the
artwork policy derives Landscape/Portrait/Square. One Panel painter runs for each
surface and breakpoint.

This replaces the former destination-row inventory: Home, generic Emby/Movies/
home videos, TV, grouped Music, Audiobookshelf books/podcasts, and Feeds all
compose through Library panel slots. Queue rows compose through Queue panel
slots. Search, settings, sessions, help, modals, and popups compose through the
appropriate overlay Panel.

## Review history

The 2026-08-27 `migrated` review established interaction state ownership and
removed the global bridge. It did not establish Panel composition or prove the
absence of a base-frame painter; those are separate obligations recorded above.
The 2026-09-13 panel work records the composition boundary explicitly.
