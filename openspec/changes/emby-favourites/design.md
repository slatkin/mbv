# Design

## Context

See `proposal.md` for the motivation. The facts below come from two read-only investigations: the
official Emby web client's own source, served by the user's Emby server (4.10.2.0) under
`/web/`, and read-only API probes of that server. Nothing was written to the server.

What Emby does:

- Write: `POST /Users/{uid}/FavoriteItems/{id}` sets; the web client clears with
  `POST /Users/{uid}/FavoriteItems/{id}/Delete`. Both answer a `UserItemDataDto` with `IsFavorite`
  (`modules/emby-apiclient/apiclient.js`, and the API reference).
- Read: `GET /Users/{uid}/Items?ParentId=<lib>&Recursive=true&Filters=IsFavorite` is scoped per
  library. On the probed server it returned 1 Movie in Movies; 3 Series and 1 Episode in TV; 3 Audio
  in Music. Every list fetch mbv makes already asks for `UserData`, which carries `IsFavorite`.
- Which items: the web client's `canRate(item)` (`baseitemcontroller.js`) is false when the item has
  no `UserData` or its type is Program, Timer, SeriesTimer, CollectionFolder, UserView, Channel,
  Season, Studio, Folder, or Tag. Every other type gets "Add to / Remove from Favorites".
- Where listed: the TV library has a Favorites tab with two sections, Favorite Shows (Series) and
  Favorite Episodes (Episode). The Movies library has a Favorites tab listing Movies. Music has no
  Favorites tab; its Albums, Artists, and Songs tabs carry a favourite filter.
- Music artists: an `/Artists?UserId&ParentId&Filters=IsFavorite&Fields=UserData` listing answers
  with `MusicArtist` items whose ids are the same id space as the `ArtistItems` ids on an album
  payload (probed: both `531595` and `590889` appear as "Metallica" in each). A library can hold
  several artist items with one name.

What mbv does today:

- `ContextAction::MarkPlayed` / `MarkUnplayed` run in the Client's `App`
  (`context_set_played`) as a Service call; no ctrl variant, no Owner involvement. The menu is built
  in `build_explicit_item_context_menu` (client-core) and shared by the TUI and the GUI row menu.
- `EmbyItem` has no favourite field. It is serialized inside queue items.
- The GUI Shows destination has a single-choice view type (`GuiShowsView`: All, New, Upcoming), its
  store, command (`ShowsSetView`), frame field, dropdown (`LibraryShowsViewDropdown` over the shared
  `LibraryDropdown`), and a launch-state mirror (`GuiLaunchShowsView`). The plain Emby destination
  and the Music destination have no view dropdown.
- Invariant 38 requires one mechanics owner for a repeated GUI shape.

## Goals / Non-Goals

**Goals:**

- One favourite write, one menu rule, and one Favourites list shape for every Emby library
  destination in the GUI.
- Behaviour that matches the Emby client for which items qualify and what each library lists.

**Non-Goals:**

- A favourite mark on rows, bulk favouriting, Queue-row entries, Person or Live TV favourites,
  Audiobookshelf or Feed favourites, and live sync of another client's change.
- A Favourites view in the TUI, and any ctrl protocol, Owner, or capability change.
- A new dependency. None is added or upgraded.

## Decisions

**D1. `EmbyItem` carries `is_favorite` and one `supports_favourite` rule.** `parse_item` reads
`UserData.IsFavorite`. The field is `#[serde(default)]` because queue items serialize `EmbyItem`;
an older Owner ignores it and an old file loads as false. `EmbyItem::supports_favourite()` encodes
the Emby `canRate` type exclusions above and is the only place the rule lives. mbv items have
`UserData` whenever they come from a list fetch that asks for it; the rule does not re-test
presence. *Alternative:* test `UserData` presence on the item. Rejected: `EmbyItem` does not keep
the raw payload, and every mbv list fetch already asks for it.

