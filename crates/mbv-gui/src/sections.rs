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

pub(crate) use marks::{DrainHook, marks_for_hook};

use std::collections::HashSet;
use std::time::{Duration, Instant};

use mbv::{App, QueuePlaybackProjection};
use mbv_surfaces::FocusStack;
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;

use crate::artwork::ArtworkFrames;
use crate::bridge::frame::GuiAttachState;
use crate::bridge::frame::chrome::{GuiQueueFooter, GuiStatusBar};
use crate::bridge::frame::focus::GuiFocus;
use crate::bridge::frame::library::{GuiLibraryContent, GuiSearchFrame};
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{
    GuiPlayhead, GuiQueuePlayback, GuiScrubberHover, GuiScrubberTile,
};
use crate::bridge::frame::queue::{GuiQueue, GuiQueueSummary};
use crate::bridge::frame::settings::GuiSettingsFrame;
use crate::bridge::frame::theme::GuiTheme;
use crate::bridge::frame::{GuiPanelToggles, scrubber_hover, scrubber_ticks};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::trickplay::TrickplayFrames;

use self::deadlines::DeadlineMarks;

/// One section of the window (design D1). The compared sections build on every
/// tick, so a mark on one of them changes nothing; the gated sections need a
/// mark to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Theme,
    Focus,
    Queue,
    Summary,
    Panel,
    Overlay,
    Playback,
    Hover,
    Footer,
    Status,
    Library,
    Search,
    Settings,
}

