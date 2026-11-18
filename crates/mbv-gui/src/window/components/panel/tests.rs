//! Headless tests of the player panel (D16): the contracts of
//! `Panel.test.ts`. `Panel` itself owns the four states the container size
//! and the core's two toggles pick, and the composition each state paints;
//! this module also carries the 5.7 children's seeded painted-content
//! contracts — the compact strip's meta line with and without a next item,
//! the folded queue's remaining count and finish clock, the rail's
//! remaining time, expand labels, and dim fill — and the region orders the
//! the former tests awaited. The footer line's own painted content and its
//! regionless report are `queue_footer/tests.rs`; the Queue rows' are
//! `queue_panel/tests/`. The clocks are the injected recorders over a fixed
//! wall clock (D10); the core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, UserAgentStylesheet, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::Panel;
use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::frame::playback::{GuiPlayhead, GuiQueuePlayback, GuiTransportAvailability};
use crate::bridge::frame::queue::{GuiQueueHeaderFacts, GuiQueueSummary};
use crate::bridge::layout::{LayoutSize, QueuePlaybackTargetId, RegionTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::layout::LayoutEpoch;
use crate::window::playhead::{Playhead, Times};
use crate::window::sections::Sections;
use crate::window::time::format_wall_clock;

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`).
const SECOND: i64 = 10_000_000;

/// The fixed wall clock the seeded tests stamp the playhead over. The
/// expected finish strings derive from it with the shared formatter, so the
/// assertions hold in every zone.
const WALL_MS: i64 = 1_700_000_000_000;

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
    }
}

/// The summary the seeded tests paint: one remaining row, the queue's
/// finish 43 minutes past the transport's finish, no filter, and one local
/// queue with no peer (so no chooser and a disabled close control).
fn summary() -> GuiQueueSummary {
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
            entries: 1,
            shown_name: "Default".to_owned(),
            close_disabled: true,
        },
    }
}

/// The summary with the next item's title set (the strip's meta line).
fn summary_with_next(next: Option<&str>) -> GuiQueueSummary {
    GuiQueueSummary {
        next_title: next.map(str::to_owned),
        tail_secs: None,
        ..summary()
    }
}

/// The playing transport band the strip's tests paint (the TypeScript
/// fixture's `playing()`, trimmed to what the tests read).
fn playback() -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: "poo".to_owned(),
        host_is_remote: true,
        shown: true,
        show_controls: true,
        now_playing_title: Some("Track title".to_owned()),
        parent_line: Some("The Show".to_owned()),
        episode_badge: None,
        still_caption: None,
        chapters: Vec::new(),
        intro_mark: None,
        credits_mark: None,
        trickplay: None,
        availability: GuiTransportAvailability {
            stop: true,
            next: true,
            previous: false,
        },
        status_indicators: None,
        artwork: None,
        track_chips: None,
        target_choices: Vec::new(),
    }
}

/// A container tall enough for the expanded artboard (the test's `TALL`).
const TALL: LayoutSize = LayoutSize {
    width: 640.0,
    height: 900.0,
};

/// A container shorter than the expanded artboard's 520 px (the test's
/// `SHORT`).
const SHORT: LayoutSize = LayoutSize {
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

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    playhead: Playhead,
    epoch: LayoutEpoch,
    receiver: Receiver<CoreInput>,
}

impl Fixture {
    /// The whole text the document paints, one node per line.
    fn text(&self) -> String {
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
    fn presents(&self, selector: &str) -> bool {
        let dom = self.doc.inner();
        matches!(dom.query_selector(selector), Ok(Some(_)))
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed.
    fn poll_until_idle(&mut self) {
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
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// The region targets of the last merged layout report.
    fn last_reported_targets(&self) -> Vec<RegionTarget> {
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
    fn seed(&mut self, summary: GuiQueueSummary) {
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

    /// Paint the transport band's facts, as the strip's tests seed it.
    fn set_playback(&mut self, playback: GuiQueuePlayback) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || {
            sections.playback.set(playback);
        });
        self.poll_until_idle();
        self.lay_out();
    }

    /// The whole text the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
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
    fn style_of(&self, selector: &str) -> String {
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
    fn attr_of(&self, selector: &str, attribute: blitz_dom::LocalName) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")?;
        dom.get_node(id)?.attr(attribute).map(str::to_owned)
    }

    /// Write the core's two toggles, as the drain task would, and let the
    /// re-render land.
    fn set_toggles(&mut self, collapsed: bool, queue_folded: bool) {
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

fn built(container: LayoutSize) -> Fixture {
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

/// Contract (`Panel.test.ts`): a tall container renders the expanded
/// artboard — the queue rows (not their folded summary) and the footer line
/// mount under the `expanded` state. The artboard's own painted content
/// (the title, the rows, the footer pills) asserts with its row's landing.
#[test]
fn a_tall_container_renders_the_expanded_artboard() {
    let fixture = built(TALL);
    assert!(
        fixture.presents("[data-panel-state=\"expanded\"]"),
        "the expanded state paints"
    );
    assert!(
        fixture.presents("[data-queue-list]"),
        "the expanded artboard paints the queue rows"
    );
    assert!(
        fixture.presents("[data-footer-line]"),
        "the expanded artboard paints the footer line"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"FoldedQueue\"]"),
        "the expanded artboard does not fold the queue"
    );
}

/// Contract (`Panel.test.ts`): a container shorter than 520 px renders the
/// compact strip — the strip mounts under the `compactStrip` state and the
/// expanded artboard does not.
#[test]
fn a_container_shorter_than_520px_renders_the_compact_strip() {
    let fixture = built(SHORT);
    assert!(
        fixture.presents("[data-panel-state=\"compactStrip\"]"),
        "the compact strip state paints"
    );
    assert!(
        fixture.presents("[data-target=\"visualSlot\"]"),
        "the compact strip paints the strip"
    );
    assert!(
        !fixture.presents("[data-target=\"playbackTarget\"]"),
        "the compact strip paints no expanded artboard"
    );
}

/// Contract (`Panel.test.ts`): the queue-fold toggle replaces the rows with
/// the folded summary — the `expandedWithoutQueue` state paints the folded
/// queue and not the queue rows.
#[test]
fn the_queue_fold_toggle_replaces_the_queue_rows_with_the_folded_summary() {
    let mut fixture = built(TALL);
    fixture.set_toggles(false, true);
    assert!(
        fixture.presents("[data-panel-state=\"expandedWithoutQueue\"]"),
        "the folded state paints"
    );
    assert!(
        fixture.presents("[data-target=\"queueFold\"]"),
        "the folded state paints the one-line summary"
    );
    assert!(
        !fixture.presents("[data-queue-list]"),
        "the folded state paints no queue rows"
    );
}

/// Contract (`Panel.test.ts`): the collapse toggle renders the 72 px rail —
/// the rail mounts with no state wrapper around it (the rail paints its own
/// `data-panel-state`), and no expanded artboard mounts.
#[test]
fn the_collapse_toggle_renders_the_rail() {
    let mut fixture = built(TALL);
    fixture.set_toggles(true, false);
    assert!(
        fixture.presents("[data-panel-state=\"rail\"]"),
        "the collapsed state paints the rail"
    );
    // The rail paints its own state attribute, and no state wrapper mounts
    // around it.
    assert!(
        !fixture.presents("[data-panel-state=\"expanded\"]"),
        "the rail mounts no expanded artboard"
    );
    assert!(
        !fixture.presents("[data-target=\"playbackTarget\"]"),
        "the rail paints no expanded artboard"
    );
}

/// Contract (`Panel.test.ts`): the compact strip reports the still's slot
/// and the transport's two controls — `visualSlot`, `playPause`, `next` —
/// its whole region list, since no expanded artboard mounts beside it.
#[test]
fn the_compact_strip_reports_the_still_and_transport_regions() {
    let fixture = built(SHORT);

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
        ]
    );
}

/// Contract (`Panel.test.ts`): the folded queue reports the expand control's
/// `queueFold` target — the line's one interactive target.
#[test]
fn the_folded_queue_reports_the_expand_control_region() {
    let mut fixture = built(TALL);
    fixture.set_toggles(false, true);

    let targets = fixture.last_reported_targets();
    assert!(
        targets.contains(&RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::QueueFold
        )),
        "{targets:?}"
    );
}

/// Contract (`Panel.test.ts`): the rail reports the transport's two controls,
/// the artwork slot, and the expand control, in paint order — its whole
/// region list, since the queue and the footer belong to the expanded
/// artboard.
#[test]
fn the_rail_reports_the_transport_artwork_and_expand_regions() {
    let mut fixture = built(TALL);
    fixture.set_toggles(true, false);

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse),
        ]
    );
}

/// Contract (`Panel.test.ts`): the compact strip paints the meta line with
/// the next item — the parent line and `Next: <title>` joined as one
/// truncating line — and no queue row paints (the next item is a summary
/// datum, not a painted row of its own).
#[test]
fn the_compact_strip_paints_the_meta_line_with_the_next_item() {
    let mut fixture = built(SHORT);
    fixture.set_playback(playback());
    fixture.seed(summary_with_next(Some("Second track")));

    assert!(fixture.text().contains("The Show · Next: Second track"));
    assert!(
        !fixture.presents("[data-slot]"),
        "the strip paints no queue row"
    );
}

/// Contract (`Panel.test.ts`): the strip paints the parent line alone while
/// no slot follows the playhead.
#[test]
fn the_strip_paints_the_parent_line_alone_while_no_slot_follows() {
    let mut fixture = built(SHORT);
    fixture.set_playback(playback());
    fixture.seed(summary_with_next(None));

    let text = fixture.text();
    assert!(text.contains("The Show"), "{text}");
    assert!(!text.contains("Next:"), "{text}");
}

/// Contract (`Panel.test.ts`): the folded queue paints the summary line —
/// the remaining count, the finish clock at the summary's tail past the
/// transport's finish, and the `Expand queue` control.
#[test]
fn the_folded_queue_paints_the_summary_line() {
    let mut fixture = built(TALL);
    fixture.seed(summary());
    fixture.set_toggles(false, true);

    assert_eq!(fixture.text_of("[data-remaining]"), "1");
    // The queue's finish is the transport's finish (the wall clock plus the
    // active item's remaining 140 s) plus the tail's 43 minutes; the
    // expectation derives from the same fixed clock with the shared
    // formatter, so the assertion holds in every zone.
    let expected_finish = format_wall_clock(WALL_MS.div_euclid(1000) + 140 + 43 * 60);
    assert_eq!(fixture.text_of("[data-finish-clock]"), expected_finish);
    assert_eq!(
        fixture
            .attr_of("[data-target=\"queueFold\"]", local_name!("aria-label"))
            .as_deref(),
        Some("Expand queue")
    );
}

/// Contract (`Panel.test.ts`): the rail paints the remaining time, the
/// expand control, and the dim canvas fill (the TUI's `BgDim` slot).
#[test]
fn the_rail_paints_the_remaining_time_the_expand_control_and_the_dim_fill() {
    let mut fixture = built(TALL);
    fixture.seed(summary());
    fixture.set_toggles(true, false);

    // The frozen prediction leaves 140 s of the 200 s item: the same
    // `-M:SS` the expanded scrubber's right end paints.
    assert!(fixture.text().contains("-2:20"));
    assert_eq!(
        fixture
            .attr_of("[data-target=\"collapse\"]", local_name!("aria-label"))
            .as_deref(),
        Some("Expand panel")
    );
    let rail = fixture.style_of("[data-panel-state=\"rail\"]");
    assert!(rail.contains("background: var(--canvas-dim)"), "{rail}");
}

/// Contract (live run of PR 1003, the live-run defect of
/// `replace-tauri-with-blitz` §11): the ring strokes the played-fraction
/// track and arc in their own roles. Blitz paints an inline `svg` through
/// usvg, which cannot parse a `var()` in a `stroke` attribute — it drops the
/// stroke, so both circles painted nothing. Each circle names its role in the
/// computed `color` instead, and the serialiser writes it into the attribute.
#[test]
fn the_rail_ring_strokes_both_circles_in_their_theme_roles() {
    let mut fixture = built(TALL);
    fixture.seed(summary());
    fixture.set_toggles(true, false);

    assert_eq!(
        svg_strokes(&fixture.doc, "[data-target=\"visualSlot\"] svg"),
        vec![Some((17, 34, 51)), Some((68, 85, 102))]
    );
}

/// The stroke colour of every path in the svg the selector names, in document
/// order: the `usvg` tree Blitz parsed from the serialised subtree, which is
/// what the painter paints. A stroke usvg could not parse reads as `None`.
fn svg_strokes(doc: &DioxusDocument, selector: &str) -> Vec<Option<Rgb>> {
    let dom = doc.inner();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    let tree = dom
        .get_node(id)
        .and_then(|node| node.element_data())
        .and_then(|element| element.svg_data())
        .unwrap_or_else(|| panic!("{selector} paints as an svg tree"));
    let mut strokes = Vec::new();
    collect_strokes(tree.root(), &mut strokes);
    strokes
}

/// A paint's colour, as rgb bytes.
type Rgb = (u8, u8, u8);

/// Push the stroke of every path of `group` and its nested groups, in
/// document order: usvg groups the view box transform's children.
fn collect_strokes(group: &usvg::Group, strokes: &mut Vec<Option<Rgb>>) {
    for child in group.children() {
        match child {
            usvg::Node::Path(path) => strokes.push(path.stroke().map(|stroke| rgb(stroke.paint()))),
            usvg::Node::Group(nested) => collect_strokes(nested, strokes),
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

/// A flat paint's colour.
fn rgb(paint: &usvg::Paint) -> Rgb {
    match paint {
        usvg::Paint::Color(color) => (color.red, color.green, color.blue),
        _ => panic!("the ring paints in a flat colour"),
    }
}
