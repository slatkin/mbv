//! The Library build (add-gui-library-panel design D2, split by
//! `gui-change-driven-sections` design D8): one gated pass that builds the three
//! Library sections, `libraryList`, `libraryDetail`, and `rowMenu`. Each
//! destination makes one App projection call, and that call feeds both its list
//! part and its detail part. The destination's ensures run inside the pass, so
//! a build starts the loads its sections wait on (design D12 of
//! add-gui-library-panel). The ledger compares each section on its own.

use mbv::{App, GuiDestinationKind, GuiLibraryTabs};

use crate::art::GuiArtRef;
use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiDestinationList, GuiEmbyDetail, GuiEmbyList,
    GuiFeedsDetail, GuiFeedsList, GuiHomeDetail, GuiHomeList, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryRowMenuFrame, GuiLibraryTabFrame, GuiMusicDetail, GuiMusicList, GuiPodcastsDetail,
    GuiPodcastsList, GuiShowsDetail, GuiShowsList,
};

/// The three Library sections one gated pass builds (design D8). The list and
/// the detail are `None` while no presented destination is active, and the row
/// menu goes with them, as the menu rode the content before the split.
#[derive(Debug, Default)]
pub(crate) struct LibraryPass {
    /// The `libraryList` section's value.
    pub(crate) list: Option<GuiLibraryList>,
    /// The `libraryDetail` section's value.
    pub(crate) detail: Option<GuiLibraryDetail>,
    /// The `rowMenu` section's value.
    pub(crate) row_menu: Option<GuiLibraryRowMenuFrame>,
}

/// The Library sections of the active destination (design D8): the tabs the top
/// bar paints with the destination's list part, the detail part, and the right-
/// click menu. Every section is `None` while no presented destination is active.
pub(crate) fn build(app: &mut App) -> LibraryPass {
    let destination = app.gui_active_destination();
    // The Podcasts destination's ensure (add-gui-library-panel design D12,
    // row 10.1): the selected pill's own loads start before any command does.
    if let Some(GuiDestinationKind::Podcasts { lib_idx }) = destination {
        app.ensure_gui_podcast_selection(lib_idx);
    }
    // The Feeds destination's ensure (row 11.1): the stored pill converges on
    // the subscription set the tab holds. The entries arrive from the shared
    // startup fetch, so the ensure starts no load.
    if matches!(destination, Some(GuiDestinationKind::Feeds)) {
        app.ensure_gui_feeds_selection();
    }
    let Some((list, detail)) = destination_parts(app, destination) else {
        return LibraryPass::default();
    };
    let GuiLibraryTabs { tabs, current } = app.gui_library_tabs();
    // The right-click item menu is the Library's, not one destination's (design
    // D8, D15): a row of any destination opens it.
    let row_menu = app.gui_library_row_menu();
    LibraryPass {
        list: Some(GuiLibraryList {
            tabs: tabs.into_iter().map(GuiLibraryTabFrame::from).collect(),
            current_tab: current,
            destination: list,
        }),
        detail: Some(detail),
        row_menu: row_menu.as_ref().map(GuiLibraryRowMenuFrame::from),
    }
}

/// The list and detail parts of the active destination, or `None` while no
/// presented destination is active. The match names every destination, so a new
/// destination forces a row here.
fn destination_parts(
    app: &mut App,
    destination: Option<GuiDestinationKind>,
) -> Option<(GuiDestinationList, GuiLibraryDetail)> {
    match destination {
        None => None,
        Some(GuiDestinationKind::Home) => Some(home_parts(app)),
        Some(GuiDestinationKind::Music { lib_idx }) => Some(music_parts(app, lib_idx)),
        Some(GuiDestinationKind::Shows { lib_idx }) => Some(shows_parts(app, lib_idx)),
        Some(GuiDestinationKind::Library { lib_idx }) => Some(emby_parts(app, lib_idx)),
        Some(GuiDestinationKind::Audiobooks { lib_idx }) => Some(audiobooks_parts(app, lib_idx)),
        Some(GuiDestinationKind::Podcasts { lib_idx }) => Some(podcasts_parts(app, lib_idx)),
        Some(GuiDestinationKind::Feeds) => Some(feeds_parts(app)),
    }
}

/// The Music destination's parts (design D5): the projection's list facts, and
/// the detail facts with the Hero's artwork, the picked item's menu, and the
/// playing track's id.
fn music_parts(app: &mut App, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    app.ensure_gui_music_selection(lib_idx);
    let projection = app.gui_music_projection(lib_idx);
    let picked = projection
        .hero
        .as_ref()
        .map(|hero| hero.album.album_id.clone());
    let art = picked
        .as_deref()
        .map(|album_id| GuiArtRef::from(&app.gui_library_artwork(album_id)));
    let menu = picked
        .map(|album_id| app.gui_library_menu(&album_id))
        .unwrap_or_default();
    let playing_track_id = app.gui_playing_track_id();
    (
        GuiDestinationList::Music(GuiMusicList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Music(GuiMusicDetail::from_projection(
            &projection,
            art,
            &menu,
            playing_track_id,
        )),
    )
}

/// The Home destination's parts (design D12).
fn home_parts(app: &mut App) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = app.gui_home_projection();
    (
        GuiDestinationList::Home(GuiHomeList::from_projection(&projection)),
        GuiLibraryDetail::Home(GuiHomeDetail::from_projection(&projection)),
    )
}

