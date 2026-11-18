//! The shared fixture of the player panel's headless tests: the context
//! providers the root hands a mounted panel, the section stores, the frozen
//! clocks, the playhead anchor, the frame builders, and the document
//! wrapper's text, style, and report readers.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, UserAgentStylesheet, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::super::Panel;
use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{
    GuiPlayerControls, GuiPlayhead, GuiQueuePlayback, GuiTrackChip, GuiTrackChips,
    GuiTransportAvailability,
};
use crate::bridge::frame::queue::{GuiQueueHeaderFacts, GuiQueueSummary};
use crate::bridge::layout::{LayoutSize, RegionTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::layout::LayoutEpoch;
use crate::window::playhead::{Playhead, Times};
use crate::window::sections::Sections;

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`).
pub(super) const SECOND: i64 = 10_000_000;

/// The fixed wall clock the seeded tests stamp the playhead over. The
/// expected finish strings derive from it with the shared formatter, so the
/// assertions hold in every zone.
pub(super) const WALL_MS: i64 = 1_700_000_000_000;

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the panel tree
/// sizes its boxes with (the same rules the Queue panel's harness uses), so
/// the `Truncate` measures read laid-out widths.
const FIXTURE_CSS: &str = concat!(
    "[data-slot] { width: 100%; }",
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".justify-center { justify-content: center; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".shrink-0 { flex-shrink: 0; }",
    ".w-full { width: 100%; }",
    ".h-full { height: 100%; }",
    ".block { display: block; }",
    ".truncate { width: 100%; }",
    ".absolute { position: absolute; }",
    ".fixed { position: fixed; }",
    // The two roles the rail's ring reads, at rgb values the ring's test names
    // (the theme's `:root` rule in a live window).
    ":root { --mbv-progressTrack: rgb(17, 34, 51); --mbv-accent: rgb(68, 85, 102); }",
);

/// The fixture's clocks: a frozen monotonic clock (the prediction stays at
/// its anchor) and the fixed wall clock (D10).
struct FakeTimes {
    wall_ms: Mutex<i64>,
}

impl Times for FakeTimes {
    fn now_ms(&self) -> f64 {
        0.0
    }

    fn wall_now_ms(&self) -> i64 {
        *self.wall_ms.lock().expect("the test clock lives")
    }
}

/// The playhead anchor the seeded tests stamp: playing a 200 s item at its
/// 60th second, finishing 140 s after the fixed wall clock.
fn anchor() -> GuiPlayhead {
    GuiPlayhead {
        active: true,
        active_slot: None,
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
        buffered_ticks: 90 * SECOND,
    }
}

/// The summary the seeded tests paint: one remaining row, the queue's
/// finish 43 minutes past the transport's finish, no filter, and one local
/// queue with no peer (so a disabled close control).
pub(super) fn summary() -> GuiQueueSummary {
    GuiQueueSummary {
        remaining: 1,
        next_title: None,
        tail_secs: Some(43 * 60),
        filter: String::new(),
        filter_active: false,
        total_secs: 6 * 60 * 60,
        total_partial: false,
        marked_secs: None,
        marked_partial: false,
        header: GuiQueueHeaderFacts {
            shown_name: "Default".to_owned(),
            shown_len: 26,
            close_disabled: true,
        },
    }
}

/// The summary with the next item's title set (the strip's meta line).
pub(super) fn summary_with_next(next: Option<&str>) -> GuiQueueSummary {
    GuiQueueSummary {
        next_title: next.map(str::to_owned),
        tail_secs: None,
        ..summary()
    }
}

/// The playing transport band the strip's tests paint (the TypeScript
/// fixture's `playing()`, trimmed to what the tests read).
pub(super) fn playback() -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: "poo".to_owned(),
        host_is_remote: true,
        shown: true,
        show_controls: true,
        now_playing_title: Some("Track title".to_owned()),
        parent_line: Some("The Show".to_owned()),
        episode_badge: None,
        availability: GuiTransportAvailability {
            stop: true,
            next: true,
            previous: false,
        },
        status_indicators: None,
        artwork: None,
        track_chips: None,
        target_choices: Vec::new(),
        controls: GuiPlayerControls::default(),
    }
}

/// The track chips the frame carries over a playing video item: a video label
/// and two chips with no chosen choice, so each paints its kind's fallback
/// label.
pub(super) fn chips() -> GuiTrackChips {
    GuiTrackChips {
        video: Some("1080p".to_owned()),
        audio: GuiTrackChip {
            choices: Vec::new(),
            chosen: None,
        },
        subtitles: GuiTrackChip {
            choices: Vec::new(),
            chosen: None,
        },
        audio_lang_code: String::new(),
    }
}

/// The stopped frame of `gui-art-player-overlay` D7: no title, no artwork, the
/// controls behind `show_controls`, and `can_start` the transport gate.
pub(super) fn stopped(can_start: bool) -> GuiQueuePlayback {
    GuiQueuePlayback {
        show_controls: false,
        now_playing_title: None,
        artwork: None,
        controls: GuiPlayerControls {
            auto_hide: false,
            can_start,
        },
        ..playback()
    }
}

/// A container tall enough for the expanded artboard (the test's `TALL`).
pub(super) const TALL: LayoutSize = LayoutSize {
    width: 640.0,
    height: 900.0,
};

/// A container shorter than the expanded artboard's 520 px (the test's
/// `SHORT`).
pub(super) const SHORT: LayoutSize = LayoutSize {
    width: 640.0,
    height: 480.0,
};

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    container: LayoutSize,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the section stores, the
/// container size the test drives, and the playhead.
type FixtureHandles = (Sections, Signal<LayoutSize>, Playhead, LayoutEpoch);

/// The root: provides every context the panel's tree consumes and paints the
/// panel, as `App` composes it inside its Queue column.
fn panel_app(props: FixtureProps) -> Element {
    provide_context(CoreLink::new(props.sender));
    let reporter = crate::window::layout::use_layout_reporter();
    provide_context(reporter);
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    // The art painter's retry clock (D7): the same context the app root
    // provides.
    provide_context(Arc::clone(&clock));
    let times: Arc<dyn Times> = Arc::new(FakeTimes {
        wall_ms: Mutex::new(WALL_MS),
    });
    let playhead = use_hook(move || Playhead::new(Arc::clone(&clock), times));
    provide_context(playhead.clone());
    provide_context(use_hook(ArtEvents::new));
    provide_context(use_hook(FiredTimers::new));
    provide_context(ScaleFactor(Signal::new(1.0)));
    let container = use_signal(move || props.container);
    props
        .handle
        .borrow_mut()
        .replace((sections, container, playhead, epoch));
    rsx! {
        Panel {
            container: container.cloned(),
            library_visible: false,
        }
    }
}

pub(super) struct Fixture {
    pub(super) doc: DioxusDocument,
    pub(super) sections: Sections,
    pub(super) playhead: Playhead,
    pub(super) epoch: LayoutEpoch,
    pub(super) receiver: Receiver<CoreInput>,
}

impl Fixture {
    /// The whole text the document paints, one node per line.
    pub(super) fn text(&self) -> String {
        let dom = self.doc.inner();
        let mut out = String::new();
        for id in blitz_dom::traversal::TreeTraverser::new(&dom) {
            if let Some(node) = dom.get_node(id) {
                out.push_str(&node.text_content());
                out.push('\n');
            }
        }
        out
    }

    /// Whether the selector names a node in the painted tree.
    pub(super) fn presents(&self, selector: &str) -> bool {
        let dom = self.doc.inner();
        matches!(dom.query_selector(selector), Ok(Some(_)))
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed.
    pub(super) fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, so the `Truncate` measures read laid-out widths, and let the
    /// re-measures the epoch bump drives land.
    pub(super) fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// The region targets of the last merged layout report.
    pub(super) fn last_reported_targets(&self) -> Vec<RegionTarget> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Layout(report) => Some(report),
                _ => None,
            })
            .last()
            .map(|report| {
                report
                    .regions
                    .into_iter()
                    .map(|region| region.target)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Stamp the playhead anchor over the fixed clock and paint the
    /// summary, as the former fixture's `seed` did.
    pub(super) fn seed(&mut self, summary: GuiQueueSummary) {
        let mut sections = self.sections.clone();
        let mut playhead = self.playhead.clone();
        let anchor = anchor();
        self.doc.vdom.in_runtime(move || {
            sections.summary.set(summary);
            playhead.stamp(anchor);
        });
        self.poll_until_idle();
        self.lay_out();
    }

    /// Paint the open overlay's frame, as the player menu's tests seed it.
    pub(super) fn set_overlay(&mut self, overlay: Option<GuiOverlayFrame>) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || {
            sections.overlay.set(overlay);
        });
        self.poll_until_idle();
        self.lay_out();
    }

    /// Paint the transport band's facts, as the strip's tests seed it.
    pub(super) fn set_playback(&mut self, playback: GuiQueuePlayback) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || {
            sections.playback.set(playback);
        });
        self.poll_until_idle();
        self.lay_out();
    }

    /// The whole text the first node the selector names.
    pub(super) fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
    }

    /// The `style` attribute of the first node the selector names.
    pub(super) fn style_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .expect("the node paints");
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(local_name!("style"))
            .map(str::to_owned)
            .unwrap_or_default()
    }

    /// The attribute of the first node the selector names.
    pub(super) fn attr_of(
        &self,
        selector: &str,
        attribute: blitz_dom::LocalName,
    ) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")?;
        dom.get_node(id)?.attr(attribute).map(str::to_owned)
    }

    /// Write the core's two toggles, as the drain task would, and let the
    /// re-render land.
    pub(super) fn set_toggles(&mut self, collapsed: bool, queue_folded: bool) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || {
            sections.panel.set(GuiPanelToggles {
                collapsed,
                queue_folded,
                ui_reset_epoch: 0,
            });
        });
        self.poll_until_idle();
        self.lay_out();
    }
}

pub(super) fn built(container: LayoutSize) -> Fixture {
    let (sender, receiver) = channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let config = DocumentConfig {
        viewport: Some(Viewport::new(640, 900, 1.0, ColorScheme::Light)),
        ua_stylesheets: vec![
            UserAgentStylesheet::default_sheet(),
            UserAgentStylesheet::parse(FIXTURE_CSS),
        ],
        ..DocumentConfig::default()
    };
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            panel_app,
            FixtureProps {
                sender,
                container,
                handle: Rc::clone(&handle),
            },
        ),
        config,
    );
    doc.initial_build();
    let (sections, _container, playhead, epoch) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the section stores over");
    let mut fixture = Fixture {
        doc,
        sections,
        playhead,
        epoch,
        receiver,
    };
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
}
