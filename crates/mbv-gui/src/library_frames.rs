//! The frame's Library section (add-gui-library-panel design D2): the content
//! revision that saves the bridge the content when nothing changed, and the
//! Hero's artwork as the `data:` URLs the window paints.
//!
//! The memo holds the content the last frame built and the revision it
//! carried, so a frame re-carries the whole section only when the content
//! differs from the one before it. A window reload forgets the front end's
//! copy; the reload's `report_layout` re-arms the section, and the next frame
//! re-carries the next build under the revision the window already holds.

use mbv::{App, GuiDestinationKind, GuiLibraryTabs};

use crate::bridge::frame::library::{
    GuiEmbyFrame, GuiHomeFrame, GuiLibraryArt, GuiLibraryContent, GuiLibraryFrame,
    GuiLibraryTabFrame, GuiMusicFrame, GuiShowsFrame,
};

/// The frames' Library section memo (design D2).
#[derive(Debug, Default)]
pub(crate) struct LibraryFrames {
    /// The content revision: it changes whenever the built content differs
    /// from the previously built content, the art included.
    revision: u64,
    /// The content the last build produced, compared against each new build.
    last: Option<GuiLibraryContent>,
    /// The revision the last frame carried; `None` arms the next frame.
    sent: Option<u64>,
}

impl LibraryFrames {
    /// The frame's Library section: the built content with its revision, or no
    /// section when no presented destination is active or the window already
    /// holds this revision.
    pub(crate) fn frame(&mut self, app: &mut App) -> Option<GuiLibraryFrame> {
        // No presented destination is active (design D2): the section is
        // absent, and the next one re-carries its content whatever the window
        // kept while the section was away.
        let destination = app.gui_active_destination();
        // The Audiobooks destination's frame-path ensure (add-gui-library-panel
        // design D12, row 9.1): the default pick's detail load starts before
        // any select does, as the Shows frame path ensures its default series'
        // detail. The section itself joins in the later core surface's unit.
        if let Some(GuiDestinationKind::Audiobooks { lib_idx }) = destination {
            app.ensure_gui_abs_selection(lib_idx);
        }
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
        // Home presents while no library destination presents content
        // (design D12): the destinations are exclusive, so no two fields of
        // the content set themselves at once.
        let home = matches!(destination, Some(GuiDestinationKind::Home)).then(|| home_frame(app));
        if music.is_none() && home.is_none() && shows.is_none() && emby.is_none() {
            self.sent = None;
            return None;
        }
        let content = content(app, music, home, shows, emby);
        if self.last.as_ref() != Some(&content) {
            // The counter wraps like the queue's revision mint: a client
            // lives far shorter than a revision's range, so a repeat is a
            // theoretical hazard, not a real one.
            self.revision = self.revision.wrapping_add(1);
            self.last = Some(content.clone());
            log_content_built(self.revision, &content);
        }
        let carried = self.sent != Some(self.revision);
        if carried {
            self.sent = Some(self.revision);
        }
        Some(GuiLibraryFrame {
            revision: self.revision,
            content: carried.then(|| Box::new(content)),
        })
    }

    /// Arm the next frame: a window reload's `report_layout` re-carries the
    /// content, as it re-arms the theme and the artwork.
    pub(crate) fn rearm(&mut self) {
        self.sent = None;
    }
}

/// The Library content's one built-differently fact per change (design D2):
/// a frame re-carries the content only when its revision changes, so the log
/// rides the same changes the window repaints — never a per-frame spam. The
/// line names the selection's shape and the list's size, so a live session's
/// empty list and its unselected pills are visible in the GUI log.
fn log_content_built(revision: u64, content: &GuiLibraryContent) {
    let music = content.music.as_ref();
    let pills = music.map_or(0, |music| music.pills.len());
    let selected = music.map_or(0, |music| music.pills.iter().filter(|p| p.selected).count());
    let artists = music.map_or(0, |music| music.artists.len());
    let home_rows = content.home.as_ref().map_or(0, |home| home.rows.len());
    let shows_rows = content.shows.as_ref().map_or(0, |shows| shows.rows.len());
    let emby_rows = content.emby.as_ref().map_or(0, |emby| emby.rows.len());
    tracing::info!(
        name: "gui_library.content.built",
        target: "gui_library",
        revision,
        pills,
        selected,
        artists,
        hero = music.is_some_and(|music| music.hero.is_some())
            || content.home.as_ref().is_some_and(|home| home.hero.is_some())
            || content.shows.as_ref().is_some_and(|shows| shows.hero.is_some()),
        home_rows,
        shows_rows,
        emby_rows,
        "the Library content changed"
    );
}

/// The Library panel's content for the active destination (design D2): the
/// destination tabs the top bar paints, the Music destination's pills, list,
/// and Hero while a Grouped Music library is active, Home's flat list and
/// Hero while Home is active (design D12), and the Shows destination's mode
/// pills, list, and Hero while a Shows library is active (design D12, row
/// 7.1). Each destination's field is computed by the caller, so the gate and
/// the content cannot disagree.
fn content(
    app: &mut App,
    music: Option<GuiMusicFrame>,
    home: Option<GuiHomeFrame>,
    shows: Option<GuiShowsFrame>,
    emby: Option<GuiEmbyFrame>,
) -> GuiLibraryContent {
    let GuiLibraryTabs { tabs, current } = app.gui_library_tabs();
    GuiLibraryContent {
        tabs: tabs.into_iter().map(GuiLibraryTabFrame::from).collect(),
        current_tab: current,
        music,
        home,
        shows,
        emby,
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
        .map(|album_id| GuiLibraryArt::from(&app.gui_library_artwork(album_id)));
    let menu = picked
        .map(|album_id| app.gui_library_menu(&album_id))
        .unwrap_or_default();
    let row_menu = app.gui_library_row_menu();
    let playing_track_id = app.gui_playing_track_id();
    GuiMusicFrame::from_projection(
        lib_idx,
        &projection,
        art,
        &menu,
        row_menu.as_ref(),
        playing_track_id,
    )
}

/// The Home destination's content (design D12, the USER-APPROVED GUI Home
/// design): the flat list's projection rows, the core's selected row, and
/// its Hero, fetched and built here per frame so an unchanged frame's
/// revision carries nothing (the memo's content compare).
fn home_frame(app: &mut App) -> GuiHomeFrame {
    let projection = app.gui_home_projection();
    GuiHomeFrame::from_projection(&projection)
}

/// The Shows destination's content (design D12, the USER-APPROVED GUI Shows
/// design): the mode pills, the active mode's rows, the selected row, and
/// its Hero, fetched and built here per frame so an unchanged frame's
/// revision carries nothing (the memo's content compare).
fn shows_frame(app: &mut App, lib_idx: usize) -> GuiShowsFrame {
    app.ensure_gui_shows_detail(lib_idx);
    let projection = app.gui_shows_projection(lib_idx);
    GuiShowsFrame::from_projection(lib_idx, &projection)
}

/// The plain Emby-library destination's content (design D12, row 8.1): the
/// feed pills, the active list's rows, the selected row, and its Hero,
/// fetched and built here per frame so an unchanged frame's revision carries
/// nothing (the memo's content compare). The frame path's ensure is the
/// shown-pass trigger the TUI projection pass's lazy-fetch arm mirrors.
fn emby_frame(app: &mut App, lib_idx: usize) -> GuiEmbyFrame {
    app.ensure_gui_emby_selection(lib_idx);
    let projection = app.gui_emby_projection(lib_idx);
    GuiEmbyFrame::from_projection(lib_idx, &projection)
}
