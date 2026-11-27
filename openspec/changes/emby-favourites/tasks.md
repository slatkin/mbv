# Tasks

Run every make target in the foreground and wait for it. Never background a command and poll.

## 1. Emby model and client

- [ ] 1.1 Add `is_favorite: bool` (`#[serde(default)]`) and `EmbyItem::supports_favourite()` to
  `crates/mbv-emby-model/src/lib.rs`. The rule is design D1: false for Season, Folder,
  CollectionFolder, UserView, Channel, Program, Timer, SeriesTimer, Studio, Tag. Update every
  `EmbyItem` constructor the compiler names. Verify: `make checks PKG=mbv-emby-model`; a `rstest`
  `#[case]` table in that crate's `tests/` binary pins one row per excluded type and one allowed
  type per family (Movie, Series, Episode, BoxSet, Audio, MusicAlbum, MusicArtist, Playlist).
- [ ] 1.2 Read `UserData.IsFavorite` in `parse_item` (`crates/mbv-emby/src/types_parsing.rs`).
  Verify: a parse test over recorded item JSON with `UserData.IsFavorite` true and absent; a serde
  test that a queue-item JSON without the field loads as false.
- [ ] 1.3 Add `EmbyClient::set_favourite(item_id, favourite) -> Result<bool, EmbyError>` in
  `crates/mbv-emby/src/client_library.rs` next to `mark_played`: `POST
  /Users/{uid}/FavoriteItems/{id}` to set, `POST .../{id}/Delete` to clear, returning the reply's
  `IsFavorite`. Verify: a mock-HTTP test asserts the method and path of each call and the returned
  value (design D3).
- [ ] 1.4 Add `EmbyClient::get_favourites(library_id, music: bool)`: the Items query of design D6
  with `LATEST_FIELDS`, plus the `/Artists` listing when `music`, merged by id, with items that fail
  `supports_favourite()` dropped. Verify: mock-HTTP tests for the TV shape (a Series, an Episode, a
  Season dropped) and the Music shape (artist merged with a song, duplicate id kept once).

## 2. Menu entry and action

- [ ] 2.1 Add `ContextAction::Favourite(String)` and `Unfavourite(String)` in
  `crates/mbv-ui-model/src/context_menu.rs`; give each an exhaustive arm wherever the compiler
  names a match (no wildcard). Verify: `make checks PKG=mbv-ui-model` reports no missing arm.
- [ ] 2.2 Push the entry in `crates/mbv-client-core/src/app/dispatch/context_menu/menu/emby.rs`: a
  `push_favourite_context_action` called after the played-state entry in both
  `push_folder_context_actions` and `push_leaf_context_actions`, single row only, only when
  `item.supports_favourite()`, and never on the Queue-focus path (design D2). Labels "Add to
  Favourites" / "Remove from Favourites". Verify: unit tests in
  `crates/mbv-client-core/src/app/tests/context_actions.rs` for movie leaf, series collection,
  music track (entry present, no played entry), season (absent), and Queue row (absent); update the
  existing Movie, Continue Watching, and Album menu assertions to the new order.
- [ ] 2.3 Execute the actions in `.../context_menu/actions.rs`: `context_set_favourite` calls
  `set_favourite`, then runs the design D4 flip over the TUI levels and Home content (new helper in
  client-core); no client flashes "Emby is unavailable"; an error flashes an error toast and
  changes nothing. Verify: tests that success flips `is_favorite` in a held level item and that an
  error leaves it unchanged and flashes.
- [ ] 2.4 Extend the GUI Library row-menu path so its `menu_target` / `library_item` resolution
  covers the entry for plain-library, Shows, Music, and Home rows and omits it for a Music artist
  row keyed by name with no artist item id (design D8). Verify: a `gui_core` test builds the menu
  for an id-keyed row (entry present) and a name-keyed artist row (absent).
