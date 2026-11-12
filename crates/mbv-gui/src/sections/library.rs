//! The Library content build (add-gui-library-panel design D2), the one builder
//! of the gated Library section (`gui-change-driven-sections` design D2). The
//! destination's ensures run inside the build, so a build starts the
//! loads its section waits on (design D12 of add-gui-library-panel).

use mbv::{App, GuiDestinationKind, GuiLibraryTabs};

use crate::art::GuiArtRef;
use crate::bridge::frame::library::{
    GuiAudiobooksFrame, GuiEmbyFrame, GuiFeedsFrame, GuiHomeFrame, GuiLibraryContent,
    GuiLibraryRowMenuFrame, GuiLibraryTabFrame, GuiMusicFrame, GuiPodcastsFrame, GuiShowsFrame,
};

/// The Library content of the active destination (design D2): the destination
/// tabs the top bar paints, and the one active destination's section. `None`
/// while no presented destination is active. The ensures run before their
/// section is built, as the step always ran them.
pub(crate) fn build_content(app: &mut App) -> Option<GuiLibraryContent> {
    // No presented destination is active (design D2): the section is absent.
    let destination = app.gui_active_destination();
    // The Podcasts destination's ensure (add-gui-library-panel
    // design D12, row 10.1): the selected pill's own loads — the Latest
    // shelf and every show's episodes, or one show's episodes — start
    // before any command does, as the Audiobooks ensure starts its
    // default pick's detail. The section itself is built with the content
    // below.
    if let Some(GuiDestinationKind::Podcasts { lib_idx }) = destination {
        app.ensure_gui_podcast_selection(lib_idx);
    }
    // The Feeds destination's ensure (add-gui-library-panel
    // design D12, row 11.1): the stored pill converges on the subscription
    // set the tab holds. The entries arrive from the shared startup fetch,
    // so the ensure starts no load.
    if matches!(destination, Some(GuiDestinationKind::Feeds)) {
        app.ensure_gui_feeds_selection();
    }
    // The Audiobooks destination's ensure (add-gui-library-panel
    // design D12, row 9.1) sits in its match arm below: the default pick's
    // detail load starts before any select does.
    let music = match destination {
        Some(GuiDestinationKind::Music { lib_idx }) => Some(music_frame(app, lib_idx)),
        _ => None,
    };
    let shows = match destination {
        Some(GuiDestinationKind::Shows { lib_idx }) => Some(shows_frame(app, lib_idx)),
        _ => None,
    };
    let emby = match destination {
        Some(GuiDestinationKind::Library { lib_idx }) => Some(emby_frame(app, lib_idx)),
        _ => None,
    };
    let audiobooks = match destination {
        Some(GuiDestinationKind::Audiobooks { lib_idx }) => {
            app.ensure_gui_abs_selection(lib_idx);
            Some(audiobooks_frame(app, lib_idx))
        }
        _ => None,
    };
    let podcasts = match destination {
        Some(GuiDestinationKind::Podcasts { lib_idx }) => Some(podcasts_frame(app, lib_idx)),
        _ => None,
    };
    let feeds = matches!(destination, Some(GuiDestinationKind::Feeds)).then(|| feeds_frame(app));
    // Home presents while no library destination presents content (design
    // D12): the destinations are exclusive, so no two fields of the content
    // set themselves at once.
    let home = matches!(destination, Some(GuiDestinationKind::Home)).then(|| home_frame(app));
    let sections = DestinationSections {
        music,
        home,
        shows,
        emby,
        audiobooks,
        podcasts,
        feeds,
    };
    sections.any().then(|| content(app, sections))
}

/// The active destination's section, computed by the gate above (design D2):
/// exactly one field is present, because the destinations are exclusive. The
/// struct keeps the content build's argument count in hand as the presented
/// destinations grow.
#[derive(Debug, Default)]
struct DestinationSections {
    music: Option<GuiMusicFrame>,
    home: Option<GuiHomeFrame>,
    shows: Option<GuiShowsFrame>,
    emby: Option<GuiEmbyFrame>,
    audiobooks: Option<GuiAudiobooksFrame>,
    podcasts: Option<GuiPodcastsFrame>,
    feeds: Option<GuiFeedsFrame>,
}

impl DestinationSections {
    /// Whether any destination presents content: the gate's condition, and the
    /// one the section's absence names.
    fn any(&self) -> bool {
        self.music.is_some()
            || self.home.is_some()
            || self.shows.is_some()
            || self.emby.is_some()
            || self.audiobooks.is_some()
            || self.podcasts.is_some()
            || self.feeds.is_some()
    }
}

/// The Library panel's content for the active destination (design D2): the
/// destination tabs the top bar paints, and the one active destination's
/// section — the Music destination's pills, list, and Hero while a Grouped
/// Music library is active, Home's flat list and Hero while Home is active
/// (design D12), the Shows destination's mode pills, list, and Hero while a
/// Shows library is active (design D12, row 7.1), and the Feeds destination's
/// pill row, entry rows, and Hero while Feeds is active (row 11.1). The gate
/// above computes the section, so the gate and the content cannot disagree.
fn content(app: &mut App, sections: DestinationSections) -> GuiLibraryContent {
    let GuiLibraryTabs { tabs, current } = app.gui_library_tabs();
    // The right-click item menu is the Library's, not one destination's (design
    // D8, D15): a row of any destination opens it, so it rides the content.
    let row_menu = app.gui_library_row_menu();
    GuiLibraryContent {
        tabs: tabs.into_iter().map(GuiLibraryTabFrame::from).collect(),
        current_tab: current,
        music: sections.music,
        home: sections.home,
        shows: sections.shows,
        emby: sections.emby,
        audiobooks: sections.audiobooks,
        podcasts: sections.podcasts,
        feeds: sections.feeds,
        row_menu: row_menu.as_ref().map(GuiLibraryRowMenuFrame::from),
    }
}