**D2. Entry and action parallel to played state.** Two `ContextAction` variants, `Favourite(id)`
and `Unfavourite(id)`, labelled "Add to Favourites" / "Remove from Favourites". The entry is pushed
in the shared builder after the played-state entry (after Enqueue when the item has no
played-state entry, as Music has none), in both the folder and the leaf paths, and only for a
single row whose item `supports_favourite()`. The builder's `Queue` focus path does not push it
(the Owner holds queue items; a flipped flag would go stale and no ctrl variant is added). The TUI
gets the entry through the shared builder; it gets no view. Spelling in the UI is "Favourites".
*Alternative:* GUI-only entry. Rejected: the builder is shared by design, and a second builder
would break the "TUI and GUI cannot drift" rule the Home menu already follows.

**D3. The write mirrors the Emby web client, and Mark Played's execution.**
`EmbyClient::set_favourite(item_id, bool) -> Result<bool, EmbyError>` sends the two POSTs above and
returns the `IsFavorite` the reply carries. The App executes it like `context_set_played`: no Emby
client shows "Emby is unavailable"; an error shows a toast and changes nothing; success writes the
returned value. *Alternative:* `DELETE /FavoriteItems/{id}`. Rejected: only the POST form is what
the official client sends and what the user's server is known to serve; mbv uses `DELETE` only for
`PlayedItems`, a different resource.

**D4. After a success, flip the flag in every held copy by id, then refresh what is open.** A helper
in client-core sets `is_favorite` on matching `EmbyItem`s in every App-held copy: the TUI levels,
Home content — both the Continue Watching list and the `home_recently_added` store, whose rows
back the Home row menu's stable target — the series details the Shows tree reads (`series_detail_item`), the album track cache
(`album_tracks_cache`), and the artist detail cache. A helper in the GUI core does the same for the
GUI's own stores: the plain-library root lists, Shows New, the Music album store, and the
favourites store. The GUI cannot write App fields (shared-core rule 2), so App-held copies flip
only through the core helper. If the open GUI view is Favourites, its list is
fetched again so an unfavourite removes the row. Queue items are not touched (D2). *Alternative:*
refetch every list. Rejected: refetching a large library list for one flag is wasteful and
would reset the list's scroll and selection.