- [ ] 2.5 Add `CONTEXT.md` term *Favourite* (the Emby per-user flag; *Avoid*: star, like, bookmark)
  and update `docs/recipes.md` only if a recipe's site list moved. Verify: `rg "Favourite"
  CONTEXT.md` shows the term.

## 3. Library view type and dropdown

- [ ] 3.1 Rename `GuiShowsView` to `GuiLibraryView` and the Shows view store, the `ShowsSetView`
  command, the frame field, and `LibraryShowsViewDropdown` to their destination-neutral names
  (design D5), adding the `Favourites` variant with key `favourites` and label "Favourites".
  Update `GuiLaunchShowsView` mapping so a saved Favourites becomes All. This unit may leave the
  workspace uncompilable if `Favourites` has unhandled arms; record that for 3.2. Verify:
  `rg "GuiShowsView|LibraryShowsViewDropdown" crates` returns no hit.
- [ ] 3.2 Add `GuiLibraryView::choices(destination)` (Shows: All, New, Upcoming, Favourites;
  other Emby destinations: All, Favourites) and handle `Favourites` in every `match` the compiler
  names, including `set_shows_view`'s successor and `select_shows_row`. Verify: a core test that
  Shows returns four entries in order and a plain library and Music return two.
- [ ] 3.3 Paint the dropdown in the plain-library and Music list headers through the same
  `LibraryViewDropdown`, with Audiobookshelf and Feeds untouched. Verify: component tests that a
  Movies header and a Music header paint a dropdown marking All, and an Audiobooks header paints
  none; extend `library_shows_view_dropdown/tests.rs` for the fourth Shows entry.
- [ ] 3.4 Pin launch persistence: Favourites saves as All and only Shows persists. Verify: a
  `gui_launch_state` round-trip test with Favourites active.

## 4. Favourites data path

- [ ] 4.1 Add the favourites store (`GuiSectionRequest` per library id plus the in-place refresh
  set) to `GuiClientState`, `LibraryFetchEvent::Favourites`, and its worker spawn using
  `get_favourites` (design D6). Entering the view starts a first load or an in-place refresh
  (design D7). Verify: a core test that setting the view to Favourites sends one fetch, that
  landing fills the store, and that a second entry refreshes in place.
- [ ] 4.2 Project the store into the list parts: the collections-first, Title-ordered
  `GuiEmbyRowFrame` list (design D6, D9) as an optional part on the Shows, Music, and
  plain-library list frames, with `loading` and `failed`. Verify: projection tests for the TV
  ordering (three Series, then the Episode), a Movies list, and a Music list with an artist, an
  album, and a song.
- [ ] 4.3 Resolve a favourites id in `item_actions::library_item` and the select, play, shuffle,
  and enqueue paths, and select into the plain-library Hero producer. Verify: tests that playing a
  favourite episode plays it, Play All on a favourite series matches the All view's call, and an
  unknown id logs a stale-item line and does nothing.
- [ ] 4.4 After a favourite change succeeds, flip the flag in the GUI stores by id and, when the
  active view is Favourites, refetch it so an unfavourited row leaves (design D4). Verify: a test
  that unfavouriting from the open view removes the row and that a favourite added from the All
  view appears on the next entry.

## 5. Favourites list component

- [ ] 5.1 Add `LibraryFavouritesList` in `crates/mbv-gui/src/window/components/` over the shared
  flat-row parts, painted in the list column while the library's view is Favourites; the Hero shows
  the selected favourite; pressing All restores the All list with its selection and scroll
  (spec "A Favourites view replaces the destination's list column"). Verify: component tests that
  the list paints the rows, the empty list paints none, and an All press restores the stored
  selection; no layout or font-metric assertions.
- [ ] 5.2 Wire row press, double-click play, and right-click menu to the existing shared commands
  by stable id. Verify: a component test that a right-click sends the row-menu command with the
  row's Service id.
- [ ] 5.3 Update `docs/architecture/gui-client.md` for the view type, the favourites store, and the
  list part, and add `docs/invariants/` text only if a new property that no type enforces appears.
  Verify: `rg "Favourites" docs/architecture/gui-client.md` shows the section.

## 6. Gates

- [ ] 6.1 Run `make format`, `make checks`, and `make test` in the foreground; fix every red gate
  whoever introduced it. Verify: all three exit 0.
- [ ] 6.2 Run `make check-code-file-lines` and split any file over 800 lines along its
  responsibility seam. Verify: exit 0.

## Workflow follow-up

- Sync the delta specs into `openspec/specs/` and archive the change only when the user approves
  the PR for merge, as the last commit before the merge.
