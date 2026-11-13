//! The deadline and playhead compares of design D3, rules 3 and 4. Each tick
//! samples the App's time-driven and playhead facts, and marks the gated
//! sections whose sources changed since the last sample. A change is the mark:
//! the core needs no deadline instant, only that the fact moved.

use std::collections::HashSet;
use std::time::Instant;

use mbv::{App, GuiPlayheadMarks, PlaybackTargetChoice, QueuePlaybackProjection};

use super::Section;

/// What the last tick sampled. The next sample compares against it.
#[derive(Debug)]
pub(super) struct DeadlineMarks {
    /// Whether the navigation-idle gate was open at the last sample.
    navigation_idle: bool,
    /// The tick the last sample ran at. An interrupted art retry falls in the
    /// window from here to the next sample.
    art_retry_checked: Instant,
    /// The playing track and book at the last sample.
    playhead: GuiPlayheadMarks,
    /// The playback target choices at the last sample; the Sessions section
    /// reads them.
    target_choices: Vec<PlaybackTargetChoice>,
}

impl DeadlineMarks {
    /// The sampler at construction: the first sample's retry window starts here.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            navigation_idle: false,
            art_retry_checked: now,
            playhead: GuiPlayheadMarks::default(),
            target_choices: Vec::new(),
        }
    }

    /// Sample the App's deadline and playhead facts and mark what changed. The
    /// navigation gate marks on its opening (not idle to idle), because that is
    /// the Hero neighbour warm-up's deadline; its closing is not a mark. The
    /// retry query and the playhead mark the Library section. The target choices
    /// mark the Settings section.
    pub(super) fn sample(
        &mut self,
        app: &App,
        playback: &QueuePlaybackProjection,
        now: Instant,
        marked: &mut HashSet<Section>,
    ) {
        let idle = app.gui_navigation_idle();
        if idle && !self.navigation_idle {
            marked.insert(Section::Library);
        }
        self.navigation_idle = idle;
        if app.gui_art_retry_due(self.art_retry_checked, now) {
            marked.insert(Section::Library);
        }
        self.art_retry_checked = self.art_retry_checked.max(now);
        let playhead = app.gui_playhead_marks();
        if playhead != self.playhead {
            marked.insert(Section::Library);
            self.playhead = playhead;
        }
        if playback.target_choices != self.target_choices {
            marked.insert(Section::Settings);
            self.target_choices.clone_from(&playback.target_choices);
        }
    }
}
