//! The frame's Library section (add-gui-library-panel design D2): the content
//! revision that saves the bridge the content when nothing changed, and the
//! Hero's artwork as the `data:` URLs the window paints.
//!
//! The memo holds the content the last frame built and the revision it
//! carried, so a frame re-carries the whole section only when the content
//! differs from the one before it. A window reload forgets the front end's
//! copy; the reload's `report_layout` re-arms the section, and the next frame
//! re-carries the next build under the revision the window already holds.

use mbv::{App, GuiLibraryTabs};

use crate::bridge::frame::library::{
    GuiLibraryArt, GuiLibraryContent, GuiLibraryFrame, GuiLibraryTabFrame, GuiMusicFrame,
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
    /// content when the window already holds this revision.
    pub(crate) fn frame(&mut self, app: &mut App) -> GuiLibraryFrame {
        let content = content(app);
        if self.last.as_ref() != Some(&content) {
            // The counter wraps like the queue's revision mint: a client
            // lives far shorter than a revision's range, so a repeat is a
            // theoretical hazard, not a real one.
            self.revision = self.revision.wrapping_add(1);
            self.last = Some(content.clone());
        }
        let carried = self.sent != Some(self.revision);
        if carried {
            self.sent = Some(self.revision);
        }
        GuiLibraryFrame {
            revision: self.revision,
            content: carried.then(|| Box::new(content)),
        }
    }

    /// Arm the next frame: a window reload's `report_layout` re-carries the
    /// content, as it re-arms the theme and the artwork.
    pub(crate) fn rearm(&mut self) {
        self.sent = None;
    }
}

/// The Library panel's content for the active destination (design D2): the
/// destination tabs the top bar paints, and the Music destination's pills,
/// list, and Hero while a Grouped Music library is active.
fn content(app: &mut App) -> GuiLibraryContent {
    let GuiLibraryTabs { tabs, current } = app.gui_library_tabs();
    let music = app
        .gui_music_destination()
        .map(|lib_idx| music_frame(app, lib_idx));
    GuiLibraryContent {
        tabs: tabs.into_iter().map(GuiLibraryTabFrame::from).collect(),
        current_tab: current,
        music,
    }
}

/// The Music destination's content (design D5): the projection's facts with
/// the Hero's artwork projected beside them and the picked item's menu.
fn music_frame(app: &mut App, lib_idx: usize) -> GuiMusicFrame {
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
    GuiMusicFrame::from_projection(lib_idx, &projection, art, &menu)
}
