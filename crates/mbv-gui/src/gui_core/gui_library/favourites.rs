//! The GUI Library's Favourites data path (emby-favourites design D6, D7, D8):
//! one store per library id, the fetch that fills it, the list it projects,
//! the pick the Favourites Hero shows, and the artist actions a favourite
//! artist row runs.
//!
//! The store is the New store's shape (design D6): a request that is absent
//! before it starts, `Requesting` while a worker fetch runs, and `Landed` with
//! the rows once it ends. A failed first load lands an empty list, and a failed
//! in-place refresh keeps the rows already held, so no list paints a failure
//! state — the frame part carries a `loading` flag and no failure mark.
//!
//! Entering the view starts the fetch (design D7): a first load when the store
//! holds no list, an in-place refresh when it does. Every step is a free
//! function over the `App` the GUI core owns and the GUI Client's state beside
//! it, with the `GuiCore` entries as one-line calls (design D4).

use mbv_client_core::dispatch::notify::ToastSeverity;
use mbv_emby_model::EmbyItem;
use mbv_ui_model::sort_filter::effective_sort_str;
use mbv_ui_model::sort_filter::strip_article;
use mbv_ui_model::ui_util::{is_playable, natural_sort_key};

use super::GuiEmbyHero;
use super::GuiEmbyRow;
use super::GuiGroupAlbums;
use super::emby::GuiEmbyRootItems;
use super::library_view::GuiLibraryView;
use super::list_sort::gui_title_sort_key;
use super::request::GuiSectionRequest;
use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;
use mbv_client_core::state::library_fetch_events::LibraryFetchEvent;

/// The Favourites list of one Emby library (design D6), as the destination's
/// list part carries it while its view is Favourites.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiFavouritesProjection {
    /// Whether the list has not landed yet: the store holds no landed request.
    /// A failed first load lands an empty list, so this is never a failure
    /// mark.
    pub loading: bool,
    /// The rows in paint order: the collections first, then the media items,
    /// each group in Title order (design D6, D9).
    pub rows: Vec<GuiEmbyRow>,
    /// The row the Favourites Hero shows: the list's own pick while the rows
    /// still carry it, else the first row — the Hero default, so a Hero paints
    /// as soon as the list lands (design D6).
    pub selected: Option<String>,
    /// The picked row's Hero facts; `None` while no row is picked (design D6).
    pub hero: Option<GuiEmbyHero>,
}

impl GuiCore {
    /// The Favourites list of `lib_idx` (design D6), for a caller that holds
    /// the GUI core.
    #[must_use]
    pub fn gui_favourites_projection(&mut self, lib_idx: usize) -> GuiFavouritesProjection {
        favourites_projection(&mut self.app, &self.gui, lib_idx)
    }

    /// Open `lib_idx`'s Favourites view again (design D7): a destination
    /// opened on its stored Favourites view fetches that list again, so a
    /// favourite added elsewhere shows on the next opening. It is the open
    /// step's work, never a per-frame one: a landed list would refetch on every
    /// paint if the frame path called it.
    pub(in crate::gui_core::gui_library) fn refetch_open_favourites(&mut self, lib_idx: usize) {
        refetch_favourites(&mut self.app, &mut self.gui, lib_idx);
    }
}

