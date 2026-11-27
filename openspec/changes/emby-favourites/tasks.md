# Tasks

Run every make target in the foreground and wait for it. Never background a command and poll.
Tests are the implementer's choice under `docs/invariants/14-test-ownership.md`; no row demands one.

## 1. Emby model and client

- [x] 1.1 Add `is_favorite: bool` (`#[serde(default)]`) and `EmbyItem::supports_favourite()` to
  `crates/mbv-emby-model/src/lib.rs`. The rule is design D1: false for Season, Folder,
  CollectionFolder, UserView, Channel, Program, Timer, SeriesTimer, Studio, Tag. Update every
  `EmbyItem` constructor the compiler names. Verify: `make checks PKG=mbv-emby-model`.
- [x] 1.2 Read `UserData.IsFavorite` in `parse_item` (`crates/mbv-emby/src/types_parsing.rs`). The
  field's `#[serde(default)]` makes a queue-item file written before this change load as not
  favourite.
- [x] 1.3 Add `EmbyClient::set_favourite(item_id, favourite) -> Result<bool, EmbyError>` in
  `crates/mbv-emby/src/client_library.rs` next to `mark_played`: `POST
  /Users/{uid}/FavoriteItems/{id}` to set, `POST .../{id}/Delete` to clear, returning the reply's
  `IsFavorite` (design D3).
- [x] 1.4 Add `EmbyClient::get_favourites(library_id, music: bool)`: the Items query of design D6
  with `LATEST_FIELDS`, plus the `/Artists` listing when `music`, merged by id, with items that
  fail `supports_favourite()` dropped. A favourited Season therefore never reaches the list, and a
  Music library's artist rows merge with its items one row per id.

## 2. Menu entry and action

- [x] 2.1 Add `ContextAction::Favourite(String)` and `Unfavourite(String)` in
  `crates/mbv-ui-model/src/context_menu.rs`; give each an exhaustive arm wherever the compiler
  names a match (no wildcard). Verify: `make checks PKG=mbv-ui-model` reports no missing arm.
- [x] 2.2 Push the entry in `crates/mbv-client-core/src/app/dispatch/context_menu/menu/emby.rs`: a
  `push_favourite_context_action` called after the played-state entry in both
  `push_folder_context_actions` and `push_leaf_context_actions`, single row only, only when
  `item.supports_favourite()`, and never on the Queue-focus path (design D2). Labels "Add to
  Favourites" / "Remove from Favourites"; a music track's entry follows Enqueue, because
  music has no played-state entry.
- [x] 2.3 Execute the actions in `.../context_menu/actions.rs`: `context_set_favourite` calls
  `set_favourite`, then runs the design D4 flip over every App-held copy (new helper in
  client-core): the TUI levels, Home content, the series details (`series_detail_item`), the
  album track cache, and the artist detail cache; no client flashes "Emby is unavailable"; an
  error flashes an error toast and changes nothing.
- [x] 2.4 Confirm the GUI Library row-menu path carries the entry for plain-library, Shows, Music,
  and Home rows whose item supports favourites, adding no artist-row resolution: the grouped Music
  tree's artist rows offer no favourites entry, keyed by Service id or by name (design D8).
