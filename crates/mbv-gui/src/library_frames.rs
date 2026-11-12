//! The frame's Library section (add-gui-library-panel design D2): the content
//! revision that saves the bridge the content when nothing changed. The
//! content build lives in `sections::library`, the one builder the section
//! ledger shares with this frame path.
//!
//! The memo holds the content the last frame built and the revision it
//! carried, so a frame re-carries the whole section only when the content
//! differs from the one before it. A window reload forgets the front end's
//! copy; the reload's `report_layout` re-arms the section, and the next frame
//! re-carries the next build under the revision the window already holds.

use mbv::App;

use crate::bridge::frame::library::{GuiLibraryContent, GuiLibraryFrame, GuiSearchFrame};
use crate::sections::library::build_content;

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
        let Some(content) = build_content(app) else {
            // No presented destination is active (design D2): the section is
            // absent, and the next one re-carries its content whatever the
            // window kept while the section was away.
            self.sent = None;
            return None;
        };
        // The Search section rides the frame, not the content (row 12.1): a
        // landed result reaches the window whichever content revision the
        // frame carries, and a closed field rides as `None`.
        let search = GuiSearchFrame::carried(&app.gui_search_projection());
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
            search,
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
    let abs_rows = content
        .audiobooks
        .as_ref()
        .map_or(0, |audiobooks| audiobooks.rows.len());
    let podcasts_rows = content
        .podcasts
        .as_ref()
        .map_or(0, |podcasts| podcasts.rows.len());
    let feeds_rows = content.feeds.as_ref().map_or(0, |feeds| feeds.rows.len());
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
        audiobooks_rows = abs_rows,
        podcasts_rows,
        feeds_rows,
        "the Library content changed"
    );
}
