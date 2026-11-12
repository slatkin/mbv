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

pub(crate) mod library;

use std::collections::HashSet;

use mbv::{App, QueuePlaybackProjection};
use mbv_surfaces::FocusStack;
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;

use crate::artwork::ArtworkFrames;
use crate::bridge::frame::chrome::{GuiQueueFooter, GuiStatusBar};
use crate::bridge::frame::focus::GuiFocus;
use crate::bridge::frame::library::{GuiLibraryContent, GuiSearchFrame};
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{GuiQueuePlayback, GuiScrubberHover, GuiScrubberTile};
use crate::bridge::frame::queue::{GuiQueue, GuiQueueSummary};
use crate::bridge::frame::settings::GuiSettingsFrame;
use crate::bridge::frame::theme::GuiTheme;
use crate::bridge::frame::{GuiPanelToggles, scrubber_hover, scrubber_ticks};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::trickplay::TrickplayFrames;

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

/// The section ledger (design D2). Each field is one section's slot. The marks
/// name the gated sections whose sources changed since the last collect.
#[derive(Debug)]
pub struct SectionLedger {
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
    marked: HashSet<Section>,
    artwork: ArtworkFrames,
    trickplay: TrickplayFrames,
}

impl Default for SectionLedger {
    /// A fresh ledger holds no section. The theme carries one mark from the
    /// start, so its first collect builds it (design D4).
    fn default() -> Self {
        let mut ledger = Self {
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
            marked: HashSet::new(),
            artwork: ArtworkFrames::default(),
            trickplay: TrickplayFrames::default(),
        };
        ledger.mark(Section::Theme);
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
        for section in [
            Section::Theme,
            Section::Library,
            Section::Search,
            Section::Settings,
        ] {
            self.mark(section);
        }
    }

    /// Forget every section the window holds (design D4, the `sync` path). The
    /// next collect sends every present section at its current revision.
    pub fn release_all(&mut self) {
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
        } = sources;
        let marked = std::mem::take(&mut self.marked);
        // One playback projection read feeds the playback section, the overlay,
        // the hover's tick, and the settings build, so they cannot disagree.
        let playback = app.queue_playback_projection();
        let summary = app.queue_summary_projection();
        self.focus.adopt(GuiFocus::from(focus));
        self.queue
            .adopt(GuiQueue::from_surface(queue, viewport_rows, &summary));
        self.summary.adopt(GuiQueueSummary::from(&summary));
        self.panel
            .adopt(GuiPanelToggles::from(&app.panel_toggles_projection()));
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
        self.adopt_marked_gated(app, &marked, &playback);
        self.take_batch()
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
    /// frame path's `hover_tile` does.
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
            ..GuiSectionBatch::default()
        };
        (!batch.is_empty()).then_some(batch)
    }
}
