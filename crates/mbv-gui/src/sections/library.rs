//! The Library build (add-gui-library-panel design D2, split by
//! `gui-change-driven-sections` design D8): one gated pass that builds the three
//! Library sections, `libraryList`, `libraryDetail`, and `rowMenu`. Each
//! destination makes one App projection call, and that call feeds both its list
//! part and its detail part. The active destination's load pass runs before the
//! parts, so a build starts the loads its sections wait on (design D12 of
//! add-gui-library-panel; gui-one-destination-ensure design D2). The ledger
//! compares each section on its own.

use crate::gui_core::{GuiCore, GuiDestinationKind, GuiLibraryTabs};

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

/// The Library sections of the active destination (design D8;
/// gui-one-destination-ensure design D2): the tabs the top bar paints with the
/// destination's list part, the detail part, and the right-click menu. Every
/// section is `None` while no presented destination is active. The
/// destination's one load pass runs before the parts are built, so a part never
/// starts a load of its own.
pub(crate) fn build(core: &mut GuiCore) -> LibraryPass {
    let destination = core.ensure_gui_active_destination();
    let Some((list, detail)) = destination_parts(core, destination) else {
        return LibraryPass::default();
    };
    let GuiLibraryTabs { tabs, current } = core.gui_library_tabs();
    // The right-click item menu is the Library's, not one destination's (design
    // D8, D15): a row of any destination opens it.
    let row_menu = core.gui_library_row_menu();
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
    core: &mut GuiCore,
    destination: Option<GuiDestinationKind>,
) -> Option<(GuiDestinationList, GuiLibraryDetail)> {
    match destination {
        None => None,
        Some(GuiDestinationKind::Home) => Some(home_parts(core)),
        Some(GuiDestinationKind::Music { lib_idx }) => Some(music_parts(core, lib_idx)),
        Some(GuiDestinationKind::Shows { lib_idx }) => Some(shows_parts(core, lib_idx)),
        Some(GuiDestinationKind::Library { lib_idx }) => Some(emby_parts(core, lib_idx)),
        Some(GuiDestinationKind::Audiobooks { lib_idx }) => Some(audiobooks_parts(core, lib_idx)),
        Some(GuiDestinationKind::Podcasts { lib_idx }) => Some(podcasts_parts(core, lib_idx)),
        Some(GuiDestinationKind::Feeds) => Some(feeds_parts(core)),
    }
}

/// The Music destination's parts (design D5): the projection's list facts, and
/// the detail facts with the Hero's artwork, the picked item's menu, and the
/// playing track's id.
fn music_parts(core: &mut GuiCore, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = core.gui_music_projection(lib_idx);
    let picked = projection
        .hero
        .as_ref()
        .map(|hero| hero.album.album_id.clone());
    let art = picked
        .as_deref()
        .map(|album_id| GuiArtRef::from(&core.gui_library_artwork(album_id)));
    let menu = picked
        .map(|album_id| core.gui_library_menu(&album_id))
        .unwrap_or_default();
    let playing_track_id = crate::gui_core::playing_track_id_for_shell(core.app());
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
fn home_parts(core: &mut GuiCore) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = core.gui_home_projection();
    (
        GuiDestinationList::Home(GuiHomeList::from_projection(&projection)),
        GuiLibraryDetail::Home(GuiHomeDetail::from_projection(&projection)),
    )
}

/// The Shows destination's parts (design D14).
fn shows_parts(core: &mut GuiCore, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = core.gui_shows_projection(lib_idx);
    (
        GuiDestinationList::Shows(GuiShowsList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Shows(GuiShowsDetail::from_projection(&projection)),
    )
}

/// The Emby-library destination's parts (design D12, row 8.1).
fn emby_parts(core: &mut GuiCore, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = core.gui_emby_projection(lib_idx);
    (
        GuiDestinationList::Emby(GuiEmbyList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Emby(GuiEmbyDetail::from_projection(&projection)),
    )
}

/// The Audiobooks destination's parts (design D12, row 9.1).
fn audiobooks_parts(core: &mut GuiCore, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = core.gui_audiobooks_projection(lib_idx);
    (
        GuiDestinationList::Audiobooks(GuiAudiobooksList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Audiobooks(GuiAudiobooksDetail::from_projection(&projection)),
    )
}

/// The Podcasts destination's parts (design D12, row 10.1).
fn podcasts_parts(core: &mut GuiCore, lib_idx: usize) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = core.gui_podcasts_projection(lib_idx);
    (
        GuiDestinationList::Podcasts(GuiPodcastsList::from_projection(lib_idx, &projection)),
        GuiLibraryDetail::Podcasts(GuiPodcastsDetail::from_projection(&projection)),
    )
}

/// The Feeds destination's parts (design D12, row 11.1). The entries arrive
/// from the shared startup fetch.
fn feeds_parts(core: &mut GuiCore) -> (GuiDestinationList, GuiLibraryDetail) {
    let projection = core.gui_feeds_projection();
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
    let (kind, pills, rows) = list.map_or(("none", 0, 0), |list| {
        let destination = &list.destination;
        (
            destination.kind_word(),
            destination
                .collections()
                .map_or(0, |collections| collections.entries.len()),
            destination.rows(),
        )
    });
    let hero = detail.is_some_and(GuiLibraryDetail::has_hero);
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
