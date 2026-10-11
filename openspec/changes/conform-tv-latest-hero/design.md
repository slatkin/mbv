# Design

## Context

See proposal.md (Why). The current state that shapes the approach:

- TV paints through the shared `LibraryPanel` and `render_wide_skeleton`, the same as every destination. The deviation is policy only.
- **Two breakpoint sources.** The panel picks Wide or Narrow with `wide_hero_fits(area)`. The shell pushes a second value to `TvContent::set_is_wide` from `App::wide_tv_library_area` (`src/app/state/projection/tv_wide.rs`). That value also requires `is_wide_tv_library`: the root level's rows must all be `Series`. Latest and Upcoming rows are episodes, so in a wide terminal the TV owner's `TvDisplayMode` is `Narrow` while the panel paints Wide. The "stale bit" guards in `tv_content/keyboard.rs` and `tv_content/interaction.rs` exist because of this.
- **Owner hero gate.** `TvContent::panel_content` and `hero_data` produce a flat-mode hero only when `display_mode == Narrow && hero_overlay_open`. That is the mini-view overlay case.
- **Panel branch that names TV.** `library_panel/panel/view.rs` sets `flat_tv_list` from `LibraryKind::TvShows` and computes `show_hero_pane = !flat_tv_list || owner.browser_rows_are_hero_bearing()`.
- **TV-only mini-view auto-open.** `Model::sync_tv_mini_view_hero` (called from `shell/run.rs`) calls `LibraryPanel::sync_mini_view_hero_overlay`, which uses the `mini_view_hero_available` trait method (only TV overrides it) and the panel's `mini_view_hero_auto_open` flag.
- **Upcoming as fake media.** `MbvEmby::get_upcoming` returns `Vec<EmbyItem>`. `App::tv_upcoming_snapshots` stores them, and the Upcoming `BrowseLevel.items` is filled from them (`dispatch/music.rs`, `dispatch/library/browse/tv.rs`, `dispatch/library/event/browse_loads.rs`). Consumers tell notices apart with `id.is_empty()`: `upcoming_episode_target`, `tree_projection.rs`, `interaction.rs`, and `App::open_series_for_unplayable_episode`, which builds a fake `Series` `EmbyItem` to navigate.
- **Narrow Enter.** The panel opens the overlay on Enter when `owner.hero_overlay_enter_available()` is true, and on a double-click when `owner.double_click_opens_hero_overlay()` is true. `EmbyLibraryContent` and `FeedsContent` use the defaults, so their Latest rows open the overlay. TV flat modes and podcast rows do not.

## Goals / Non-Goals

**Goals:**
- One breakpoint fact for TV, the same one the panel uses.
- The panel decides "Hero pane or no Hero pane" from typed content, not from the destination key.
- A premiere notice cannot reach any media path (play, enqueue, hero, overlay) because of its type.
- One Latest rule for narrow Enter and double-click, held in one place.

**Non-Goals:**
- No change to TV show modes (`All`, ranges), the show tree, or the series Workspace.
- No change to how Upcoming loads (eager, once per session) or how its rows look (date headings, `Sxx:Eyy` subtitles).
- No change to the Wide hero's content for a selected episode. It uses the existing `hero_content_emby` producer that Home already uses for episodes.

## Decisions

### D1: TV gets the breakpoint from geometry only

The shell pushes `App::is_right_panel_wide()` to `TvContent::set_is_wide`. `is_wide_tv_library` and its row-type inspection are deleted. Callers that need "the Wide series Workspace is active" (`input/browse_dispatch.rs`, `shell/tv_workspace.rs` TvActivate and series navigation, `shell/overlays/menus.rs` pointer anchor) use one predicate, `App::wide_tv_show_area(lib_idx)` (the renamed `wide_tv_library_area`). It means "Wide geometry and the current level's content mode is a show mode (`All` or a range)". The mode comes from the level's `tv_content_mode`, the value that `push_tv_workspace_content` already reads, not from row types.

*Alternative:* the owner reads its painted geometry (ADR 0024 retained rects). Rejected: the first frame after a tab switch would paint with the wrong breakpoint. That is the flash `wide_tv_library_area` was made to prevent.

### D2: The panel takes "no Hero pane" from the hero slot type

Replace `LibraryPanelContent::hero: Option<HeroContent>` with a closed `HeroSlot`:

- `HeroSlot::Pane(Option<HeroContent>)`: the list carries media. Wide paints the Hero pane, empty when the selection has no hero. Every owner except TV Upcoming returns this.
- `HeroSlot::Absent`: the list carries no media. Wide paints no Hero pane and the list takes the whole content area. The overlay can never open.

TV returns `Absent` in Upcoming mode in every list state (rows, loading, or empty), so the loading or empty placeholder also spans the content area. Delete `flat_tv_list` and the `show_hero_pane` expression in `panel/view.rs`. The skeleton's `show_hero_pane` parameter comes from the slot variant. `LibraryPanelContent::workspace_focused`, the painted-link read in `view.rs`, `render_wide_skeleton`, and the overlay painter read the hero through the slot.

*Alternatives:* a `ListSlot::Notices` variant was rejected because an empty or loading Upcoming list uses `ListSlot::Empty`, which cannot carry the fact. A new owner trait method `rows_are_media()` was rejected because it is one more bool that has to agree with the content the owner returns.

