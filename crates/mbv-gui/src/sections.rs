//! The section ledger (design D2 of `gui-change-driven-sections`): the Client
//! core's state for what the window holds. For each section it keeps the last
//! built value, the revision that value carries, and whether the window holds
//! that revision.
//!
//! [`SectionLedger::collect`] builds the compared sections on every tick and
//! compares them with their last built value. It builds the gated sections only
//! when a mark names them (design D3), and it sends a section only when the
//! window does not hold its revision. The result is one [`GuiSectionBatch`],
//! or `None` when the window already holds everything.
//!
//! The Client core's step calls `collect` once per tick that did not ask for a
//! restart (design D2), and sends the batch it returns. The ledger is the only
//! builder of the sections.

mod deadlines;
pub(crate) mod library;
mod marks;
mod playhead;

#[cfg(any(test, feature = "test"))]
pub use marks::client_event_marks;
pub(crate) use marks::{DrainHook, marks_for_hook};

use std::collections::HashSet;
use std::time::{Duration, Instant};

use mbv::{GuiCore, QueuePlaybackProjection};
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;

use crate::bridge::frame::GuiAttachState;
use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::frame::chrome::{GuiQueueFooter, GuiStatusBar, GuiToasts};
use crate::bridge::frame::library::{
    GuiLibraryDetail, GuiLibraryList, GuiLibraryRowMenuFrame, GuiSearchFrame,
};
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{
    GuiPlayerControls, GuiPlayhead, GuiQueuePlayback, GuiScrubberHover, card_artwork_ref,
};
use crate::bridge::frame::playlists::GuiPlaylistsFrame;
use crate::bridge::frame::queue::{GuiQueue, GuiQueueSummary};
use crate::bridge::frame::scrubber_hover;
use crate::bridge::frame::settings::GuiSettingsFrame;
use crate::bridge::frame::theme::GuiTheme;
use crate::bridge::sections::{GuiSection, GuiSectionBatch};

use self::deadlines::DeadlineMarks;

/// One section of the window (design D1). The compared sections build on every
/// tick, so a mark on one of them changes nothing; the gated sections need a
/// mark to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Theme,
    Queue,
    Summary,
    Panel,
    Overlay,
    Playback,
    Hover,
    Footer,
    Status,
    Toasts,
    Library,
    Search,
    Settings,
    Playlists,
}

/// The gated sections (design D2). A mark on each one rebuilds them all; the
/// ledger compares each build, so an unchanged section sends nothing.
const GATED_SECTIONS: &[Section] = &[
    Section::Theme,
    Section::Library,
    Section::Search,
    Section::Settings,
    Section::Playlists,
];

/// The revisions the pre-attach campaign sent (design D4). The attached core
/// continues from them, so its first attach and theme sections mint past the
/// revisions the window already holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SentRevisions {
    /// The last attach revision the campaign sent.
    pub attach: u64,
    /// The last theme revision the campaign sent.
    pub theme: u64,
}

/// What one [`SectionLedger::collect`] reads: the GUI core whose read-only
/// projections the sections carry (and whose gated builds start their loads),
/// the Queue Surface state, the Queue playback Surface state
/// (for the scrubber hover), and the Queue viewport's whole rows.
#[derive(Debug)]
pub struct SectionSources<'a> {
    /// The GUI Client's core the sections read.
    pub core: &'a mut GuiCore,
    /// The Queue Surface state. The caller has already adopted the Queue
    /// projection into it for this tick.
    pub queue: &'a QueueSurface,
    /// The Queue playback Surface state: the scrubber hover the last pointer
    /// move resolved.
    pub playback_surface: &'a QueuePlaybackSurface,
    /// The whole Queue rows the viewport fits.
    pub viewport_rows: usize,
    /// The tick's clock: the deadline reads compare against it (design D3 rule
    /// 3).
    pub now: Instant,
}

/// The ledger's state for one section (design D2): the last built value, its
/// revision, and whether the window holds that revision.
#[derive(Debug)]
struct Slot<T> {
    /// The last built value; `None` until the section's first build.
    value: Option<T>,
    /// The revision `value` carries. It starts at 0 and mints 1 on the first
    /// build.
    revision: u64,
    /// Whether the window holds `revision`. `release` clears it.
    held: bool,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            value: None,
            revision: 0,
            held: false,
        }
    }
}

impl<T: PartialEq> Slot<T> {
    /// Adopt a fresh build. A value that differs from the last built value mints
    /// the next revision and arms the send; an equal value changes nothing.
    fn adopt(&mut self, built: T) {
        if self.value.as_ref() == Some(&built) {
            return;
        }
        // The counter wraps like the library memo's revision mint: a client
        // lives far shorter than a revision's range.
        self.revision = self.revision.wrapping_add(1);
        self.value = Some(built);
        self.held = false;
    }
}