- [x] 2.5 Add `CONTEXT.md` term *Favourite* (the Emby per-user flag; *Avoid*: star, like, bookmark)
  and update `docs/recipes.md` only if a recipe's site list moved. Verify: `rg "Favourite"
  CONTEXT.md` shows the term.

## 3. Library view type and dropdown

- [x] 3.1 Rename `GuiShowsView` to `GuiLibraryView` and the Shows view store, the `ShowsSetView`
  command, the frame field, and `LibraryShowsViewDropdown` to their destination-neutral names
  (design D5), adding the `Favourites` variant with key `favourites` and label "Favourites".
  Update `GuiLaunchShowsView` mapping so a saved Favourites becomes All. This unit may leave the
  workspace uncompilable if `Favourites` has unhandled arms; record that for 3.2. Verify:
  `rg "GuiShowsView|LibraryShowsViewDropdown" crates` returns no hit.
- [x] 3.2 Add `GuiLibraryView::choices(destination)` and handle `Favourites` in every `match` the
  compiler names, including `set_shows_view`'s successor and `select_shows_row`. Shows offers All,
  New, Upcoming, and Favourites in that order; every other Emby destination offers All and
  Favourites.
- [x] 3.3 Paint the dropdown in the plain-library and Music list headers through the same
  `LibraryViewDropdown`, with Audiobookshelf and Feeds untouched. Music and feed-group Emby headers
  paint the collection dropdown and the view dropdown together (the header's `collections`/`view`
  branches stop being exclusive); the collection dropdown is hidden while the view is Favourites
  and returns with its pick intact on All. Follow the design D5 title line order and Favourites
  header rules: no sort control, no tree buttons, no collection dropdown.
- [x] 3.4 Pin launch persistence: Favourites saves as All and only Shows persists.
- [x] 3.5 Paint the Library Status bar's location as design D5 states: while the view is
  Favourites it names Favourites in place of the collection names.

## 4. Favourites data path

- [x] 4.1 Add the favourites store (`GuiSectionRequest` per library id plus the in-place refresh
  set) to `GuiClientState`, `LibraryFetchEvent::Favourites`, and its worker spawn using
  `get_favourites` (design D6). Entering the view starts a first load or an in-place refresh
  (design D7).
- [x] 4.2 Project the store into the list parts: the collections-first, Title-ordered
  `GuiEmbyRowFrame` list (design D6, D9) as an optional part on the Shows, Music, and
  plain-library list frames, with `loading` and no failure mark (design D6's failure rule).
  Add the Favourites Hero and the separate favourites pick (design D6).
- [x] 4.3 Resolve a favourites id in `item_actions::library_item` and the select, play, shuffle,
  and enqueue paths, and select into the Favourites Hero of design D6, not the plain-library Hero
  producer, whose `hero_item` gate paints no Hero for a folder or a non-`Movie`. The favourites
  store resolves an id only while that library's active view is Favourites. Route a favourite
  MusicArtist row's Play All, Shuffle, and Enqueue (row controls and menu entries) through
  the design D8 artist path, and update the `get_artist_audio_tracks` doc comment.
- [x] 4.4 After a favourite change succeeds, flip the flag in the GUI stores by id and, when the
  active view is Favourites, refetch it so an unfavourited row leaves (design D4).

## 5. Favourites list component

- [x] 5.1 Add `LibraryFavouritesList` in `crates/mbv-gui/src/window/components/` over the shared
  flat-row parts, painted in the list column while the library's view is Favourites; the Hero shows
  the selected favourite; pressing All restores the All list with its selection and scroll
  (spec "A Favourites view replaces the destination's list column"). The list keeps its own
  `ListScroll` per library (design D6), paints no letter headings and no rail, and its refresh
  button refetches the favourites list.
- [x] 5.2 Wire row press, double-click play, and right-click menu to the existing shared commands
  by stable id.
- [x] 5.3 Update `docs/architecture/gui-client.md` for the view type, the favourites store, and the
  list part, and add `docs/invariants/` text only if a new property that no type enforces appears.
  Verify: `rg "Favourites" docs/architecture/gui-client.md` shows the section.

## 6. Gates (first pass, pre-correction)

- [x] 6.1 (first pass) Run `make format`, `make checks`, and `make test` in the foreground; fix
  every red gate whoever introduced it. Verify: all three exit 0.
- [x] 6.2 (first pass) Run `make check-code-file-lines` and split any file over 800 lines along
  its responsibility seam. Verify: exit 0.

Correction pass (2026-11-27 review): the first pass merged what Emby keeps separate (TV Favourite
Shows / Favourite Episodes), gave the Grouped Music library favourites affordances the user
excluded, and missed the Hero heart and two live defects. Section 7 corrects the implementation
to the revised spec; sections 1–6 stay as the record of the first pass.

## 7. Correction: separate TV views, exclude Grouped Music, Hero heart

- [x] 7.1 Split the view type: add `FavouriteShows` and `FavouriteEpisodes` variants to
  `GuiLibraryView` beside `Favourites` (design D5). `choices` returns (All, New, Upcoming,
  Favourite Shows, Favourite Episodes) for Shows, (All, Favourites) for non-music Emby
  destinations, and no entries for a Grouped Music library, whose header paints no view
  dropdown. Labels "Favourite Shows" / "Favourite Episodes", keys `favourite-shows` /
  `favourite-episodes`. Verify: `make checks PKG=mbv-gui`.
- [x] 7.2 Project the split (design D6, D9): the favourites store keeps holding the whole fetch;
  the projection filters Series rows for Favourite Shows and Episode rows for Favourite
  Episodes. The dropdown, header rules, status bar location, scroll, pick, and Hero wiring treat
  both new views exactly as they treated Favourites.
- [x] 7.3 Exclude Grouped Music (design D11): `push_favourite_context_action` pushes nothing when
  the resolved target belongs to a Grouped Music library; the destination never starts a
  favourites fetch and paints no dropdown. The `/Artists` merge in `get_favourites` and the
  artist play routing (design D8) stay for non-grouped music libraries only.
- [x] 7.4 Add the Hero heart (design D10): a heart control in the Hero's upper right on every
  non-Grouped-Music Emby Hero whose item `supports_favourite()`, filled per `is_favorite`,
  press toggles through the same write path as the menu entry. No heart on Music destination
  Heroes or unsupported items.
- [x] 7.5 Fix the stale-copy defect: `flip_favourite_held_copies` reaches `home_recently_added`
  so a Home row's menu never shows a stale favourites entry after a write (design D4).
- [x] 7.6 Fix the Favourites list keyboard routing: Enter, `a`, and the arrows work on
  Favourites rows in every destination, per `crates/mbv-gui/AGENTS.md` keyboard-and-pointer
  rule.
- [x] 7.7 Gates: `make format`, `make checks`, `make test`, `make check-code-file-lines` in the
  foreground, all exit 0.

## Workflow follow-up

- Sync the delta specs into `openspec/specs/` and archive the change only when the user approves
  the PR for merge, as the last commit before the merge.
