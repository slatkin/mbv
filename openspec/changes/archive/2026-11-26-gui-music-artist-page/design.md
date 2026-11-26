# Design

## Context

See `proposal.md` for the motivation and the behavior. This design is for the GUI Client only. The facts below are what the code holds now.

- The Music Hero is always an album. `GuiMusicHero` (`crates/mbv-gui/src/gui_core/gui_library/projection.rs`) holds the artist header facts and one `album`. `music_hero` resolves the picked album from the kept pick of the one checked group (`library_picks`) or from the session union pick (`music_union_picks`), and it falls back to the first album, so the Hero is never blank.
- The list's press on an artist row calls `show_detail(None)` (`window/library/state/rows.rs`, `picked_item_id`), so it sends no command. The core's `ShowDetail` takes an item id and stores an album pick. Only albums and tracks name an album id.
- The window's Music list marks the Hero's album row as the picked row (`components/library_list.rs`) and reveals it once per new pick (`reveal_picked_album`: it opens the album's artist and scrolls).
- `GuiMusicList.artists` is the union of the checked groups, one `GuiMusicArtist` per `ArtistKey`, and each artist holds its `GuiMusicAlbum`s with their `tracks` (`Option`) once the album's tracks are in `App::album_tracks_cache`. `GuiMusicHero.artist_tracks` already totals the artist's tracks once every album of the artist is cached.
- The GUI holds no settled-catalog revision, so the TUI's artist-id track query (`request_artist_tracks`, keyed by the catalog revision) does not fit. The core already has the bounded per-album fan-out for the TUI's fallback artist (`enqueue_artist_album_tracks`, at most 6 requests at a time), but it is `pub(in crate::app)`.
- The core already has `play_music_albums(albums, shuffle, confirm)` (a blocking walk of the albums in order), `enqueue_lib_item` (a blocking, in-order folder append for an album), and `play_artist_tracks(tracks, start_idx)` (the TUI's play-from-track for an artist Workspace).
- The Hero artwork path fetches and resolves any Emby source: `ArtworkSource::Emby { item_id, series_id, image_types, cache_key }` through `home_hero_artwork`. `ArtistNameLookup::item_id` (core) resolves the Service artist item of a name-grouped (fallback) artist, as the row art does.
- The Hero tracklist rows send `play_library_track` for a double-click and the hover Play, and the key route (`keys.rs`, `hovered_item_id`) sends `Play { item_id }` for Enter on a `PointerList::Tracklist` marker. The marker carries the track id.

No new or upgraded dependency. No Cargo manifest changes.

## Goals / Non-Goals

**Goals:**

- A press on an artist row shows the artist page in the Hero, on every arrangement that paints a Hero.
- The page behaves as the TUI artist root detail does: artist artwork, name, album count and year span, tracks grouped by album, and play-from-track through the artist's remaining tracks.
- Keep the GUI rules: the window never touches `App`, a command names stable ids, no cursor.

**Non-Goals:**

- No change to the TUI, the Owner, or any ctrl or wire protocol.
- No saved artist page, and no change to the launch location file.
- No press target on an album label, and no More actions on the artist page.
- No new Service call: the track loads are the existing per-album fetches.

## Decisions

### D1. The artist page is a session pick, and the Hero has a subject

`GuiClientState` gets `music_artist_pages: HashMap<usize, ArtistKey>`, keyed by library index. It is session state, not saved. The Music projection reads it after the album pick:

- If the entry for the library holds an `ArtistKey` that the current union still holds, the Hero's subject is the artist page for that artist.
- Otherwise the Hero's subject is the album, resolved as now.

The entry is cleared by the two album-picking arms (`show_library_detail`, the existing `ShowDetail` handler) and by the settings reset (`gui_settings/reset.rs`, next to `library_picks.clear()`). A collection change does not clear it. The union test above is the one rule that drops it (the spec scenarios "A selection that drops the artist" and "A selection that keeps the artist"). The kept album picks (`library_picks`, `music_union_picks`) and the launch file do not change: a pick of an artist writes no album pick, so the group keeps the album shown before the page.

`GuiMusicHero` changes shape. Its `album: GuiMusicAlbum` field becomes `subject: GuiMusicHeroSubject`:

```rust
pub enum GuiMusicHeroSubject {
    Album(GuiMusicAlbum),
    Artist(GuiMusicArtistPage),
}
pub struct GuiMusicArtistPage {
    pub artist_key: ArtistKey,
    pub albums: Vec<GuiMusicAlbum>,      // the union's albums of the artist, tracks when cached
    pub years: Option<(String, String)>, // first and last release year of the albums that name one
}
```

The header fields (`artist`, `collections`, `album_count`, `artist_tracks`) stay on `GuiMusicHero` and read from the page's artist the same way they read from the album's artist now. The frame mirror `GuiMusicHeroFrame` and `From<&GuiMusicHero>` take the same shape.

Alternatives considered: keep `album` and add `artist_page: Option<…>`. It keeps churn low, but then the Hero holds an album of an unrelated artist while the page shows, and every reader must remember to ignore it. An enum makes the compiler list the readers (`library_hero.rs`, `status.rs`, `library_list.rs`, `rows.rs`), which is the point of a subject. Persisting the page in `library_picks`: it holds album ids only, and a fallback artist has no Service id, so it would need a second key kind in the launch file for a view the user does not expect to survive a restart.

### D2. Four new Library commands, each a named arm in every pass

The command set gets four variants, mirrored in `bridge/library.rs`:

- `ShowArtist { lib_idx, artist_key }`: store the page pick (D1) and start the track loads (D3).
- `PlayArtist { album_ids }`: `App::play_music_albums(albums, false, false)` over the albums, in the given order (the existing blocking walk, the same one Shuffle artist runs with `shuffle` true).
- `AddArtistToQueue { album_ids }`: for each album in order, the existing add path (`add_library_item`), whose album arm is `enqueue_lib_item` (a blocking, in-order folder append).
- `PlayArtistTrack { album_ids, track_id }`: the core builds the sequence from the album track cache (D4), then calls `play_artist_tracks(tracks, start_idx)`.

`ShuffleArtist { album_ids }` stays as it is. The window names album ids from the carried union (the `artist_union_albums` pattern), so the core never trusts a position. An id the store no longer holds is skipped and logged, as `shuffle_artist` does today.

Every pass in `gui_library.rs` and the four matches in `destination.rs` name every variant, with no wildcard (the repo rule). The new variants take their arms in the routing pass of `dispatch_gui_library_command`. The other passes list them in their ignored-arm lists. This is the wiring list the tasks follow.

### D3. Track loads: one new core intent, and one step in the Music ensure pass

`App` gets `pub fn request_albums_tracks(&mut self, album_ids: Vec<String>)`, a thin intent method over `enqueue_artist_album_tracks` (kept as the one implementation, bounded at 6 in flight, deduplicated against the cache and the pending queue). The core names no Client in it.

The loads start in two places. `ShowArtist` calls it for the albums of the picked artist. The Music destination's ensure pass (`MusicLoads`, "the content that depends on the selection", the requirement "Every Library destination loads its content from the frame") calls it for the albums of the shown artist page, so a page that a collection change widened loads the new albums too. Both calls are idempotent: an album already cached, loading, or queued starts nothing, so a repaint starts no second request.

Alternative considered: the TUI's artist-id query (`request_artist_tracks`). It returns all tracks in one request, but its cache key holds the settled catalog revision, which the GUI's union does not have. Faking a revision would add a second cache path for a view that the per-album cache already serves. The cost of the choice: an artist with many albums needs one request per album, at most 6 at a time.

### D4. The played sequence is built in the core from the caches

`PlayArtistTrack` builds the sequence the page shows: for each id in `album_ids`, in order, the album's cached tracks that are playable, sorted in disc and track order (`sort_audio_tracks`), concatenated. An album with no cached tracks adds nothing. The start index is the position of `track_id` in the result. A `track_id` not found logs and plays nothing. The call is `play_artist_tracks`, which replaces the queue through the Album queue source, as the TUI does.

The sequence matches the TUI's: its Workspace sequence also holds only the groups with tracks. A later album that has not loaded yet is not in the queue. The page loads all albums when it is picked, so this is a short window.

### D5. Artist artwork uses the Hero's art path

`music_parts` in `crates/mbv-gui/src/sections/library.rs` computes the Hero's art and menu from the Hero's album id today. It branches on the subject. For the artist page, a new `GuiCore::gui_artist_artwork(&ArtistKey)` returns the art, and the menu is empty (the page has no More actions).

For a Service artist the art source is `ArtworkSource::Emby { item_id: <artist id>, series_id: "", image_types: ["Primary"], cache_key: mbv_images::emby_card_cache_key(<artist id>, "") }`, fetched and resolved by `home_hero_artwork`. For a fallback artist the id comes from `ArtistNameLookup::item_id` with the artist name, as the row art does; with no id the art is the placeholder. The Hero keeps the square art box that the album uses (the window's `object-fit: cover` crops a portrait or landscape image), so the artist branch adds no shape logic. The album's art is not fetched while the page shows.

### D6. The window

- `LibraryRow::Artist` carries the artist's `GuiArtistKey` (it has `base.id` that encodes it now; the row gets the typed key so the press need not parse an id). `press_library_row` for an artist sends `ShowArtist { lib_idx, artist_key }` with `lib_idx` from `GuiMusicList.lib_idx`, and closes the item menu as `show_detail` does. It opens and closes nothing.
- The list's picked row: the picked row id is the artist's row id while the subject is the artist page, and the album's row id otherwise. `components/library_list.rs` reads it from a helper on `GuiMusicHeroFrame` (`picked_row_id`), so the two lines that read `hero.album.album_id` today call one function.
- The reveal rule follows the subject: an album pick opens its artist and scrolls (as now); an artist pick scrolls the artist row into view and does not open it. The "revealed once" record keys on the picked row id.
- `LibraryHero` (`components/library_hero.rs`) branches on the subject. The album branch is unchanged. The artist branch paints `LibraryHeroFrame` with: the header (kicker, artist name, the artist facts that include the year span), the Shuffle artist header action, a block with the artist's name as its label and no facts line (the header's facts line carries the year span), the actions row with Play and Add to queue and no More actions, the artist art in the shape of D5, and a new child, `LibraryArtistTracklist`.
- `LibraryArtistTracklist` (a new component beside `library_tracklist.rs`) paints, for each album of the page, an album label (name and year) and then the album's tracks through the one shared Workspace table, as `LibraryTracklist` does. It reuses the table's row, hover controls, and pointer marker (`PointerList::Tracklist`). A row's hover Add sends `AddToQueue { item_id: track_id }` (one track). A row's Play and a double-click call `LibraryState::play_library_track`.
- `LibraryState::play_library_track(track_id)` reads the carried Music detail. While the subject is the artist page it sends `PlayArtistTrack { album_ids, track_id }` with the page's album ids. Otherwise it sends `Play { item_id: track_id }` as now. The key route's `PointerList::Tracklist` arm calls `play_library_track` for Enter (it calls `play_library_item` for `PointerList::Music`), so Enter, the hover Play, and the double-click share one path (the spec scenario "Enter plays the hovered artist page track").
- The status bar's location line reads the picked name from the Hero (`picked_name(music_hero(detail))`). On an artist page the picked name is the artist's name. A helper on `GuiMusicHeroFrame` returns it, so the status code does not match on the subject.
- `LibraryHero` Play and Add to queue send `PlayArtist` and `AddArtistToQueue` on the artist branch, with the album ids of the page. While the page has albums that are still loading, they still play, because the commands name the albums and the core's walk fetches what it needs.

Alternative considered: a press on an album label opens that album. It gives a short path from the page to an album, but the label would be a pressable heading, and `CONTEXT.md` keeps a Group heading a non-selectable label. The album rows under the open artist already give that path.

### D7. Docs and the glossary

`docs/architecture/gui-client.md` (the Music section that lists `GuiMusicHeroFrame` fields and the Hero's behavior) names the subject and the artist page. `CONTEXT.md` gets one term, **Artist page** (GUI): the Hero state for a picked Music artist, named so it is not confused with the TUI's artist root detail. Its *Avoid* line holds artist detail page, artist screen, and artist hero.

### D8. Tests (contracts, by owner)

Per `docs/invariants/14-test-ownership.md` and `crates/mbv-gui/AGENTS.md`: headless, no real mpv, no sleeps, one contract per test, and an existing table extends before a new test starts.

- Core projection (`gui_core/tests`): the Hero subject. One `#[case]` table on the resolution rule: no page pick → album; a page pick for an artist in the union → artist page; a page pick for an artist not in the union → album; and an album pick after a page pick → album.
- Core commands (`gui_core/tests`): `PlayArtistTrack` builds the sequence that spans the albums in order and starts at the track (this is the contract of the spec scenario "A track is played from the artist page"); `ShowArtist` starts the loads for each album of the artist and none for a cached album.
- Bridge: one conversion test for the frame's subject.
- Window: one painted-structure test for the artist branch of `LibraryHero` (the album labels, the tracks per album, no More actions control); one test for the press (an artist row sends `ShowArtist` and does not toggle the row); one test that the key route's tracklist arm sends `PlayArtistTrack` while the page shows.

## Risks / Trade-offs

- [An artist with many albums issues many requests] → The existing bound of 6 in flight limits the burst, and each arrival frees a slot.
- [A track played before later albums have loaded drops those albums from the queue] → The page loads every album at the pick. The TUI has the same rule, and the spec says the sequence is what the page shows.
- [`GuiMusicHero.album` readers break] → The enum makes each reader a compile error. The tasks list the known readers, and the first unit is allowed to leave the build broken for the next.
- [Two track-play paths could drift] → `play_library_track` is the one entry the double-click, the hover Play, and Enter share.
- [The page gives no path to an album from the label] → Accepted for now (see D6's alternative). The album rows under the open artist hold the path.

## Migration Plan

No data migration. The launch file is unchanged. The change ships as one GUI release. Rollback is a revert.