impl<T: Clone> Slot<T> {
    /// The section to send, or `None` when the window already holds its
    /// revision or the section was never built. A sent section is held.
    fn take(&mut self) -> Option<GuiSection<T>> {
        if self.held {
            return None;
        }
        let value = self.value.clone()?;
        self.held = true;
        Some(GuiSection {
            revision: self.revision,
            value,
        })
    }
}

impl<T> Slot<T> {
    /// Forget what the window holds, so the next take sends the value again at
    /// its current revision (the `sync` path).
    fn release(&mut self) {
        self.held = false;
    }

    /// Drop the built value, so the next adopt starts over: it treats the
    /// section as never built and mints a new revision (the playhead's `sync`
    /// path, design D5).
    fn forget(&mut self) {
        self.value = None;
        self.held = false;
    }
}

/// The audit's check of one clean gated section (design D3): its fresh build
/// must equal the last built value. A difference is a source that changed with
/// no mark, so the section would stay stale in production.
fn audit_clean<T: PartialEq>(section: Section, slot: &Slot<T>, built: &T) {
    assert!(
        slot.value.as_ref() == Some(built),
        "section {section:?} changed without a mark: a drain hook or an input misses its mark"
    );
}

/// The section ledger (design D2). Each field is one section's slot. The marks
/// name the gated sections whose sources changed since the last collect.
#[derive(Debug)]
pub struct SectionLedger {
    attach: Slot<Option<GuiAttachState>>,
    theme: Slot<GuiTheme>,
    queue: Slot<GuiQueue>,
    summary: Slot<GuiQueueSummary>,
    panel: Slot<GuiPanelToggles>,
    overlay: Slot<Option<GuiOverlayFrame>>,
    playback: Slot<GuiQueuePlayback>,
    hover: Slot<Option<GuiScrubberHover>>,
    footer: Slot<GuiQueueFooter>,
    status: Slot<GuiStatusBar>,
    toasts: Slot<GuiToasts>,
    /// The `libraryList` section (design D8): the tabs and the list part. One
    /// gated Library pass builds it with `library_detail` and `row_menu`.
    library_list: Slot<Option<GuiLibraryList>>,
    /// The `libraryDetail` section (design D8): the detail part.
    library_detail: Slot<Option<GuiLibraryDetail>>,
    /// The `rowMenu` section: the right-click item menu.
    row_menu: Slot<Option<GuiLibraryRowMenuFrame>>,
    search: Slot<Option<GuiSearchFrame>>,
    settings: Slot<Option<GuiSettingsFrame>>,
    playlists: Slot<Option<GuiPlaylistsFrame>>,
    /// The playhead anchor (design D5): the last sent anchor, the revision it
    /// carries, and whether the window holds it. A trigger adopts a new
    /// anchor; the window predicts the position between anchors.
    playhead: Slot<GuiPlayhead>,
    marked: HashSet<Section>,
    /// The deadline and playhead values the last tick sampled (design D3 rules
    /// 3 and 4).
    deadlines: DeadlineMarks,
    /// The anchor compare's own state (design D5): the last observed position
    /// and the `Instant` the last anchor went out.
    playhead_anchor: playhead::PlayheadAnchor,
    /// Whether the audit runs (design D3). Production never turns it on; the
    /// core-thread tests do, through `CoreThread::set_section_audit_for_test`.
    audit: bool,
}

impl Default for SectionLedger {
    /// A fresh ledger holds no section. Every gated section carries a mark
    /// from the start, so its first collect builds it (design D4): a gated
    /// section no drain or input has marked yet would otherwise never reach the
    /// window, because no mark would name it. The theme and the Library, Search,
    /// and Settings builds all start this way.
    fn default() -> Self {
        let mut ledger = Self {
            attach: Slot::default(),
            theme: Slot::default(),
            queue: Slot::default(),
            summary: Slot::default(),
            panel: Slot::default(),
            overlay: Slot::default(),
            playback: Slot::default(),
            hover: Slot::default(),
            footer: Slot::default(),
            status: Slot::default(),
            toasts: Slot::default(),
            library_list: Slot::default(),
            library_detail: Slot::default(),
            row_menu: Slot::default(),
            search: Slot::default(),
            settings: Slot::default(),
            playlists: Slot::default(),
            playhead: Slot::default(),
            marked: HashSet::new(),
            deadlines: DeadlineMarks::new(Instant::now()),
            playhead_anchor: playhead::PlayheadAnchor::new(Instant::now()),
            audit: false,
        };
        ledger.mark_all_gated();
        ledger
    }
}