**D5. One generalised view type and dropdown.** `GuiShowsView` becomes `GuiLibraryView` with
variants All, New, Upcoming, Favourites, FavouriteShows, and FavouriteEpisodes.
`GuiLibraryView::choices(destination)` returns the Shows
set (All, New, Upcoming, FavouriteShows, FavouriteEpisodes) for Shows, (All, Favourites) for
every non-music Emby
destination, and nothing for a Grouped Music library: its header paints no view dropdown at all,
exactly as before this change. FavouriteShows and FavouriteEpisodes never apply outside a Shows
destination, because only a TV library holds Series and Episode rows. The per-library store (`shows_views`) becomes the library-view store keyed by library
index. `ShowsSetView` becomes a destination-neutral `LibrarySetView`. The dropdown component
becomes `LibraryViewDropdown`, still a thin supplier of rows to `LibraryDropdown` (invariant 38).
The launch state keeps `GuiLaunchShowsView` unchanged; saving maps Favourites to All, and only
Shows persists. *Alternative:* add `Favourites` to the Shows type and give Movies and Music their
own sibling type and dropdown. Rejected: it copies the same shape three times, which invariant 38
forbids. A Music or feed-group Emby header already paints a collection dropdown; it paints the view
dropdown beside it (the header's `collections` and `view` branches stop being exclusive). While the
view is Favourites the collection dropdown is hidden, because Favourites lists the whole library
and a collection pick does not filter it; pressing All paints it again with its pick intact.
The title line order is: Service mark, refresh, Expand all / Collapse all (Music All and Shows All
only), the collection dropdown (Music and feed-group, All view only), the view dropdown, then the
sort control (where the view paints one). The Favourites view paints no sort control, no letter
headings or rail, no tree buttons, and no collection dropdown. The refresh button in the
Favourites view refetches the favourites list, and the Library Status bar's location names
Favourites in place of the collection names.

**D6. One Favourites list, fetched per library, painted by one component.** The GUI core holds a
favourites store keyed by library id, holding the same request state type the New store uses
(`GuiSectionRequest`, with the in-place refresh set). The store holds every favourite the fetch
returns; the projection filters it by the active view: a Shows destination's FavouriteShows view
projects the Series rows, its FavouriteEpisodes view projects the Episode rows, and any other
view or destination projects the whole list. A favourite in a TV library that is neither a Series
nor an Episode shows in neither view, as Emby's own TV Favorites tab has no section for it. Failure follows the New store's rule exactly:
a failed first load lands an empty list, and a failed in-place refresh keeps the rows already held.
No spec scenario paints a failure state, so the list's frame part carries a `loading` flag and no
failure mark. The fetch runs on a worker and lands through
a new `LibraryFetchEvent::Favourites { library_id, result }`:

- every library: `Items?ParentId=<lib>&Recursive=true&Filters=IsFavorite` with the same `Fields` as
  `LATEST_FIELDS` (`UserData` included, invariant 19 does not apply: no root level is stamped);
- Music libraries only: also `/Artists?UserId&ParentId=<lib>&Filters=IsFavorite&Fields=UserData`,
  merged by id. A Grouped Music library never starts the fetch at all (D11): the merge exists
  for a non-grouped music library, the only music library that can open a Favourites view.

Client-side the list drops any item that fails `supports_favourite()` (Emby can return a favourited
Season), orders collections first (types that are folders) then media items, each by `SortName`
ascending with the existing Title sort key. No sort control is painted. The list's frame part
reuses `GuiEmbyRowFrame` (the rows the plain-library list already paints) and is carried as an
optional part on each of the Shows, Music, and plain-library list parts; a new
`LibraryFavouritesList` component paints it, using the shared flat-row parts. Every Favourites row
paints one Hero, built as the Home Hero builds an Emby item's: `hero_content_emby`,
`hero_facts_emby`, the declared artwork, and the cast, with no `hero_item` gate. A Series, BoxSet,
album, or artist favourite therefore paints a Hero, though the plain-library gate excludes folders
and non-Movies. The favourites selection is its own pick per library id, held beside the
favourites store; it never writes the All list's `picked_row`, so pressing All finds the All
selection unchanged. The Favourites list keeps its own `ListScroll` per library and never writes
the destination's All scroll store (Shows New's shared `scrolls.shows` is not the model).
Rows carry stable Service ids; row menu, select, play, shuffle, and enqueue resolve a favourites id
through the same lookup the other destinations use (`item_actions::library_item`), extended to read
the favourites store only while that library's active view is Favourites. *Alternative:* a favourites mode inside each destination's own list. Rejected:
three painters for one list.

**D7. Entering the view refetches; leaving it restores the All list untouched.** Setting the view to
Favourites starts the fetch (first load, or an in-place refresh when rows are already held). The
All lists, their selection, and their scroll belong to their own stores and are not edited, so
pressing All restores them (spec "A Favourites view replaces the destination's list column").