/// The Favourites list of `lib_idx` (design D6, D9): the store's rows,
/// collections first and each group in Title order, the list's own pick, and
/// the picked row's Hero. A `lib_idx` that names no Emby library projects the
/// empty list.
pub(in crate::gui_core::gui_library) fn favourites_projection(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
) -> GuiFavouritesProjection {
    let Some(library_id) = app.emby_library_id(lib_idx) else {
        return GuiFavouritesProjection::default();
    };
    let (items, loading) = match gui.favourites.get(library_id) {
        Some(GuiSectionRequest::Landed(items)) => (items.as_slice(), false),
        Some(GuiSectionRequest::Requesting) | None => (&[][..], true),
    };
    let mut items: Vec<&EmbyItem> = items.iter().collect();
    // The Shows destination's two favourite views project only their own
    // type (design D9): Favourite Shows the Series rows, Favourite Episodes
    // the Episode rows. Every other view and destination projects the whole
    // list. A favourite in a TV library that is neither shows in neither
    // view, as Emby's own TV Favorites tab has no section for it.
    match super::library_view::library_view(gui, lib_idx) {
        GuiLibraryView::FavouriteShows => items.retain(|item| item.item_type == "Series"),
        GuiLibraryView::FavouriteEpisodes => items.retain(|item| item.item_type == "Episode"),
        _ => {}
    }
    // Collections first, then the media items, each group in the existing
    // Title order (design D6, D9): a TV library therefore lists its Series
    // before its Episodes.
    items.sort_by_cached_key(|item| {
        (
            !favourites_collection(item),
            gui_title_sort_key(effective_sort_str(item)),
            item.id.clone(),
        )
    });
    let rows: Vec<GuiEmbyRow> = items
        .iter()
        .map(|item| super::emby_projection::favourites_row(app, item))
        .collect();
    let selected = gui
        .favourites_picks
        .get(library_id)
        .filter(|pick| rows.iter().any(|row| &row.id == *pick))
        .cloned()
        .or_else(|| rows.first().map(|row| row.id.clone()));
    let hero = selected
        .as_deref()
        .and_then(|selected| items.iter().find(|item| item.id == selected))
        .map(|item| super::emby_projection::emby_hero(app, item));
    GuiFavouritesProjection {
        loading,
        rows,
        selected,
        hero,
    }
}

/// Whether a favourite row belongs to the list's collections group (design
/// D6, D9): the items that hold items — a Series, a `BoxSet`, a music artist, a
/// music album — take the same rule the list's row kind takes.
fn favourites_collection(item: &EmbyItem) -> bool {
    super::emby_projection::row_kind(item) == super::GuiEmbyRowKind::Collection
}

/// Start `lib_idx`'s favourites fetch (design D7): entering the view, the
/// Favourites view's refresh control, the destination's open step, and a
/// favourite write's landing all call this. With no request held the fetch
/// takes the first-load path, and with a landed list it keeps that list in
/// place and starts the in-place refresh. No Emby client starts nothing at all,
/// and the no-client condition is not a fetch failure: no mark is stored.
pub(crate) fn refetch_favourites(app: &mut App, gui: &mut GuiClientState, lib_idx: usize) {
    let Some(client) = app.emby_snapshot() else {
        return;
    };
    let Some(library_id) = app.emby_library_id(lib_idx).map(str::to_string) else {
        return;
    };
    // The `/Artists` merge (design D6) is a music library's, grouped or not:
    // a Grouped Music library never reaches this call at all (design D11),
    // so the flag names the library's type, not its presentation.
    let music = app
        .emby_library(lib_idx)
        .is_some_and(|lib| lib.library.collection_type == "music");
    let refreshing = gui.favourites_refreshing.contains(&library_id);
    match favourites_fetch(gui.favourites.get(&library_id), refreshing) {
        FavouritesFetch::Hold => return,
        FavouritesFetch::Refresh => {
            gui.favourites_refreshing.insert(library_id.clone());
        }
        FavouritesFetch::FirstLoad => {
            // The requesting mark blocks a second fetch; the completion
            // replaces it with landed rows, empty when the fetch fails.
            gui.favourites
                .insert(library_id.clone(), GuiSectionRequest::Requesting);
        }
    }
    start_favourites_fetch(app, client, library_id, music);
}

