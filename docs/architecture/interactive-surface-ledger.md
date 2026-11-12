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

## Surface state and adapters

A surface that both Clients share keeps its interaction state one time, in
`crates/mbv-surfaces`, as front-end-neutral Surface state (the Client core).
The pointer gesture policy lives there too (`mbv_surfaces::gesture`): the
double-click window with the `SamePlace` rule and `DOUBLE_CLICK_SLOP`, the
uniform wheel step, the drag anchor, and the Ctrl/Shift choice of toggle or
range selection. `FocusStack` owns the mounted set, Panel focus, and the
overlay stack. `HitMap<Target>` resolves a neutral pointer event against the
last painted frame's regions, and `ListCapacity` carries how many whole items
of a shared list a Client painted fully visible — the one source of paging
stride and viewport clamping.

The mounted TuiRealm component of a surface with Surface state is an adapter.
It forwards neutral events into the state, paints from the state, and reports
each frame's capacity and `HitMap` from its own painted geometry. It keeps
only geometry and translation: the paint policy, the painted rectangles, the
retained hit geometry, and the raw-input translation. It holds no copy of a
value the Surface state owns.

Behaviour lives once: each interaction rule exists in the Client core one
time, and each Client only translates input and paints. A rule change in the
core reaches both Clients with no change to either Client's code. A row below
names its Surface state where one exists; a surface whose state is still
component-private keeps it until its migration moves it.

The GUI Library panel and the GUI Settings screen are the GUI Client's
surfaces that hold no Surface state: each keeps its interaction state in its
own window content. They share no interaction rule with the TUI, so there is
nothing for `mbv-surfaces` to own (ADR 0036, ADR 0037). Their pointer input
never reaches a `HitMap` or a `GuiSurfaceLeaf`. The Library panel has no
`[keys]` action; the Settings screen's open actions are shared `[keys]`
actions, which the GUI Client core decides.

## Panel ledger

| Root Panel | Owned surface and state | Painter / content source | Placement and input | Verification |
| --- | --- | --- | --- | --- |
| Tab panel | Tab selection and overflow state; local hovered tab identity | `TabPanel` | Library column only; owns tab hit regions and local hover painting | Local hover: `tab_panel::tests::moved_over_unselected_tab_sets_hover_without_selection_or_msg`; live delivery: `tick_integration::mouse::tick_mouse_hover_delivery_updates_only_the_pointed_surface_in_narrow_and_wide_modes` |
| Library panel | Library focus, destination owner map, selector/list/hero/workspace slots; local main Selector-row hovered pill identity | `LibraryPanel`; Home, Emby, TV, Music, Audiobookshelf and Feeds owners supply typed content | Wide or Narrow skeleton; owns all library geometry, slot hits, and main Selector-row hover painting | Local hover: `library_panel::panel_tests::selector_move_updates_only_private_hover_identity_and_returns_no_message`; live delivery: `tick_integration::mouse::tick_mouse_hover_delivery_updates_only_the_pointed_surface_in_narrow_and_wide_modes` (Narrow + Wide) |
| Library playback panel | Library-column playback transport state | `LibraryPlaybackPanel` | Queue column hidden; owns transport geometry | playback component and tick tests |
| Queue panel | `QueueSurface` (`mbv_surfaces::queue`): the `MediaList<QueueSlotId>` owner's rows, cursor, marks, selection, and viewport, the queue link, the reported viewport height, the row hover (the grip and actions-button row), and the row drag in progress with its drop index and insertion anchor | `QueueComponent`, the TuiRealm adapter over `QueueSurface`; `mbv-render` paints it | Queue column visible; owns queue frame and row hits | queue component and tick tests |
| Queue playback panel | `QueuePlaybackSurface` (`mbv_surfaces::queue_playback`): the transport band's focus presentation, the seek target's throttle state, and the scrubber hover's position | `QueuePlaybackPanel`, the TuiRealm adapter over `QueuePlaybackSurface` | Queue column visible, including idle header; owns transport and visual geometry | queue playback buffer and tick tests |
| Status bar panel | `StatusBarSurface` (`mbv_surfaces::status_bar`): the prefix-armed display and the toast presentation | `StatusBarPanel`, the TuiRealm adapter over `StatusBarSurface` | Library column only; owns control hits | status component and tick tests |
| Pane boundaries | Live split gesture state | boundary Interactive Components | Root-placed only where the corresponding split is painted | resize tick tests |
| Overlay stack | Overlay presence, focus and z-order | `UiRootComponent` plus mounted overlay Components | Root-composed above Panels; each overlay owns its surface | overlay tick tests |
| GUI Library panel | GUI-owned interaction state in the GUI Client's window content (`crates/mbv-gui/ui/src/lib/library/state.ts`): cursor, hover, scroll, open tree nodes, pane focus, filter text, the open Hero overlay and item menu. The GUI Client core holds the data selection (the active destination, each destination's selected collections, the Hero's item). No `mbv-surfaces` Surface state, no `GuiSurfaceLeaf`, no `[keys]` action (ADR 0036) | `LibraryPanel.svelte` and its children (`LibraryTopBar`, `LibraryPills`, `LibraryList`, `LibraryLetterRail`, `LibraryHero`, `LibraryTracklist`, `LibraryStatusBar`) | GUI window only, in the Library column beside the Queue panel; the window content takes its own keys before the forwarder and handles its own pointer input | key routing: `$lib/library/keys.test.ts`; commands and frames: `crates/mbv-gui/tests/core_thread/library.rs`, `crates/mbv-gui/tests/core_thread/launch.rs`; paint: `LibraryPanel.test.ts`, `LibraryList.test.ts`, `LibraryPills.test.ts`, `LibraryHero.test.ts`, `LibraryStatusBar.test.ts` |
| GUI Settings screen | GUI-owned interaction state in the GUI Client's window content (design D11, `crates/mbv-gui/ui/src/lib/settings/`): the sidebar cursor, the keyboard position (sidebar or content), each list's cursor and scroll, form drafts, choice marks before apply, the capture state, and the Mini pane. The GUI Client core holds whether the screen is open, the shown section, the section's data and caches, the setup form's progress and error, the last refused Keys edit, and every action. No `mbv-surfaces` Surface state and no `GuiSurfaceLeaf`; the open actions are shared `[keys]` actions that the core router resolves while `ComponentId::Overlay(OverlayId::Settings)` holds the core focus, and the screen's global keys keep working behind it (ADR 0037) | `SettingsScreen.svelte`, `SettingsSidebar.svelte`, and the section components; the core's `GuiSettingsFrame` supplies the open section's content | GUI window only, in place of the two columns while `frame.settings` is present; the window content takes the screen's keys before the forwarder, and a key it does not take reaches the core router | key routing and capture: `$lib/settings/keys.test.ts`; commands, section loads, and frames: `crates/mbv-gui/tests/core_thread/settings.rs`; paint: `SettingsScreen.test.ts`, `SettingsSidebar.test.ts` |

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

