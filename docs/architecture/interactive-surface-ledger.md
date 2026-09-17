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
| Tab panel | Tab selection and overflow state; local hovered tab identity | `TabPanel` | Library column only; owns tab hit regions and local hover painting | Local hover: `tab_panel::tests::moved_over_unselected_tab_sets_hover_without_selection_or_msg`, `moved_into_gap_clears_hover_without_selection_or_msg`, `moved_over_selected_tab_sets_hover_without_changing_selection`; live delivery: `tests_tick_integration_mouse::tick_mouse_hover_delivery_repaints_only_the_pointed_surface_in_narrow_and_wide_modes` |
| Library panel | Library focus, destination owner map, selector/list/hero/workspace slots; local main Selector-row hovered pill identity | `LibraryPanel`; Home, Emby, TV, Music, Audiobookshelf and Feeds owners supply typed content | Wide or Narrow skeleton; owns all library geometry, slot hits, and main Selector-row hover painting | Local hover: `library_panel::panel_tests::selector_move_updates_only_private_hover_identity_and_returns_no_message`; live delivery: `tests_tick_integration_mouse::tick_mouse_hover_delivery_repaints_only_the_pointed_surface_in_narrow_and_wide_modes` (Narrow + Wide) |
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

## Embedded content owners

Library content owners remain mounted while their Service library is in the
catalog and preserve cursor, scroll, local focus, drafts, and provider intent
translation. They are not root-composed surfaces and do not select a skeleton,
Hero header arm, style, or slot. The Library panel derives Wide/Narrow and the
artwork policy derives Landscape/Portrait/Square. One Panel painter runs for each
surface and breakpoint.

### Viewport step (media-list-viewport-scroll)

Every carrier-backed canonical surface interprets the wheel and the `Ctrl+e`/
`Ctrl+y`/`PgUp`/`PgDn` chords as viewport steps (design D1/D6): the window moves
one display row (or one painted page), the selection rides only when the step
would leave it outside, and the first and last display rows are reachable. Each
converted surface names its viewport-step proof:

| Converted surface | Viewport-step proof |
| --- | --- |
| Emby library (plain kinds: Movies / Home videos / Generic) | `tests_tick_integration_library_scroll::viewport_step_inputs_walk_a_grouped_list_to_display_row_0_wide_and_narrow` (wheel and chord each step the letter-grouped window one row, drag the selection only at the window's edge, and reach display row 0 where the first group's `Heading` paints, at Wide and non-Wide heights); `tests_tick_integration_emby_library::browser_viewport_chords_step_the_window_at_wide_and_narrow_heights`; `tests_tick_integration_library_scroll::library_panel_viewport_wheel_reports_position_without_a_cursor_echo` |
| Wide TV | `tests_tick_integration_library_panel::tv_series_rail_viewport_chords_step_the_window_at_wide_and_narrow_heights` |
| Home | `tests_tick_integration_home::home_viewport_chords_step_the_window_at_wide_and_narrow_heights`; wheel delivery over current inline geometry: `home_narrow_tick_wheel_and_click_use_current_inline_geometry` |
| Queue | `queue_component_tests::queue_viewport_ctrl_chords_step_the_window`; wheel steps through the shared conversion — `queue.rs` delegates `Wheel` to `delegate_row_local_input` |
| Music | `music_content_tests::album_wheel_steps_the_viewport_and_requests_the_cursor_only_when_dragged`, `album_viewport_chords_step_the_rail_and_report_only_drags`, `track_viewport_chords_step_the_focused_track_list`; live-tick rail: `tests_tick_integration_music_mouse::music_wide_album_rail_wheel_steps_the_viewport_and_requests_the_cursor_only_when_dragged` |
| Feeds | `feeds_component_tests::feeds_admits_the_two_viewport_ctrl_chords_and_rejects_others`; live-tick wheel claim: `tests_tick_integration_feeds::feeds_tick_wheel_is_claimed_only_over_active_control` |
| Audiobookshelf podcast | `tests_tick_integration_podcast::podcast_wheel_steps_the_show_viewport_without_a_selection_move`; `podcast_viewport_chords_step_the_show_window` |
| Audiobookshelf books | `tests_tick_integration_book::books_panel_is_focused_and_mouse_eligible` (wheel steps the book window one row); `book_viewport_chords_step_the_book_window` |
| Inline Search | `inline_search::tests::wheel_steps_the_viewport_and_the_selection_rides_only_at_the_edge`; `viewport_ctrl_chords_step_the_results_window_and_other_ctrl_chords_stay_rejected` |

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