/// What one favourites refetch call starts (design D6, D7): the first load
/// while the store holds no request, the in-place refresh while it holds a
/// landed list and none is in flight, and nothing while a first load or a
/// refresh is already in flight. Both in-flight marks hold, so a duplicate
/// first load can never land empty after a successful worker landed rows.
pub(crate) fn favourites_fetch(
    state: Option<&GuiSectionRequest<EmbyItem>>,
    refreshing: bool,
) -> FavouritesFetch {
    match state {
        None => FavouritesFetch::FirstLoad,
        Some(GuiSectionRequest::Requesting) => FavouritesFetch::Hold,
        Some(GuiSectionRequest::Landed(_)) if refreshing => FavouritesFetch::Hold,
        Some(GuiSectionRequest::Landed(_)) => FavouritesFetch::Refresh,
    }
}

/// The work one favourites refetch call starts (design D6, D7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FavouritesFetch {
    /// Start the first load: the store holds no request.
    FirstLoad,
    /// Start the in-place refresh: a landed list is held and none is in
    /// flight.
    Refresh,
    /// Start nothing: a first load or a refresh is already in flight.
    Hold,
}

/// Start one library's favourites fetch worker (design D6): the one spawn body
/// the first load and the refetch share, so both send the same result event,
/// whose landing replaces the store's value or reports a failed refresh.
fn start_favourites_fetch(
    app: &App,
    client: mbv_emby::EmbyClient,
    library_id: String,
    music: bool,
) {
    let tx = app.library_fetch_event_sender();
    std::thread::spawn(move || {
        let result = client.get_favourites(&library_id, music);
        tx.send(LibraryFetchEvent::Favourites { library_id, result });
    });
}

/// Flip this Client's own copies of `item_id` after a favourite write landed,
/// then refetch the favourites list of a library whose open view is Favourites
/// and whose list holds the row (emby-favourites design D4): the plain
/// library's root lists, the Shows New rows, the Music album store, and the
/// favourites store take the new value by id, and an unfavourited row leaves an
/// open Favourites list through its refetch. App-held copies are the core
/// helper's work; this is the GUI's own state.
pub(crate) fn apply_favourite_change(
    app: &mut App,
    gui: &mut GuiClientState,
    item_id: &str,
    favourite: bool,
) {
    let flip = |items: &mut [EmbyItem]| {
        for item in items {
            if item.id == item_id {
                item.is_favorite = favourite;
            }
        }
    };
    for roots in gui.emby_roots.values_mut() {
        if let GuiEmbyRootItems::Loaded(items) = roots {
            flip(items);
        }
    }
    for request in gui.shows_new.values_mut() {
        if let GuiSectionRequest::Landed(items) = request {
            flip(items);
        }
    }
    for groups in gui.music_albums.values_mut() {
        for state in groups.values_mut() {
            if let GuiGroupAlbums::Loaded(items) = state {
                flip(items);
            }
        }
    }
    for request in gui.favourites.values_mut() {
        if let GuiSectionRequest::Landed(items) = request {
            flip(items);
        }
    }
    let open: Vec<usize> = gui
        .library_views
        .iter()
        .filter(|(_, view)| **view == GuiLibraryView::Favourites)
        .map(|(lib_idx, _)| *lib_idx)
        .filter(|lib_idx| {
            app.emby_library_id(*lib_idx)
                .and_then(|library_id| gui.favourites.get(library_id))
                .is_some_and(|request| request.items().iter().any(|item| item.id == item_id))
        })
        .collect();
    for lib_idx in open {
        refetch_favourites(app, gui, lib_idx);
    }
}

/// The favourite item `item_id` names in `library_id`'s Favourites list, or
/// `None` while that list does not hold it (design D6). It reads the store
/// only, so a stale frame's id resolves nothing.
pub(in crate::gui_core::gui_library) fn favourites_item(
    gui: &GuiClientState,
    library_id: &str,
    item_id: &str,
) -> Option<EmbyItem> {
    gui.favourites
        .get(library_id)?
        .items()
        .iter()
        .find(|item| item.id == item_id)
        .cloned()
}