**D8. Music artists are listed by item id; the grouped tree offers no artist entry.** This
whole decision now applies only to a non-grouped music library, the one music library shape
that can list favourites (D11). Favourite
artist rows come from the `/Artists` listing and carry their artist item id, which is in the album
payload's id space. A favourite MusicArtist row's Play All, Shuffle, and Enqueue, from its row
controls and from its menu, fetch the artist's tracks with `get_artist_audio_tracks` (`ArtistIds`)
and play, shuffle, or append them in album, disc, and track order. The fetch is synchronous, as
`play_folder_gated` is. The GUI routes them: `run_library_menu_entry` catches `PlayFolder`,
`ShuffleFolder`, and `EnqueueFolder` for a MusicArtist target, and the row's play, shuffle, and
enqueue paths do the same. The core builder and the TUI do not change, and no path sends
`ParentId=<artist id>`. The doc comment on `get_artist_audio_tracks`, which says `/Artists` ids
have no proven equivalence, is updated to cite the probe above. No Music tree
artist row offers the favourites entry, whether keyed by Service id or by name: the tree's artist
rows are client-side groupings over albums, mbv holds no artist `EmbyItem` for them, and the entry
could not follow the item's current state without a fetch this change does not add. A favourite
artist row of the Favourites list itself carries a real item from the `/Artists` listing, so the
entry — and unfavouriting — works there. A tree artist row's id can equal a favourite artist's id;
because the favourites store resolves ids only while the view is Favourites (D6), the tree row
still resolves no item and offers no entry.

**D9. The TV library separates Favourite Shows and Favourite Episodes into two views.** Emby
paints Favorite Shows and Favorite Episodes as two sections of its TV Favorites tab. The GUI
follows that separation exactly, as two view dropdown entries, instead of merging both types
into one list. Inside each view the rows order by Title, and neither view paints a sort control.
*Alternative:* one Favourites view listing Series rows then Episode rows. Rejected: it merges
what Emby keeps separate, and the user separated them when planning this change.

**D10. Every non-music Emby Hero carries a heart toggle in its upper right.** The heart shows the
Hero item's favourite state (empty when not a favourite, filled when one) and a press toggles it
through the same write the menu entry runs (D3, D4), so the heart, the menu entry, and every held
copy flip together. The heart paints exactly when the same item's menu would offer the favourites
entry in that place (D11): the item supports favourites and the Hero does not belong to a Grouped
Music destination. A Hero whose item does not support favourites paints no heart, never a
disabled one. The heart is a GUI control; the TUI already reaches the same write through its
shared menu entry. *Alternative:* heart only in the Favourites views. Rejected: favouriting an
item the user is looking at is the primary use the control exists for, and restricting it to the
list of already-favourited items inverts it.

**D11. A Grouped Music library is excluded from favourites everywhere.** Its header paints no
view dropdown, its rows' menus push no favourites entry — artist, album, and track rows alike,
through every path that resolves the row inside that library — and its Heroes paint no heart.
The exclusion is contextual, not type-based: the same Audio item in a non-grouped music library
or a mixed plain library keeps its entry, because what the user excluded is the grouped Music
destination, not the music types. The shared builder learns the exclusion from the resolved
target's library context; a Home row is never grouped-music context and keeps its entry.
*Alternative:* exclude the music types everywhere. Rejected: it would strip the entry from
non-grouped music, which the user explicitly kept.

## Risks / Trade-offs

- [BoxSets sit outside a library's `ParentId`, so Movies Favourites may miss a favourite collection]
  → The server probed had no BoxSets, so this is unverified. The favourites fetch keeps the
  library's `ParentId` scope and adds nothing for BoxSets in this change; the Emby Movies Favorites
  tab itself lists Movies only, and the spec lists BoxSet as a collection type without promising
  one in the Movies view. The right-click entry on a BoxSet row works regardless, because it needs
  no library scope.
- [A flipped flag can disagree with another client] → Accepted (non-goal). The next refetch or
  reopening of the view shows the server's value.
- [Renaming `GuiShowsView` touches many files] → The rename is mechanical and lands in its own task
  before behaviour changes, so a compile pass proves it.
- [Music `/Artists` duplicates one name under several ids] → Rows are keyed by id, so duplicates
  show as separate rows, as the server holds them.
- [The `DELETE`-vs-`POST .../Delete` choice is only proven by Emby's client] → D3 uses the
  documented and client-sent form; a task's unit test pins the request the mock sees.

## Migration Plan

No data migration. `EmbyItem.is_favorite` and the launch state are additive and default to the old
behaviour. Rollback is a revert of the change; no server state is created by mbv except the user's
own favourite flags.