/// The gated sections (design D2). A mark on each one rebuilds them all; the
/// ledger compares each build, so an unchanged section sends nothing.
const GATED_SECTIONS: &[Section] = &[
    Section::Theme,
    Section::Library,
    Section::Search,
    Section::Settings,
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

/// What one [`SectionLedger::collect`] reads: the `App` whose read-only
/// projections the sections carry (and whose gated builds start their loads),
/// the focus stack, the Queue Surface state, the Queue playback Surface state
/// (for the scrubber hover), and the Queue viewport's whole rows.
#[derive(Debug)]
pub struct SectionSources<'a> {
    /// The `App` the sections read.
    pub app: &'a mut App,
    /// The focus stack: the focused surface and the Panel focus.
    pub focus: &'a FocusStack,
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
    focus: Slot<GuiFocus>,
    queue: Slot<GuiQueue>,
    summary: Slot<GuiQueueSummary>,
    panel: Slot<GuiPanelToggles>,
    overlay: Slot<Option<GuiOverlayFrame>>,
    playback: Slot<GuiQueuePlayback>,
    hover: Slot<Option<GuiScrubberHover>>,
    footer: Slot<GuiQueueFooter>,
    status: Slot<GuiStatusBar>,
    library: Slot<Option<GuiLibraryContent>>,
    search: Slot<Option<GuiSearchFrame>>,
    settings: Slot<Option<GuiSettingsFrame>>,
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
    artwork: ArtworkFrames,
    trickplay: TrickplayFrames,
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
            focus: Slot::default(),
            queue: Slot::default(),
            summary: Slot::default(),
            panel: Slot::default(),
            overlay: Slot::default(),
            playback: Slot::default(),
            hover: Slot::default(),
            footer: Slot::default(),
            status: Slot::default(),
            library: Slot::default(),
            search: Slot::default(),
            settings: Slot::default(),
            playhead: Slot::default(),
            marked: HashSet::new(),
            deadlines: DeadlineMarks::new(Instant::now()),
            playhead_anchor: playhead::PlayheadAnchor::new(Instant::now()),
            audit: false,
            artwork: ArtworkFrames::default(),
            trickplay: TrickplayFrames::default(),
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
    pub fn release_all(&mut self) {
        self.attach.release();
        self.theme.release();
        self.focus.release();
        self.queue.release();
        self.summary.release();
        self.panel.release();
        self.overlay.release();
        self.playback.release();
        self.hover.release();
        self.footer.release();
        self.status.release();
        self.library.release();
        self.search.release();
        self.settings.release();
        self.playhead.release();
    }

    /// Build the sections this tick reads and return the batch of the sections
    /// the window does not hold, or `None` when it holds all of them. Consumes
    /// the marks.
    pub fn collect(&mut self, sources: SectionSources<'_>) -> Option<GuiSectionBatch> {
        let SectionSources {
            app,
            focus,
            queue,
            playback_surface,
            viewport_rows,
            now,
        } = sources;
        let mut marked = std::mem::take(&mut self.marked);
        // One playback projection read feeds the playback section, the overlay,
        // the hover's tick, the settings build, and the target-choice compare,
        // so they cannot disagree.
        let playback = app.queue_playback_projection();
        self.deadlines.sample(app, &playback, now, &mut marked);
        let summary = app.queue_summary_projection();
        // The queue's end offsets (design D5): the duration facts the summary
        // and queue sections carry in place of the clock strings the window
        // now derives from the playhead anchor.
        let offsets = app.gui_queue_end_offsets();
        // The attached core clears the pre-attach state (design D4): `null`
        // rides the first batch, and no later build changes it.
        self.attach.adopt(None);
        self.focus.adopt(GuiFocus::from(focus));
        self.queue.adopt(GuiQueue::from_surface(
            queue,
            viewport_rows,
            &summary,
            &offsets,
        ));
        self.summary
            .adopt(GuiQueueSummary::build(&summary, &offsets));
        self.panel.adopt(GuiPanelToggles::new(
            &app.panel_toggles_projection(),
            app.gui_ui_reset_epoch(),
        ));
        self.overlay.adopt(
            app.gui_overlay_projection(&playback)
                .as_ref()
                .map(GuiOverlayFrame::from),
        );
        let tile = self.hover_tile(app, &playback, playback_surface);
        self.hover.adopt(scrubber_hover(
            playback_surface.scrubber_hover(),
            &playback,
            tile,
        ));
        let artwork = self.artwork.current_value(&app.queue_card_artwork());
        self.playback.adopt(GuiQueuePlayback {
            artwork: Some(artwork),
            ..GuiQueuePlayback::from(&playback)
        });
        self.footer
            .adopt(GuiQueueFooter::from(&app.queue_footer_projection()));
        self.status
            .adopt(GuiStatusBar::from(&app.status_bar_projection()));
        // The anchor reads the same projection read and the same now-playing
        // marker the queue section built from, so it cannot disagree with
        // them (design D5).
        self.playhead_anchor
            .sample(&mut self.playhead, &playback, queue, now);
        self.adopt_marked_gated(app, &marked, &playback);
        if self.audit {
            self.audit_clean_gated(app, &marked, &playback);
        }
        self.take_batch()
    }

    /// The audit of design D3: build each clean gated section (one without a
    /// mark this collect) and check that the build equals its last built value.
    /// A mismatch means a source changed with no mark, so the section would stay
    /// stale in production.
    fn audit_clean_gated(
        &self,
        app: &mut App,
        marked: &HashSet<Section>,
        playback: &QueuePlaybackProjection,
    ) {
        if !marked.contains(&Section::Theme) {
            audit_clean(Section::Theme, &self.theme, &GuiTheme::current());
        }
        if !marked.contains(&Section::Library) {
            audit_clean(
                Section::Library,
                &self.library,
                &library::build_content(app),
            );
        }
        if !marked.contains(&Section::Search) {
            let search = GuiSearchFrame::carried(&app.gui_search_projection());
            audit_clean(Section::Search, &self.search, &search);
        }
        if !marked.contains(&Section::Settings) {
            let settings = GuiSettingsFrame::build(app, playback);
            audit_clean(Section::Settings, &self.settings, &settings);
        }
    }

    /// Build each gated section that carries a mark (design D2). A mark that
    /// changes nothing sends nothing, because the build compares equal.
    fn adopt_marked_gated(
        &mut self,
        app: &mut App,
        marked: &HashSet<Section>,
        playback: &QueuePlaybackProjection,
    ) {
        if marked.contains(&Section::Theme) {
            self.theme.adopt(GuiTheme::current());
        }
        if marked.contains(&Section::Library) {
            let before = self.library.revision;
            self.library.adopt(library::build_content(app));
            if self.library.revision != before
                && let Some(Some(content)) = self.library.value.as_ref()
            {
                library::log_built(self.library.revision, content);
            }
        }
        if marked.contains(&Section::Search) {
            self.search
                .adopt(GuiSearchFrame::carried(&app.gui_search_projection()));
        }
        if marked.contains(&Section::Settings) {
            self.settings.adopt(GuiSettingsFrame::build(app, playback));
        }
    }

    /// The scrubber's hover tile this tick (task 6.7's rule, with the sheet
    /// image always carried). Leaving the bar ends the hover episode, as the
    /// tick's `hover_tile` does.
    fn hover_tile(
        &mut self,
        app: &mut App,
        playback: &QueuePlaybackProjection,
        surface: &QueuePlaybackSurface,
    ) -> Option<GuiScrubberTile> {
        let Some(fraction) = surface.scrubber_hover() else {
            app.trickplay_hover_ended();
            return None;
        };
        let ticks = scrubber_ticks(playback, fraction);
        let preview = app.trickplay_preview_tile(ticks)?;
        Some(self.trickplay.current_tile(&preview))
    }

    /// The batch of every slot the window does not hold, or `None` when the
    /// batch is empty. Each sent slot is held after this.
    fn take_batch(&mut self) -> Option<GuiSectionBatch> {
        let batch = GuiSectionBatch {
            attach: self.attach.take(),
            theme: self.theme.take(),
            focus: self.focus.take(),
            queue: self.queue.take(),
            summary: self.summary.take(),
            panel: self.panel.take(),
            overlay: self.overlay.take(),
            playback: self.playback.take(),
            hover: self.hover.take(),
            footer: self.footer.take(),
            status: self.status.take(),
            library: self.library.take(),
            search: self.search.take(),
            settings: self.settings.take(),
            playhead: self.playhead.take(),
        };
        (!batch.is_empty()).then_some(batch)
    }
}