/// Select a Favourites row (design D6): the list's own pick, by library id, so
/// the Favourites Hero shows the row and the All list's pick is left as it was.
/// `false` when the view is not Favourites or the list does not hold the row,
/// so the caller's own stale arm runs.
pub(in crate::gui_core::gui_library) fn select_favourites_row(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    item_id: &str,
) -> bool {
    if !super::library_view::paints_favourites(gui, lib_idx) {
        return false;
    }
    let Some(library_id) = app.emby_library_id(lib_idx).map(str::to_string) else {
        return false;
    };
    // The store answers whether it holds the row; the item's own copy is not
    // needed here, so a press copies no row.
    let holds = gui
        .favourites
        .get(&library_id)
        .is_some_and(|request| request.items().iter().any(|item| item.id == item_id));
    if !holds {
        return false;
    }
    if gui.favourites_picks.get(&library_id).map(String::as_str) == Some(item_id) {
        return true;
    }
    gui.favourites_picks.insert(library_id, item_id.to_string());
    // A pick move is library navigation: it closes the image warm gate, so
    // fast arrow traffic warms no neighbour art (the TUI's own rule).
    app.stamp_navigation();
    true
}

/// Play or shuffle a favourite music artist row's tracks (design D8): the
/// artist's tracks from the synchronous `ArtistIds` fetch, in album, disc, and
/// track order, replace the queue — shuffled for the shuffle entry. No path
/// sends `ParentId=<artist id>`.
pub(in crate::gui_core::gui_library) fn play_favourite_artist(
    app: &mut App,
    artist_id: &str,
    shuffle: bool,
) {
    let Some(tracks) = artist_tracks(app, artist_id, shuffle) else {
        return;
    };
    app.play_artist_tracks_from_start(tracks, shuffle);
}

/// Append a favourite music artist row's tracks to the queue's tail (design
/// D8): the same synchronous fetch and order the play runs, appended through
/// the shared plural add path.
pub(in crate::gui_core::gui_library) fn enqueue_favourite_artist(
    app: &mut App,
    lib_idx: usize,
    artist_id: &str,
) {
    let Some(tracks) = artist_tracks(app, artist_id, false) else {
        return;
    };
    app.enqueue_lib_items(lib_idx, tracks);
}

/// The artist's playable tracks in album, disc, and track order (design D8),
/// or `None` with the reason flashed: a missing Emby client, a failed fetch,
/// and an artist with no playable track each flash and act on nothing. The
/// fetch is synchronous, as `play_folder_gated` is.
fn artist_tracks(app: &mut App, artist_id: &str, shuffle: bool) -> Option<Vec<EmbyItem>> {
    let Some(client) = app.emby_snapshot() else {
        app.flash("Emby is unavailable".into(), ToastSeverity::Warning);
        return None;
    };
    let tracks = match client.get_artist_audio_tracks(artist_id) {
        Ok(tracks) => ordered_artist_tracks(tracks),
        Err(error) => {
            app.flash(
                format!("Couldn't load the artist's tracks: {error}"),
                ToastSeverity::Error,
            );
            return None;
        }
    };
    if tracks.is_empty() {
        app.flash(
            if shuffle {
                "Nothing to shuffle"
            } else {
                "Nothing to play"
            }
            .into(),
            ToastSeverity::Error,
        );
        return None;
    }
    Some(tracks)
}

/// The artist's tracks in album, disc, and track order (design D8): the
/// playable tracks grouped by album in the Music list's own album order
/// (release year, then name), and each album's disc and track order, with the
/// track's own sort key as the last tie-break. The `ArtistIds` query returns
/// the artist's whole catalogue, so the album order is the list's, not the
/// server's reply order.
fn ordered_artist_tracks(tracks: Vec<EmbyItem>) -> Vec<EmbyItem> {
    let mut tracks: Vec<EmbyItem> = tracks.into_iter().filter(is_playable).collect();
    tracks.sort_by_cached_key(|track| {
        (
            track.production_year == 0,
            track.production_year,
            natural_sort_key(strip_article(&track.album)),
            track.album_id.clone(),
            track.parent_index_number,
            track.index_number,
            natural_sort_key(track.sort_key()),
            track.id.clone(),
        )
    });
    tracks
}