/// The Shows destination's parts (design D14). The ensure loads the selected
/// series' detail and its selected season's episodes, so the Hero's Workspace
/// paints on arrival.
fn shows_parts(app: &mut App, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    app.ensure_gui_shows_workspace(lib_idx);
    let projection = app.gui_shows_projection(lib_idx);
    (
        GuiDestinationList::Shows(GuiShowsList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Shows(GuiShowsDetail::from_projection(&projection)),
    )
}

/// The plain Emby-library destination's parts (design D12, row 8.1). The
/// ensure is the shown-pass trigger the TUI projection pass's lazy-fetch arm
/// mirrors.
fn emby_parts(app: &mut App, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    app.ensure_gui_emby_selection(lib_idx);
    let projection = app.gui_emby_projection(lib_idx);
    (
        GuiDestinationList::Emby(GuiEmbyList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Emby(GuiEmbyDetail::from_projection(&projection)),
    )
}

/// The Audiobooks destination's parts (design D12, row 9.1). The ensure is the
/// default pick's detail-load trigger.
fn audiobooks_parts(app: &mut App, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    app.ensure_gui_abs_selection(lib_idx);
    let projection = app.gui_audiobooks_projection(lib_idx);
    (
        GuiDestinationList::Audiobooks(GuiAudiobooksList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Audiobooks(GuiAudiobooksDetail::from_projection(&projection)),
    )
}

/// The Podcasts destination's parts (design D12, row 10.1). The build's ensure
/// above is the selected pill's load trigger.
fn podcasts_parts(app: &mut App, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = app.gui_podcasts_projection(lib_idx);
    (
        GuiDestinationList::Podcasts(GuiPodcastsList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Podcasts(GuiPodcastsDetail::from_projection(&projection)),
    )
}

/// The Feeds destination's parts (design D12, row 11.1). The entries arrive
/// from the shared startup fetch, so the build starts no load.
fn feeds_parts(app: &mut App) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = app.gui_feeds_projection();
    (
        GuiDestinationList::Feeds(GuiFeedsList::from_projection(&projection)),
        GuiLibraryDetail::Feeds(GuiFeedsDetail::from_projection(&projection)),
    )
}

/// The Library sections' revisions and the active destination's shape, logged
/// when either section's revision changes (the GUI log names the selection's
/// shape and the list's size, so an empty list is visible in a live session).
pub(crate) fn log_built(
    list_revision: u64,
    detail_revision: u64,
    list: Option<&GuiLibraryList>,
    detail: Option<&GuiLibraryDetail>,
) {
    let (kind, pills, rows) = list.map_or(("none", 0, 0), |list| list_size(&list.destination));
    let hero = detail.is_some_and(detail_has_hero);
    tracing::info!(
        name: "gui_library.content.built",
        target: "gui_library",
        list_revision,
        detail_revision,
        kind,
        pills,
        rows,
        hero,
        "the Library sections changed"
    );
}

/// The destination's kind word, its pill count, and its row count, for the log.
fn list_size(list: &GuiDestinationList) -> (&'static str, usize, usize) {
    match list {
        GuiDestinationList::Music(list) => ("music", list.pills.len(), list.artists.len()),
        GuiDestinationList::Home(list) => ("home", 0, list.rows.len()),
        GuiDestinationList::Shows(list) => ("shows", list.pills.len(), list.rows.len()),
        GuiDestinationList::Emby(list) => ("emby", list.pills.len(), list.rows.len()),
        GuiDestinationList::Audiobooks(list) => ("audiobooks", 0, list.rows.len()),
        GuiDestinationList::Podcasts(list) => ("podcasts", list.pills.len(), list.rows.len()),
        GuiDestinationList::Feeds(list) => ("feeds", list.pills.len(), list.rows.len()),
    }
}

/// Whether the detail carries a Hero, for the log.
fn detail_has_hero(detail: &GuiLibraryDetail) -> bool {
    match detail {
        GuiLibraryDetail::Music(detail) => detail.hero.is_some(),
        GuiLibraryDetail::Home(detail) => detail.hero.is_some(),
        GuiLibraryDetail::Shows(detail) => detail.hero.is_some(),
        GuiLibraryDetail::Emby(detail) => detail.hero.is_some(),
        GuiLibraryDetail::Audiobooks(detail) => detail.hero.is_some(),
        GuiLibraryDetail::Podcasts(detail) => detail.hero.is_some(),
        GuiLibraryDetail::Feeds(detail) => detail.hero.is_some(),
    }
}