impl SectionLedger {
    /// Mark one section for the next collect. A gated section builds then; a
    /// compared section builds on every collect anyway.
    pub fn mark(&mut self, section: Section) {
        self.marked.insert(section);
    }

    /// Mark every gated section (design D3): the startup drains and the
    /// reprojections change their sources, which no single section names.
    pub fn mark_all_gated(&mut self) {
        for section in GATED_SECTIONS {
            self.mark(*section);
        }
    }

    /// Turn the audit on or off (design D3). With it on, `collect` builds every
    /// clean gated section and panics when the build differs from the last
    /// built value, which names a missed mark.
    pub fn set_audit_for_test(&mut self, on: bool) {
        self.audit = on;
    }

    /// Move the playhead anchor's send instant back by `by` (design D5's
    /// drift rule), for a core-thread test that pins the drift with no real
    /// wait. Production never calls it.
    pub(crate) fn backdate_playhead_anchor_for_test(&mut self, by: Duration) {
        self.playhead_anchor.backdate_for_test(by);
    }

    /// Continue the revisions the attach campaign sent (design D4), so the
    /// attached core's first attach and theme sections mint past them.
    pub fn resume(&mut self, sent: SentRevisions) {
        self.attach.revision = sent.attach;
        self.theme.revision = sent.theme;
    }

    /// Forget every section the window holds (design D4, the `sync` path). The
    /// next collect sends every present section at its current revision.
    ///
    /// The playhead is the one exception (design D5): its value is a
    /// position-at-send sample the window stamps on receipt, so replaying
    /// it would rewind the position by the age of the anchor. Sync forgets it
    /// instead, and the next collect re-adopts a fresh anchor — current
    /// position, fresh send instant — at a new revision.
    pub fn release_all(&mut self) {
        self.attach.release();
        self.theme.release();
        self.queue.release();
        self.summary.release();
        self.panel.release();
        self.overlay.release();
        self.playback.release();
        self.hover.release();
        self.footer.release();
        self.status.release();
        self.toasts.release();
        self.library_list.release();
        self.library_detail.release();
        self.row_menu.release();
        self.search.release();
        self.settings.release();
        self.playlists.release();
        self.playhead.forget();
        self.playhead_anchor.reset();
    }

    /// Build the sections this tick reads and return the batch of the sections
    /// the window does not hold, or `None` when it holds all of them. Consumes
    /// the marks.
    pub fn collect(&mut self, sources: SectionSources<'_>) -> Option<GuiSectionBatch> {
        let SectionSources {
            core,
            queue,
            playback_surface,
            viewport_rows,
            now,
        } = sources;
        let mut marked = std::mem::take(&mut self.marked);
        // One playback projection read feeds the playback section, the overlay,
        // the hover's tick, the settings build, and the target-choice compare,
        // so they cannot disagree.
        let playback = core.app().queue_playback_projection();
        self.deadlines.sample(core, &playback, now, &mut marked);
        // The frame's reset counter is read before the core's longer read
        // borrow, so both borrows stay sequential.
        let reset_epoch = core.gui_ui_reset_epoch();
        let app = core.app();
        let summary = app.queue_summary_projection(queue.multi_selection());
        // The queue's end offsets (design D5): the duration facts the summary
        // and queue sections carry in place of the clock strings the window
        // now derives from the playhead anchor.
        let offsets = app.gui_queue_end_offsets();
        // The header's chooser facts read the same shown queue and entry list
        // the projections above read, so the frame cannot disagree inside one
        // tick.
        let header = core.gui_queue_header_projection();
        // The attached core clears the pre-attach state (design D4): `null`
        // rides the first batch, and no later build changes it.
        self.attach.adopt(None);
        self.queue
            .adopt(GuiQueue::from_surface(queue, viewport_rows, &summary));
        self.summary
            .adopt(GuiQueueSummary::build(summary, &offsets, &header));
        self.panel.adopt(GuiPanelToggles::new(
            &core.panel_toggles_projection(),
            reset_epoch,
        ));
        self.overlay.adopt(
            core.gui_overlay_projection(&playback)
                .as_ref()
                .map(GuiOverlayFrame::from),
        );
        self.hover
            .adopt(scrubber_hover(playback_surface.scrubber_hover(), &playback));
        let artwork = card_artwork_ref(&core.app_mut().queue_card_artwork());
        let app = core.app();
        // The GUI-only facts (gui-art-player-overlay design D5, D7): the
        // auto-hide preference the core owns, and whether the shown local
        // queue can start while nothing plays. Both are core reads.
        let controls = GuiPlayerControls {
            auto_hide: core.gui_player_auto_hide(),
            can_start: app.shown_queue_can_start(),
        };
        self.playback.adopt(GuiQueuePlayback {
            artwork,
            controls,
            ..GuiQueuePlayback::from(&playback)
        });
        self.footer
            .adopt(GuiQueueFooter::from(&app.queue_footer_projection()));
        self.status
            .adopt(GuiStatusBar::from(&app.status_bar_projection()));
        // The toast cards the window's stack paints (design D6). The section
        // is compared, so a toast that expires leaves on the next build with
        // no deadline of its own.
        self.toasts.adopt(GuiToasts::from(app.toast_cards()));
        // The anchor reads the same projection read and the same now-playing
        // marker the queue section built from, so it cannot disagree with
        // them (design D5).
        self.playhead_anchor
            .sample(&mut self.playhead, &playback, queue, now);
        self.adopt_marked_gated(core, &marked, &playback);
        if self.audit {
            self.audit_clean_gated(core, &marked, &playback);
        }
        self.take_batch()
    }