## Wheel ledger

Every scrollable surface interprets the wheel as one viewport step of three
rows or text lines, toward the preceding rows for `ScrollUp` and the following
rows for `ScrollDown`, clamped at the flow bounds (mouse-input "Wheel scrolling
moves the viewport by a uniform step"). A wheel gesture never moves, selects or
extends the selection. The owning component applies the step in its own
viewport state and sends beyond itself only for the semantic boundary below,
never to relay the movement (mouse-input "The owning component applies the
wheel scroll"). Each surface claims the wheel from its own painted region, or
as the sole eligible focused overlay (mouse-input "A wheel gesture is claimed
only by its painted surface"). A wheel scroll releases the viewport to
`ViewportAnchor::Free`; the next keyboard or shell operation on the selection
re-anchors it (shared-list-components "A selection change makes the viewport
follow the selection again").

| Surface | Local owner and painted claim | Breakpoint evidence | Semantic boundary | Wheel proof |
| --- | --- | --- | --- | --- |
| Emby library (plain kinds: Movies / Home videos / Generic) | `EmbyLibraryContent` over the Library panel's canonical list; `WideMediaList` in Wide or `InlineMediaBrowser` in Normal/Narrow | Wide and Normal/Narrow owner paths; panel tick coverage | resolved viewport reach for pagination and resolved scroll for persistence only | `emby_library_content::tests::wheel_reach_resolves_the_last_painted_selectable_row_as_an_item_index` (grouped flow: selection unchanged, offset +3, reach is the last painted selectable item index); empty library: `emby_library_content::tests::wheel_over_an_empty_library_reports_no_reach_and_never_panics`; panel-delivered home-videos library: `narrow_browse_migration::feed_home_video_group_browser_wheel_scrolls_viewport_and_reports_reach`; shared `Scroll` semantics: `media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` |
| Wide TV | `TvContent`; painted series rail claimed by its embedded list | Wide workspace tests; narrow ownership is TV's own content owner | none for wheel; no relay | tree `Scroll`: `list::tree_browser::tests::structure::a_wheel_scroll_moves_the_viewport_and_keeps_the_selected_node`; shared list `Scroll`: `media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` — `TvContent` routes the wheel into its shared owners (carrier list or tree) and adds no step of its own |
| Home | `HomeComponent`; canonical list or inline-hero claim | Wide and Normal/Narrow | none for wheel | `media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` — `claim_wheel` only delegates `Wheel` into the shared `Scroll` and claims the gesture; Home adds no step, clamp or cursor-follow rule |
| Queue | `QueueComponent`; painted `WideMediaList` queue region | Wide and narrow | none for wheel | `queue::tests::a_preserve_push_after_a_wheel_scroll_keeps_the_scrolled_offset`; live delivery: `tick_integration::mouse_sidebar::tick_queue_only_wheel_excludes_unpainted_library_and_keeps_keyboard`; obscured-queue rejection: `tick_integration::mouse::tick_context_menu_wheel_does_not_mutate_the_obscured_queue` |
| Music | `MusicWorkspaceComponent`; Wide rail or Normal/Narrow inline list | Wide and Normal/Narrow | resolved viewport reach for pagination | `music_content::tests::wheel_scrolls_the_viewport_and_reports_the_last_painted_album_reach` (reach is the last painted album-or-later row, selection unchanged); collapsed artist root: `music_content::tests::wheel_over_a_painted_artist_root_still_focuses_library`; tree `Scroll`: `list::tree_browser::tests::structure::a_wheel_scroll_moves_the_viewport_and_keeps_the_selected_node` |
| Feeds | `FeedsComponent`; active canonical list region | Wide and Normal/Narrow | none for wheel | `tests::feeds_component_tests::unfocused_component_wheel_scrolls_viewport_and_keeps_selection` |
| Audiobookshelf podcast | `PodcastContent` over the Library panel's canonical list; painted episode-row geometry | Wide and Normal/Narrow | none for wheel | `media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` — `PodcastContent` delegates the wheel into the same shared `Scroll`; its panel gates the claim on the painted episode rows and adds no step of its own |
| Audiobookshelf books | `AudiobookshelfBookComponent`; painted book- or chapter-row geometry | Wide and Normal/Narrow | panel focus only | `media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` — `AudiobookshelfBookComponent` delegates the wheel into the same shared `Scroll`; its panel gates the claim on the painted book- or chapter-row geometry and adds no step of its own |
| Inline Search | active host component; painted results `left_area`, first refusal | Emby library, Music, and TV host paths | local results viewport only | `media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` — an active Inline Search routes the wheel into its canonical list and takes no step of its own |
| Global Search sidebar | `SearchSidebarComponent`'s shared list owner (`MediaListCarrier`) over the filtered results; claim through painter-published result-row hit regions | fixed overlay geometry (breakpoint-invariant) | local results viewport only | `search_sidebar::tests::search_sidebar_wheel_scrolls_the_results` (selection unchanged, offset +3, offset survives a repaint) |
| Settings | `SettingsComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel | `settings::tests::mouse_and_setup::settings_wheel_steps_the_document_and_leaves_the_cursors_alone` (document offset +3, all three cursors unchanged) |
| Help | `HelpComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel | `tick_integration::mouse_sidebar::tick_help_sidebar_scrolls_immediately_after_open_without_click` (focused sole overlay at an off-panel pointer; one step = three lines) |
| Sessions | `SessionsComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel | `sessions::tests::a_wheel_event_scrolls_sessions_and_keeps_the_selection` (selection unchanged, viewport offset +3) |
| Playlists | `PlaylistsComponent`'s shared list owners (`MediaListCarrier`, one for the saved playlists and one for the open playlist); focus-owned wheel while sole eligible overlay scrolls the visible list | fixed overlay geometry (breakpoint-invariant) | none for wheel | `playlists::tests::playlists_overlay_wheel_scrolls_the_visible_list` (open-list cursor unchanged, offset +3, offset survives a repaint); live claim at an off-panel pointer: `tick_integration::mouse_sidebar::playlists_sidebar_claims_immediate_wheel_and_keeps_normal_keys` |

Shared list-layer proofs behind the rows above:
`list::viewport::tests::scroll_viewport_clamps_at_flow_bounds_and_releases_following`
owns the three-row step, the flow-bound clamp and the `Free` release;
`list::viewport::tests::a_freely_scrolled_viewport_resolves_without_the_selection_pull`
and `media_list::tests::refreshed_content_keeps_a_freely_scrolled_viewport` own
the free resolve across a repaint and a content refresh;
`media_list::tests::a_key_after_a_free_scroll_bring_the_selection_back_into_view`
owns the re-anchor on a keyboard move;
`gesture::tests::back_to_back_wheel_events_at_the_same_instant_all_recognize`
(the `mbv-surfaces` public-API test) owns the no-drop recognizer rule;
`library_panel::panel::tests::surface::overview_wheel_steps_inside_the_painted_box_and_not_outside`
owns the Wide hero overview's prose step and its inside-box claim;
`library_viewport_reach_tests::viewport_reach_within_prefetch_ahead_arms_the_next_page_fetch`
owns the shell next-page arm (mouse-input "Wheel scrolling loads further
library pages").

## Review history

The 2026-08-27 `migrated` review established interaction state ownership and
removed the global bridge. It did not establish Panel composition or prove the
absence of a base-frame painter; those are separate obligations recorded above.
The 2026-09-13 panel work records the composition boundary explicitly.