/// The Music destination's content (design D5): the projection's facts with
/// the Hero's artwork projected beside them, the picked item's menu, and the
/// playing track's id.
fn music_frame(app: &mut App, lib_idx: usize) -> GuiMusicFrame {
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
    GuiMusicFrame::from_projection(lib_idx, &projection, art, &menu, playing_track_id)
}

/// The Home destination's content (design D12, the USER-APPROVED GUI Home
/// design): the flat list's projection rows, the core's selected row, and
/// its Hero, fetched and built here on each rebuild so an unchanged build's
/// revision carries nothing (the memo's content compare).
fn home_frame(app: &mut App) -> GuiHomeFrame {
    let projection = app.gui_home_projection();
    GuiHomeFrame::from_projection(&projection)
}

/// The Shows destination's content (design D14): the mode pills, the active
/// mode's rows — the All series tree or the flat lists — the selected row,
/// and its Hero, fetched and built here on each rebuild so an unchanged
/// build's revision carries nothing (the memo's content compare). The ensure loads
/// the selected series' detail **and its selected season's episodes**, so the
/// Hero's Workspace paints on arrival.
fn shows_frame(app: &mut App, lib_idx: usize) -> GuiShowsFrame {
    app.ensure_gui_shows_workspace(lib_idx);
    let projection = app.gui_shows_projection(lib_idx);
    GuiShowsFrame::from_projection(lib_idx, &projection)
}

/// The plain Emby-library destination's content (design D12, row 8.1): the
/// feed pills, the active list's rows, the selected row, and its Hero,
/// fetched and built here on each rebuild so an unchanged build's revision
/// carries nothing (the memo's content compare). The build's ensure is the
/// shown-pass trigger the TUI projection pass's lazy-fetch arm mirrors.
fn emby_frame(app: &mut App, lib_idx: usize) -> GuiEmbyFrame {
    app.ensure_gui_emby_selection(lib_idx);
    let projection = app.gui_emby_projection(lib_idx);
    GuiEmbyFrame::from_projection(lib_idx, &projection)
}

/// The Audiobooks destination's content (design D12, row 9.1): the book
/// list, the selected book, and its Hero with its chapters Workspace, built
/// here on each rebuild so an unchanged build's revision carries nothing (the
/// memo's content compare). The ensure above the content build is the
/// default pick's detail-load trigger.
fn audiobooks_frame(app: &mut App, lib_idx: usize) -> GuiAudiobooksFrame {
    let projection = app.gui_audiobooks_projection(lib_idx);
    GuiAudiobooksFrame::from_projection(lib_idx, &projection)
}

/// The Podcasts destination's content (design D12, row 10.1): the pill row,
/// the active pill's episode rows, the selected episode, and its Hero, built
/// here on each rebuild so an unchanged build's revision carries nothing (the
/// memo's content compare). The ensure above the content build is the selected pill's
/// load trigger.
fn podcasts_frame(app: &mut App, lib_idx: usize) -> GuiPodcastsFrame {
    let projection = app.gui_podcasts_projection(lib_idx);
    GuiPodcastsFrame::from_projection(lib_idx, &projection)
}

/// The Feeds destination's content (design D12, row 11.1): the pill row, the
/// active pill's entry rows, the selected entry, and its Hero, built here per
/// rebuild so an unchanged build's revision carries nothing (the memo's content
/// compare). The entries arrive from the shared startup fetch, so the build
/// starts no load; the ensure has already settled the pill.
fn feeds_frame(app: &mut App) -> GuiFeedsFrame {
    let projection = app.gui_feeds_projection();
    GuiFeedsFrame::from_projection(&projection)
}

/// The Library content's one built-differently fact per change (design D2):
/// the section is sent only when its revision changes, so the log rides the
/// same changes the window repaints. The line names the selection's shape and
/// the list's size, so a live session's empty list and its unselected pills
/// are visible in the GUI log.
pub(crate) fn log_built(revision: u64, content: &GuiLibraryContent) {
    let music = content.music.as_ref();
    let pills = music.map_or(0, |music| music.list.pills.len());
    let selected = music.map_or(0, |music| {
        music.list.pills.iter().filter(|p| p.selected).count()
    });
    let artists = music.map_or(0, |music| music.list.artists.len());
    let home_rows = content.home.as_ref().map_or(0, |home| home.list.rows.len());
    let shows_rows = content
        .shows
        .as_ref()
        .map_or(0, |shows| shows.list.rows.len());
    let emby_rows = content.emby.as_ref().map_or(0, |emby| emby.list.rows.len());
    let abs_rows = content
        .audiobooks
        .as_ref()
        .map_or(0, |audiobooks| audiobooks.list.rows.len());
    let podcasts_rows = content
        .podcasts
        .as_ref()
        .map_or(0, |podcasts| podcasts.list.rows.len());
    let feeds_rows = content
        .feeds
        .as_ref()
        .map_or(0, |feeds| feeds.list.rows.len());
    tracing::info!(
        name: "gui_library.content.built",
        target: "gui_library",
        revision,
        pills,
        selected,
        artists,
        hero = music.is_some_and(|music| music.detail.hero.is_some())
            || content
                .home
                .as_ref()
                .is_some_and(|home| home.detail.hero.is_some())
            || content.shows.as_ref().is_some_and(|shows| shows.detail.hero.is_some()),
        home_rows,
        shows_rows,
        emby_rows,
        audiobooks_rows = abs_rows,
        podcasts_rows,
        feeds_rows,
        "the Library content changed"
    );
}