    /// The audit of design D3: build each clean gated section (one without a
    /// mark this collect) and check that the build equals its last built value.
    /// A mismatch means a source changed with no mark, so the section would stay
    /// stale in production.
    fn audit_clean_gated(
        &self,
        core: &mut GuiCore,
        marked: &HashSet<Section>,
        playback: &QueuePlaybackProjection,
    ) {
        if !marked.contains(&Section::Theme) {
            audit_clean(Section::Theme, &self.theme, &GuiTheme::current());
        }
        if !marked.contains(&Section::Library) {
            let pass = library::build(core);
            audit_clean(Section::Library, &self.library_list, &pass.list);
            audit_clean(Section::Library, &self.library_detail, &pass.detail);
            audit_clean(Section::Library, &self.row_menu, &pass.row_menu);
        }
        if !marked.contains(&Section::Search) {
            let search = core
                .gui_search_projection()
                .map(GuiSearchFrame::from_projection);
            audit_clean(Section::Search, &self.search, &search);
        }
        if !marked.contains(&Section::Settings) {
            let settings = GuiSettingsFrame::build(core, playback);
            audit_clean(Section::Settings, &self.settings, &settings);
        }
        if !marked.contains(&Section::Playlists) {
            let playlists = GuiPlaylistsFrame::build(core);
            audit_clean(Section::Playlists, &self.playlists, &playlists);
        }
    }

    /// Build each gated section that carries a mark (design D2). A mark that
    /// changes nothing sends nothing, because the build compares equal.
    fn adopt_marked_gated(
        &mut self,
        core: &mut GuiCore,
        marked: &HashSet<Section>,
        playback: &QueuePlaybackProjection,
    ) {
        if marked.contains(&Section::Theme) {
            self.theme.adopt(GuiTheme::current());
        }
        if marked.contains(&Section::Library) {
            let before = (self.library_list.revision, self.library_detail.revision);
            let pass = library::build(core);
            self.library_list.adopt(pass.list);
            self.library_detail.adopt(pass.detail);
            self.row_menu.adopt(pass.row_menu);
            let after = (self.library_list.revision, self.library_detail.revision);
            if after != before {
                library::log_built(
                    after.0,
                    after.1,
                    self.library_list.value.as_ref().and_then(Option::as_ref),
                    self.library_detail.value.as_ref().and_then(Option::as_ref),
                );
            }
        }
        if marked.contains(&Section::Search) {
            self.search.adopt(
                core.gui_search_projection()
                    .map(GuiSearchFrame::from_projection),
            );
        }
        if marked.contains(&Section::Settings) {
            self.settings.adopt(GuiSettingsFrame::build(core, playback));
        }
        if marked.contains(&Section::Playlists) {
            self.playlists.adopt(GuiPlaylistsFrame::build(core));
        }
    }

    /// The batch of every slot the window does not hold, or `None` when the
    /// batch is empty. Each sent slot is held after this.
    fn take_batch(&mut self) -> Option<GuiSectionBatch> {
        let batch = GuiSectionBatch {
            attach: self.attach.take(),
            theme: self.theme.take(),
            queue: self.queue.take(),
            summary: self.summary.take(),
            panel: self.panel.take(),
            overlay: self.overlay.take(),
            playback: self.playback.take(),
            hover: self.hover.take(),
            footer: self.footer.take(),
            status: self.status.take(),
            toasts: self.toasts.take(),
            library_list: self.library_list.take(),
            library_detail: self.library_detail.take(),
            row_menu: self.row_menu.take(),
            search: self.search.take(),
            settings: self.settings.take(),
            playlists: self.playlists.take(),
            playhead: self.playhead.take(),
        };
        (!batch.is_empty()).then_some(batch)
    }
}