`browser_rows_are_hero_bearing` keeps its current meaning ("the narrow overlay flow exists"). TV stops overriding it for this purpose (see D4 and D5).

### D3: Upcoming premieres are their own type from the Emby boundary

Add `UpcomingPremiere` to `crates/mbv-emby-model`: `series_id: ItemId`, `series_name`, `season: Option<u32>`, `episode: Option<u32>`, `title`, `premiere_date`. It has no item id, no media sources, and no conversion into `EmbyItem`. `get_upcoming` maps each DTO at the client boundary. A row with no `SeriesId` is dropped there and logged once per fetch, because the only action of a notice is "open its series".

- Storage: `App::tv_upcoming_snapshots: HashMap<String, Vec<UpcomingPremiere>>` is the only store. The Upcoming `BrowseLevel` keeps `items` empty and tracks only `loading` and `rows`. The `TvUpcomingSnapshotFetched` event and the refresh path carry `Vec<UpcomingPremiere>`.
- Projection: `TvWideRenderCtx` gains `upcoming: Vec<UpcomingPremiere>`. `upcoming_episode_rows` builds rows from it. The row target is the premiere's own stable key (series, season, episode, title, date). It replaces `upcoming_episode_target` and the `id.is_empty()` fallbacks in `tree_projection.rs` and `interaction.rs`.
- Activation: a new `ShellRequest::TvOpenUpcomingSeries { series_id, series_name }` replaces the Upcoming use of `TvActivate { item }`. The shell keeps the reveal logic of `open_series_for_unplayable_episode` (letter range, series navigation, Workspace open) but takes the series reference, not a fake `EmbyItem`. Keyboard Enter and mouse activation both emit it.
- Media actions: in Upcoming mode the TV owner emits no play, enqueue, watched, or context-menu requests. `selected_episode_item` and `selected_item` return `None` in Upcoming, so no media path can see a notice.

*Alternative:* an enum on `BrowseLevel` (`Media(Vec<EmbyItem>) | Premieres(..)`). Rejected: every level consumer would have to handle a variant that only one TV mode uses. The snapshot map already exists and is per-library.

### D4: TV Latest's hero is produced like Movies'

In Latest mode, `TvContent::panel_content` and `hero_data` return `hero_content_emby(selected episode)` with the projected hero image, in every geometry and with no overlay check. This matches `EmbyLibraryContent::hero_data`. The narrow skeleton does not paint a hero, so a Narrow frame does not change. The flat-mode `hero_overlay_open && display_mode == Narrow` gates are deleted.

### D5: One Latest rule for narrow Enter and double-click

Add a trait method `LibraryContentOwner::latest_list_active(&self) -> bool`, default `false`. The panel's two narrow overlay gates (Enter in `panel/view.rs`, double-click in `panel/mouse.rs`) refuse the overlay when it is true. The key or click then goes to the owner, which already plays on Enter and double-click: `EmbyLibraryActivate` for Emby libraries, `play_selected_feed` for Feeds, `TvActivate` for TV, and podcast play. Owners return true when Latest is selected: `EmbyLibraryContent::latest_mode`, `FeedsContent` `latest_selected`, `TvContent` in Latest, and `PodcastContent` in Latest (no visible change there).

*Alternative:* each owner overrides `hero_overlay_enter_available` and `double_click_opens_hero_overlay` for Latest. Rejected: four owners each repeat two overrides, and the next Latest destination can forget one. The panel gate is one place.

### D6: Remove the mini-view auto-open

Delete `Model::sync_tv_mini_view_hero` and its call in `shell/run.rs`, `LibraryPanel::sync_mini_view_hero_overlay`, the panel's `mini_view_hero_auto_open` field, and the `mini_view_hero_available` trait method with TV's override. `LibraryPanel` has an approved `#[expect(clippy::struct_excessive_bools, reason = ...)]` whose reason lists `mini_view_hero_auto_open`. If the remaining bools drop below the lint threshold, the expectation is unfulfilled. Then remove the attribute. Otherwise, only edit its reason text. Never add a new suppression.

## Risks / Trade-offs

- [The Emby probe listed `SeriesName` but not `SeriesId` for Upcoming rows.] The current navigation path already requires `series_id` and works on the user's server. → If a live payload has no `SeriesId`, D3 drops the row. The implementer confirms the field from `get_upcoming`'s requested `Fields` before dropping rows, and asks if it is absent.
- [Narrow Movies, Home Videos, and Feeds Latest lose the detail-first overlay.] This is the user's decision (2026-10-11). Other pills keep the overlay, so detail stays one pill away.
- [Removing `TvDisplayMode`'s disagreement can change key routing in flat modes.] `handle_key_wide` and `handle_key_narrow` now really switch at the panel breakpoint in Latest. → An owner test drives Enter, Ctrl+P, and arrows on a Wide TV Latest owner and checks that the displayed row is addressed (the existing "Keyboard actions address the displayed row" scenario).
- [`HeroSlot` changes a public field of `LibraryPanelContent` in `mbv-components`.] All seven constructors and every reader change. The compiler finds these sites. Use no wildcard arm on `HeroSlot`.

## Migration Plan

None. Session-only state: Upcoming snapshots are not persisted, and the launch snapshot records no TV selector.
