//! Headless tests of the Queue panel (D16): the contracts of
//! `QueuePanel.test.ts` the window's port keeps — the frame's viewport rows
//! with their watched, active, and partial-progress states, the empty-queue
//! state, the rows' right column under the summary's mode, the hovered row's
//! grip and actions button, the open row menu's chrome and hints, the drag's
//! ghost and insertion line, and the capacity and region targets the panel
//! reports — plus the two region-order contracts
//! `QueuePlaybackPanel.test.ts` deferred until this panel existed (an open
//! chooser's rows reporting after the queue's, and the full player panel's
//! region order). The presses are not ported: they are the root input
//! layer's contract (`window/input.rs`), which the panel forwards to
//! untouched. The header's own contracts (the header tests of
//! `QueuePanel.test.ts`) travel with the header's port, task 5.6. The core
//! channel is a mock pair; the clocks are the injected recorders (D10); no
//! test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use blitz_dom::{Document as _, UserAgentStylesheet, local_name, traversal::TreeTraverser};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::QueuePanel;
use crate::art::GuiArtRef;
use crate::bridge::frame::focus::{GuiFocus, GuiPanel, GuiSurface};
use crate::bridge::frame::overlay::{GuiOverlayFrame, GuiRowMenuEntry};
use crate::bridge::frame::playback::{
    GuiNowPlayingStatus, GuiPlayhead, GuiQueuePlayback, GuiTargetChoice, GuiTargetKey,
    GuiTransportAvailability,
};
use crate::bridge::frame::queue::{
    GuiMediaKind, GuiQueue, GuiQueueDrag, GuiQueueInsertion, GuiQueueRow, GuiQueueSummary,
    GuiRightColumn, GuiSemanticState, GuiTrailing,
};
use crate::bridge::layout::{
    LayoutReport, ListId, ListRows, QueuePlaybackTargetId, QueueTargetId, RegionTarget,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::components::queue_playback_panel::QueuePlaybackPanel;
use crate::window::layout::{LayoutEpoch, use_layout_reporter};
use crate::window::playhead::{Playhead, Times};
use crate::window::sections::Sections;
use crate::window::time::format_wall_clock;

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`).
const SECOND: i64 = 10_000_000;

/// The fixed wall clock the tests stamp the playhead over: the port of the
/// TypeScript fixture's test clock. The expected finish strings derive from
/// it with the shared formatter, so the assertion holds in every zone.
const WALL_MS: i64 = 1_700_000_000_000;

/// A queue of `rows`, no cursor, marks, active slot, hover, or drag.
fn queue(rows: Vec<GuiQueueRow>) -> GuiQueue {
    GuiQueue {
        rows,
        cursor: None,
        marks: Vec::new(),
        active: None,
        hover: None,
        drag: None,
    }
}

/// The summary the fixture paints: two remaining rows, the queue's finish
/// 43 minutes past the transport's finish, the "Length" mode, no filter.
fn summary() -> GuiQueueSummary {
    GuiQueueSummary {
        remaining: 2,
        next_title: None,
        tail_secs: Some(43 * 60),
        right_column: GuiRightColumn::Length,
        filter: String::new(),
        filter_active: false,
    }
}

/// The summary in the "Ends at" mode.
fn ends_at_summary() -> GuiQueueSummary {
    GuiQueueSummary {
        right_column: GuiRightColumn::EndsAt,
        ..summary()
    }
}

/// One item row for `slot` (the TypeScript `item` helper): `Track <slot>` in
/// the "Length" mode, ending 40 minutes after the transport's finish.
fn item(slot: u64) -> GuiQueueRow {
    GuiQueueRow::Item {
        slot,
        ordinal: usize::try_from(slot).unwrap_or(0),
        primary: format!("Track {slot}"),
        secondary: None,
        trailing: None,
        duration: Some("3:12".to_owned()),
        ends_after_secs: Some(40 * 60),
        kind: GuiMediaKind::Media,
        state: GuiSemanticState::Ordinary,
        striped: false,
    }
}

/// The item row with its title replaced.
fn renamed(mut row: GuiQueueRow, primary: &str) -> GuiQueueRow {
    if let GuiQueueRow::Item { primary: field, .. } = &mut row {
        *field = primary.to_owned();
    }
    row
}

/// The item row with its semantic state replaced.
fn in_state(mut row: GuiQueueRow, state: GuiSemanticState) -> GuiQueueRow {
    if let GuiQueueRow::Item { state: field, .. } = &mut row {
        *field = state;
    }
    row
}

/// The item row whose end offset is `secs` seconds past the active item's
/// end, or none.
fn ending(mut row: GuiQueueRow, secs: Option<i64>) -> GuiQueueRow {
    if let GuiQueueRow::Item {
        ends_after_secs: field,
        ..
    } = &mut row
    {
        *field = secs;
    }
    row
}

/// The item row whose duration — the "Length" value — is gone.
fn untimed(mut row: GuiQueueRow) -> GuiQueueRow {
    if let GuiQueueRow::Item {
        duration: field, ..
    } = &mut row
    {
        *field = None;
    }
    row
}

/// The item row with a second line and a date gutter, which the canvas's
/// one-liner row does not paint.
fn with_details(mut row: GuiQueueRow, secondary: &str, trailing: &str) -> GuiQueueRow {
    if let GuiQueueRow::Item {
        secondary: secondary_field,
        trailing: trailing_field,
        ..
    } = &mut row
    {
        *secondary_field = Some(secondary.to_owned());
        *trailing_field = Some(GuiTrailing::Gutter(trailing.to_owned()));
    }
    row
}

/// One row-menu entry (the TypeScript fixture's entries).
fn entry(label: &str, key_hint: Option<&str>, destructive: bool) -> GuiRowMenuEntry {
    GuiRowMenuEntry {
        label: label.to_owned(),
        key_hint: key_hint.map(str::to_owned),
        destructive,
    }
}

/// The open row menu the fixture paints: five entries, the cursor on the
/// second, the destructive `Remove from queue` last.
fn row_menu() -> GuiOverlayFrame {
    GuiOverlayFrame::RowMenu {
        anchor_slot: 2,
        cursor: 1,
        entries: vec![
            entry("Play now", Some("Enter"), false),
            entry("Play next", Some("n"), false),
            entry("Mark watched", Some("w"), false),
            entry("Go to episode", Some("g"), false),
            entry("Remove from queue", Some("dd"), true),
        ],
    }
}

/// The row drag the fixture paints: slot 1 dragged, its ghost rebuilt by the
/// caller, the insertion line anchored to slot 2.
fn drag(ghost: GuiQueueRow, insertion: GuiQueueInsertion) -> GuiQueueDrag {
    GuiQueueDrag {
        slot: 1,
        ghost,
        drop_index: 2,
        insertion,
    }
}

/// The "Ends at" clock one fixture row paints: the wall clock at the row's
/// 40-minute offset past the active item's end (design D5). The active item
/// holds 140 s of its 200 s runtime at the frozen prediction, so its finish
/// is the wall clock plus 140 s.
fn expected_row_end() -> String {
    let finish_secs = WALL_MS.div_euclid(1000) + 140;
    format_wall_clock(finish_secs + 40 * 60)
}

/// The times the tests drive: a frozen monotonic clock (the prediction stays
/// at its anchor) and the fixed wall clock (D10).
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

/// The playhead anchor the fixture stamps: playing a 200 s item at its 60th
/// second, with no slot carrying the anchor (the TypeScript fixture's stamp).
fn anchor() -> GuiPlayhead {
    GuiPlayhead {
        active: true,
        active_slot: None,
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
    }
}

/// The contexts the root hands the test: the section stores, the playhead,
/// and the layout epoch.
type FixtureHandles = (Sections, Playhead, LayoutEpoch);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The fixture's shared context set, as the expanded artboard's root
/// provides it.
fn provide_fixture_contexts(
    sender: Sender<CoreInput>,
    handle: &Rc<RefCell<Option<FixtureHandles>>>,
) {
    provide_context(CoreLink::new(sender));
    let reporter = use_layout_reporter();
    provide_context(reporter);
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let handle_epoch = epoch;
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let times: Arc<dyn Times> = Arc::new(FakeTimes {
        wall_ms: Mutex::new(WALL_MS),
    });
    let art_clock = Arc::clone(&clock);
    let playhead = use_hook(move || Playhead::new(clock, times));
    provide_context(playhead.clone());
    // The art painter's contexts: the art store, the timer router, the clock
    // the retry arms, and the window's scale factor.
    provide_context(use_hook(ArtEvents::new));
    provide_context(use_hook(FiredTimers::new));
    provide_context(art_clock);
    provide_context(ScaleFactor(Signal::new(1.0)));
    handle
        .borrow_mut()
        .replace((sections, playhead, handle_epoch));
}

/// The root: paints the Queue panel, as the expanded artboard composes it.
fn queue_panel_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    provide_fixture_contexts(sender, &handle);
    rsx! {
        QueuePanel {}
    }
}

/// The playing transport band's facts the chooser test paints (the
/// TypeScript `playing()` fixture, trimmed to what the top block reads).
fn playing() -> GuiQueuePlayback {
    GuiQueuePlayback {
        status: GuiNowPlayingStatus::Playing,
        host: "poo".to_owned(),
        host_is_remote: true,
        shown: true,
        show_controls: true,
        now_playing_title: Some("Track title".to_owned()),
        parent_line: Some("The Show".to_owned()),
        episode_badge: Some("S2 · E5".to_owned()),
        still_caption: Some("Episode still · Emby Primary / Thumb".to_owned()),
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
        artwork: Some(GuiArtRef::Image {
            src: "mbv-art://localhost/card/k".to_owned(),
        }),
        track_chips: None,
        target_choices: Vec::new(),
    }
}

/// The open playback-target chooser: two choices, the first attached.
fn chooser() -> GuiOverlayFrame {
    GuiOverlayFrame::TargetChooser {
        cursor: 0,
        choices: vec![
            GuiTargetChoice {
                key: GuiTargetKey::Emby("session-1".to_owned()),
                label: "Living room".to_owned(),
                is_active: true,
            },
            GuiTargetChoice {
                key: GuiTargetKey::Cast("receiver-1".to_owned()),
                label: "Kitchen speaker".to_owned(),
                is_active: false,
            },
        ],
    }
}

/// The root of the chooser's region-order contracts: the player panel's top
/// block and the Queue panel under one reporter, as the expanded artboard
/// composes them (`QueueFooter` joins with its port, task 5.6).
fn panel_tree_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    provide_fixture_contexts(sender, &handle);
    rsx! {
        QueuePlaybackPanel {}
        QueuePanel {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    receiver: Receiver<CoreInput>,
    sections: Sections,
    playhead: Playhead,
    epoch: LayoutEpoch,
}

impl Fixture {
    /// The whole text the document paints.
    fn text(&self) -> String {
        let dom = self.doc.inner();
        let mut out = String::new();
        for id in TreeTraverser::new(&dom) {
            if let Some(node) = dom.get_node(id) {
                out.push_str(&node.text_content());
                out.push('\n');
            }
        }
        out
    }

    /// The text of the first node the selector names.
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

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        let dom = self.doc.inner();
        matches!(dom.query_selector(selector), Ok(Some(_)))
    }

    /// The `style` attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        self.attr_of(selector, local_name!("style"))
            .unwrap_or_else(|| panic!("{selector} paints"))
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: blitz_dom::LocalName) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")?;
        let node = dom.get_node(id)?;
        node.attr(attribute).map(str::to_owned)
    }

    /// The node ids the selector names, in tree order.
    fn all(&self, selector: &str) -> Vec<blitz_dom::NodeId> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .collect()
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Run `f` inside the document's runtime, as the drain task does: a
    /// signal write outside it reaches no subscriber.
    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

    /// The region targets of the last merged layout report.
    fn last_reported_targets(&self) -> Vec<RegionTarget> {
        self.last_report()
            .map(|report| {
                report
                    .regions
                    .into_iter()
                    .map(|region| region.target)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The last merged layout report.
    fn last_report(&self) -> Option<LayoutReport> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Layout(report) => Some(report),
                _ => None,
            })
            .last()
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, so the mounted rects are the laid-out ones, and let the
    /// re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Paint the frame's queue and summary (and `overlay` when one is up),
    /// with the queue surface and panel focused as the TypeScript fixture
    /// sets, and let the re-render land.
    fn show(&mut self, rows: GuiQueue, summary: GuiQueueSummary, overlay: Option<GuiOverlayFrame>) {
        let mut sections = self.sections.clone();
        let anchor = anchor();
        let mut playhead = self.playhead.clone();
        self.doc.vdom.in_runtime(move || {
            sections.queue.set(rows);
            sections.summary.set(summary);
            sections.overlay.set(overlay);
            sections.focus.set(GuiFocus {
                surface: Some(GuiSurface::Queue),
                panel: GuiPanel::Queue,
            });
            playhead.stamp(anchor);
        });
        self.poll_until_idle();
        self.lay_out();
    }
}

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the queue tree
/// sizes its cells with, so a title that fits its box paints whole. The
/// styles the row assertions read are the component's inline styles, which
/// the document applies regardless.
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
    // The title box fills its row, so a title that fits paints whole from
    // the first resolved measure on (a real window's first layout is sized
    // before its first measure; a zero-height start would wedge the box at
    // its cut).
    ".truncate { width: 100%; }",
);

/// A fixture over a `width` × `height` viewport. A zero-sized viewport lays
/// everything out at zero, which the capacity contract reads.
fn built_at(app: fn(FixtureProps) -> Element, width: u32, height: u32) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let config = DocumentConfig {
        viewport: Some(Viewport::new(width, height, 1.0, ColorScheme::Light)),
        ua_stylesheets: vec![
            UserAgentStylesheet::default_sheet(),
            UserAgentStylesheet::parse(FIXTURE_CSS),
        ],
        ..DocumentConfig::default()
    };
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            app,
            FixtureProps {
                sender,
                handle: Rc::clone(&handle),
            },
        ),
        config,
    );
    doc.initial_build();
    let (sections, playhead, epoch) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the contexts over");
    let mut fixture = Fixture {
        doc,
        receiver,
        sections,
        playhead,
        epoch,
    };
    fixture.doc.inner.borrow_mut().resolve(0.0);
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
}

/// A fixture whose document lays out into a 640×900 viewport, so the rows'
/// titles fit their boxes and the `Truncate` measure paints them whole.
fn built_sized(app: fn(FixtureProps) -> Element) -> Fixture {
    built_at(app, 640, 900)
}

/// A fixture whose document lays out into a zero viewport: every rect is
/// zero, so the list's whole-row capacity is its zero-height value.
fn built_unsized(app: fn(FixtureProps) -> Element) -> Fixture {
    built_at(app, 0, 0)
}

/// A fixture whose painted frame is `rows` under `summary` (and `overlay`
/// when one is up).
fn built_with_frame(
    app: fn(FixtureProps) -> Element,
    rows: GuiQueue,
    summary: GuiQueueSummary,
    overlay: Option<GuiOverlayFrame>,
) -> Fixture {
    let mut fixture = built_sized(app);
    fixture.show(rows, summary, overlay);
    fixture
}

/// Contract (`QueuePanel.test.ts`): a frame with slots renders its rows — the
/// item rows' titles and the structural heading among them, and one
/// `[data-slot]` button per item row.
#[test]
fn a_frame_with_slots_renders_its_rows() {
    let rows = vec![
        renamed(item(1), "First track"),
        GuiQueueRow::Heading {
            text: "album".to_owned(),
        },
        renamed(item(2), "Second track"),
    ];
    let fixture = built_with_frame(queue_panel_app, queue(rows), summary(), None);

    let text = fixture.text();
    assert!(text.contains("First track"), "{text}");
    assert!(text.contains("album"), "{text}");
    assert!(text.contains("Second track"), "{text}");
    assert_eq!(fixture.all("[data-slot]").len(), 2);
}

/// Contract (`QueuePanel.test.ts`): an empty frame renders the empty-queue
/// state — the shrug placeholder, and no row buttons.
#[test]
fn an_empty_frame_renders_the_empty_queue_state() {
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), summary(), None);

    assert!(fixture.text().contains("¯\\_(ツ)_/¯"));
    assert_eq!(fixture.all("[data-slot]").len(), 0);
}

/// Contract (`QueuePanel.test.ts`): the right column paints the mode's value
/// as a repeated time — the "Ends at" mode paints the wall clock at the
/// row's end offset in the time token at the repeat opacity, and the
/// "Length" mode paints the duration in the time token.
#[test]
fn the_right_column_paints_the_modes_value_as_a_repeated_time() {
    let ends = built_with_frame(
        queue_panel_app,
        queue(vec![item(1)]),
        ends_at_summary(),
        None,
    );
    let expected = expected_row_end();
    assert_eq!(fixture_text_of_row_value(&ends), expected);
    let token = ends.style_of("[data-meta-kind=\"time\"]");
    assert!(token.contains("color: var(--canvas-type-time)"), "{token}");
    assert!(
        token.contains("opacity: var(--canvas-type-repeat-opacity)"),
        "{token}"
    );
    assert!(!ends.text().contains("3:12"));

    let length = built_with_frame(queue_panel_app, queue(vec![item(1)]), summary(), None);
    assert!(length.text().contains("3:12"));
    let token = length.style_of("[data-meta-kind=\"time\"]");
    assert!(token.contains("color: var(--canvas-type-time)"), "{token}");
}

/// The right-column value of row 1 (the helper the repeated-time contract
/// reads).
fn fixture_text_of_row_value(fixture: &Fixture) -> String {
    fixture.text_of("[data-slot=\"1\"] [data-right-value]")
}

/// Contract (`QueuePanel.test.ts`): the current row keeps its ink in the
/// right column at full strength — the now-playing row's value paints in the
/// canvas's ink, not the repeated time token.
#[test]
fn the_current_row_keeps_its_ink_in_the_right_column() {
    let fixture = built_with_frame(
        queue_panel_app,
        queue(vec![in_state(item(1), GuiSemanticState::NowPlaying)]),
        summary(),
        None,
    );

    let value = fixture.style_of("[data-slot=\"1\"] [data-right-value]");
    assert!(value.contains("color: var(--canvas-ink)"), "{value}");
    assert!(fixture.text().contains("3:12"));
}

/// Contract (`QueuePanel.test.ts`): the rows paint the watched, active, and
/// partial-progress states — the watched row strikes its title through in
/// the canvas's strike colour, the playing row keeps its own fill, shows a
/// play glyph, and weights its title 600, and the track under the title
/// fills to max(percent, 3)%.
#[test]
fn the_rows_paint_the_watched_active_and_partial_progress_states() {
    let rows = vec![
        in_state(renamed(item(1), "Watched"), GuiSemanticState::Played),
        in_state(renamed(item(2), "Playing"), GuiSemanticState::NowPlaying),
        in_state(
            renamed(item(3), "Partial"),
            GuiSemanticState::Active { progress: Some(42) },
        ),
        in_state(
            renamed(item(4), "Started"),
            GuiSemanticState::Active { progress: Some(1) },
        ),
    ];
    let fixture = built_with_frame(queue_panel_app, queue(rows), summary(), None);

    let watched = fixture.attr_of("[data-slot=\"1\"] .truncate", local_name!("class"));
    assert!(watched.is_some_and(|class| class.contains("line-through")));
    let watched_style = fixture.style_of("[data-slot=\"1\"] .truncate");
    assert!(
        watched_style.contains("text-decoration-color: var(--canvas-strike)"),
        "{watched_style}"
    );

    let playing_row = fixture.style_of("[data-slot=\"2\"]");
    assert!(!playing_row.contains("background"), "{playing_row}");
    assert!(fixture.presents("[data-slot=\"2\"] svg"));
    let playing_title = fixture.style_of("[data-slot=\"2\"] .truncate");
    assert!(
        playing_title.contains("font-weight: 600"),
        "{playing_title}"
    );

    let partial = fixture.style_of("[data-slot=\"3\"] [data-progress] span");
    assert!(partial.contains("width: 42%"), "{partial}");
    let started = fixture.style_of("[data-slot=\"4\"] [data-progress] span");
    assert!(started.contains("width: 3%"), "{started}");
}

/// Contract (`QueuePanel.test.ts`): the row paints the canvas one-liner —
/// its padded ordinal, one title line, and the value; no second line, no
/// date gutter, and no play marker.
#[test]
fn the_row_paints_the_canvas_one_liner() {
    let row = with_details(renamed(item(1), "Nether Zone"), "Peep Show", "mon 08");
    let fixture = built_with_frame(queue_panel_app, queue(vec![row]), summary(), None);

    assert!(fixture.presents("button[data-slot=\"1\"]"));
    assert_eq!(fixture.text_of("[data-slot=\"1\"] [data-ordinal]"), "01");
    let text = fixture.text();
    assert!(text.contains("Nether Zone"), "{text}");
    assert!(!text.contains("Peep Show"), "{text}");
    assert!(!text.contains("mon 08"), "{text}");
    assert!(!fixture.presents("[data-slot=\"1\"] svg"));
}

/// Contract (`QueuePanel.test.ts`): the cursor and marked rows paint the
/// canvas picked fill, playing or not.
#[test]
fn the_cursor_and_marked_rows_paint_the_canvas_picked_fill() {
    let rows = vec![item(1), in_state(item(2), GuiSemanticState::NowPlaying)];
    let mut rows_frame = queue(rows);
    rows_frame.active = Some(2);
    rows_frame.cursor = Some(1);
    rows_frame.marks = vec![2];
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    assert!(
        fixture
            .style_of("[data-slot=\"1\"]")
            .contains("background: var(--canvas-selected)")
    );
    assert!(
        fixture
            .style_of("[data-slot=\"2\"]")
            .contains("background: var(--canvas-selected)")
    );
}

/// Contract (`QueuePanel.test.ts`, the Menus artboard B.3): the hovered row
/// alone paints the grip and the actions button in place of its ordinal and
/// value — the grip's six dots in the leading slot, the 30 px actions box in
/// the canvas's fill and radius at the trailing slot, the row filled with
/// the hover colour, and its title ellipsised in the hover text.
#[test]
fn the_hovered_row_alone_paints_the_grip_and_the_actions_button() {
    let rows = vec![item(1), item(2), item(3)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(2);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    assert_eq!(fixture.all("[data-grip]").len(), 1);
    assert_eq!(fixture.all("[data-actions]").len(), 1);
    assert_eq!(fixture.all("[data-row=\"2\"] [data-grip] circle").len(), 6);
    assert!(!fixture.presents("[data-row=\"2\"] [data-ordinal]"));
    assert!(!fixture.presents("[data-slot=\"2\"] [data-right-value]"));
    assert!(fixture.presents("[data-row=\"1\"] [data-ordinal]"));

    let actions = fixture.attr_of("[data-actions]", local_name!("aria-label"));
    assert_eq!(actions.as_deref(), Some("Row actions"));
    let actions_style = fixture.style_of("[data-actions]");
    assert!(actions_style.contains("width: 30px"), "{actions_style}");
    assert!(actions_style.contains("height: 30px"), "{actions_style}");
    assert!(
        actions_style.contains("border-radius: 4px"),
        "{actions_style}"
    );
    assert!(
        actions_style.contains("background: var(--canvas-action)"),
        "{actions_style}"
    );

    let row_style = fixture.style_of("[data-slot=\"2\"]");
    assert!(
        row_style.contains("background: var(--canvas-selected)"),
        "{row_style}"
    );
    assert!(row_style.contains("border-radius: 4px"), "{row_style}");
    let title = fixture.style_of("[data-slot=\"2\"] .truncate");
    assert!(title.contains("color: var(--canvas-heading)"), "{title}");
}

/// Contract (`QueuePanel.test.ts`, the Menus artboard B.2): the open row
/// menu paints the canvas chrome — a 230 px card in the menu fill, border,
/// radius, and shadow with 6 px padding and 2 px gap — 34 px entries at
/// 13 px, the muted mono hints with the paint-time `⏎` glyph for the core's
/// raw `Enter` chord, the cursor row in the selected fill, and the
/// destructive entry in the destructive tone above its divider.
#[test]
fn the_open_row_menu_paints_the_canvas_chrome_hints_and_destructive_entry() {
    let rows = vec![item(1)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(1);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), Some(row_menu()));

    let menu_style = fixture.style_of("[data-row-menu]");
    assert_eq!(
        fixture
            .attr_of("[data-row-menu]", local_name!("role"))
            .as_deref(),
        Some("menu")
    );
    assert_eq!(
        fixture
            .attr_of("[data-row-menu]", local_name!("aria-label"))
            .as_deref(),
        Some("Row actions")
    );
    assert!(menu_style.contains("width: 230px"), "{menu_style}");
    assert!(menu_style.contains("padding: 6px"), "{menu_style}");
    assert!(menu_style.contains("gap: 2px"), "{menu_style}");
    assert!(
        menu_style.contains("background: var(--canvas-menu)"),
        "{menu_style}"
    );
    assert!(
        menu_style.contains("border: 1px solid var(--canvas-menu-border)"),
        "{menu_style}"
    );
    assert!(menu_style.contains("border-radius: 8px"), "{menu_style}");
    assert!(
        menu_style.contains("box-shadow: var(--canvas-menu-shadow)"),
        "{menu_style}"
    );

    let entries = fixture.all("[data-menu-entry]");
    assert_eq!(entries.len(), 5);
    let first = fixture.text_of("[data-menu-entry=\"0\"]");
    assert!(first.contains("Play now"), "{first}");
    assert!(first.contains("⏎"), "{first}");
    assert!(!fixture.text().contains("Enter"));

    let first_style = fixture.style_of("[data-menu-entry=\"0\"]");
    assert!(first_style.contains("height: 34px"), "{first_style}");
    assert!(first_style.contains("padding: 0 10px"), "{first_style}");
    assert!(first_style.contains("font-size: 13px"), "{first_style}");
    let hint = fixture.style_of("[data-menu-entry=\"0\"] span");
    assert!(hint.contains("font-size: 11px"), "{hint}");
    assert!(hint.contains("color: var(--canvas-muted)"), "{hint}");

    let cursor_style = fixture.style_of("[data-menu-entry=\"1\"]");
    assert!(
        cursor_style.contains("background: var(--canvas-selected)"),
        "{cursor_style}"
    );
    let remove_style = fixture.style_of("[data-menu-entry=\"4\"]");
    assert!(
        remove_style.contains("color: var(--canvas-destructive)"),
        "{remove_style}"
    );
    let divider = fixture.style_of("[data-menu-divider]");
    assert!(
        divider.contains("background: var(--canvas-menu-border)"),
        "{divider}"
    );
}

/// Contract (`QueuePanel.test.ts`, the Menus artboard B.5): a drag paints
/// the ghost without an ordinal — its title in the heading tone and its
/// percent in the partial tone at the ghost's offset and overhang, in the
/// canvas's fill, radius, tilt, and lift — and the inset insertion line at
/// the row the anchor names, while the dragged row stays lit.
#[test]
fn a_drag_paints_the_ghost_its_percent_and_the_inset_insertion_line() {
    let ghost = in_state(
        renamed(item(1), "Dragged track"),
        GuiSemanticState::Active { progress: Some(5) },
    );
    let mut rows_frame = queue(vec![item(1), item(2), item(3)]);
    rows_frame.drag = Some(drag(ghost, GuiQueueInsertion::After { slot: 2 }));
    let fixture = built_with_frame(queue_panel_app, rows_frame, ends_at_summary(), None);

    let ghost_style = fixture.style_of("[data-ghost]");
    assert!(fixture.text().contains("Dragged track"));
    assert!(!fixture.presents("[data-ghost] [data-ordinal]"));
    assert_eq!(fixture.text_of("[data-ghost-percent]"), "5%");
    assert!(ghost_style.contains("left: 28px"), "{ghost_style}");
    assert!(ghost_style.contains("right: -6px"), "{ghost_style}");
    let card = fixture.style_of("[data-ghost] div");
    assert!(card.contains("background: var(--canvas-ghost)"), "{card}");
    assert!(card.contains("border-radius: 4px"), "{card}");
    assert!(card.contains("transform: rotate(-1deg)"), "{card}");
    assert!(
        card.contains("box-shadow: var(--canvas-ghost-shadow)"),
        "{card}"
    );
    let ghost_title = fixture.style_of("[data-ghost] .truncate");
    assert!(
        ghost_title.contains("color: var(--canvas-heading)"),
        "{ghost_title}"
    );
    assert!(
        !fixture
            .text_of("[data-ghost]")
            .contains(&expected_row_end())
    );

    let line = fixture.style_of("[data-insertion]");
    // The `after` anchor: the line pins to the row's bottom edge, not its top.
    assert!(line.contains("bottom: 0;"), "{line}");
    assert!(!line.contains("top: 0;"), "{line}");
    assert!(line.contains("margin: 0 8px"), "{line}");
    assert!(line.contains("border-radius: 1px"), "{line}");
    assert!(
        line.contains("background: var(--canvas-insertion)"),
        "{line}"
    );
    assert!(fixture.presents("[data-row=\"2\"] [data-insertion]"));

    let dragged = fixture.style_of("[data-slot=\"1\"]");
    assert!(!dragged.contains("opacity"), "{dragged}");
}

/// Contract (`QueuePanel.test.ts`): the drag ghost paints no percent when
/// the dragged row has no progress.
#[test]
fn the_drag_ghost_paints_no_percent_when_the_dragged_row_has_no_progress() {
    let ghost = renamed(item(1), "Dragged track");
    let mut rows_frame = queue(vec![item(1), item(2)]);
    rows_frame.drag = Some(drag(ghost, GuiQueueInsertion::Before { slot: 2 }));
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    assert!(fixture.presents("[data-ghost]"));
    assert!(!fixture.presents("[data-ghost-percent]"));
}

/// Contract (`QueuePanel.test.ts`): the rows report their slot regions, the
/// hovered row its grip and actions regions after them, and the open menu
/// its entry regions last — the header's `rightColumn` and `filter` regions
/// join the front of the list with the header's port (task 5.6).
#[test]
fn the_regions_carry_the_slot_grip_actions_and_menu_entry_ids_menu_last() {
    let rows = vec![item(1), item(2)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(2);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), Some(row_menu()));

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::Queue(QueueTargetId::Slot(1)),
            RegionTarget::Queue(QueueTargetId::Slot(2)),
            RegionTarget::Queue(QueueTargetId::RowGrip(2)),
            RegionTarget::Queue(QueueTargetId::RowActions(2)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(0)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(1)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(2)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(3)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(4)),
        ]
    );
}

/// Contract (`QueuePanel.test.ts`): the open menu overlaps the bottom edge
/// of the row its anchorSlot names — the second painted row — not the
/// hovered row's and not the cursor row's.
#[test]
fn the_open_menu_overlaps_the_bottom_edge_of_the_row_its_anchor_slot_names() {
    let rows = vec![item(1), item(2), item(3)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(3);
    rows_frame.cursor = Some(1);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), Some(row_menu()));

    let menu_style = fixture.style_of("[data-row-menu]");
    assert!(
        menu_style.contains("top: calc(var(--canvas-queue-header-height) + 54px)"),
        "{menu_style}"
    );
}

/// Contract (`QueuePanel.test.ts`): the report carries the queue list's
/// capacity and the painted regions, with the panel container's size — the
/// shape the core's queue viewport and hit map read (D4, D9). The headless
/// document lays no pixels out, so the capacity is the zero-height list's.
#[test]
fn the_report_carries_the_queue_lists_capacity_shape() {
    // The zero viewport lays the list out at zero height: the capacity is
    // its zero-height value.
    let mut fixture = built_unsized(queue_panel_app);
    fixture.show(queue(vec![untimed(ending(item(7), None))]), summary(), None);

    let report = fixture.last_report().expect("the paint reports");
    // The capacity is the shared whole-rows rule over the list box's
    // measured height (the zero viewport lays it out small).
    let list_height = {
        let dom = fixture.doc.inner.borrow();
        let id = dom
            .query_selector("[data-queue-list]")
            .expect("the selector is valid")
            .expect("the list box paints");
        dom.get_client_bounding_rect(id)
            .map(|rect| crate::window::input::f32_from_f64(rect.height))
            .unwrap_or_default()
    };
    assert_eq!(
        report.lists,
        vec![ListRows {
            list: ListId::Queue,
            whole_rows: super::whole_rows(list_height),
        }]
    );
    assert_eq!(
        report
            .regions
            .iter()
            .map(|region| region.target)
            .collect::<Vec<_>>(),
        vec![RegionTarget::Queue(QueueTargetId::Slot(7))]
    );
    assert_eq!(report.container_size.width, 0.0);
    assert_eq!(report.container_size.height, 0.0);
}

/// Contract (`QueuePlaybackPanel.test.ts`, deferred to 5.5): an open chooser
/// reports its rows after the queue's regions — the hit map's last push
/// wins where a chooser row paints over a queue row beneath it. The
/// footer's regions and the header's `rightColumn` and `filter` regions
/// join the front with their ports (task 5.6).
#[test]
fn an_open_chooser_reports_its_rows_after_the_queue_regions() {
    let mut fixture = built_sized(panel_tree_app);
    let mut sections = fixture.sections.clone();
    let anchor = anchor();
    let mut playhead = fixture.playhead.clone();
    fixture.in_runtime(move || {
        sections.playback.set(playing());
        sections.queue.set(queue(vec![item(1)]));
        sections.summary.set(summary());
        sections.focus.set(GuiFocus {
            surface: None,
            panel: GuiPanel::Queue,
        });
        playhead.stamp(anchor);
    });
    let mut sections = fixture.sections.clone();
    fixture.in_runtime(move || sections.overlay.set(Some(chooser())));

    let targets = fixture.last_reported_targets();
    let first_choice = targets
        .iter()
        .position(|target| {
            matches!(
                target,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(_))
            )
        })
        .expect("the chooser reports its rows");
    let before = &targets[..first_choice];
    assert!(
        before.contains(&RegionTarget::Queue(QueueTargetId::Slot(1))),
        "{targets:?}"
    );
    assert_eq!(
        &targets[first_choice..],
        &[
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(1)),
        ]
    );
}

/// Contract (`QueuePlaybackPanel.test.ts`, deferred to 5.5): a paint reports
/// the player panel's transport, seek, header, seekbar, and target regions,
/// then the queue rows' slot regions — the two panels mount in that order
/// under the expanded artboard.
#[test]
fn a_paint_reports_the_player_panels_regions_then_the_queue_rows() {
    let mut fixture = built_sized(panel_tree_app);
    let mut sections = fixture.sections.clone();
    let anchor = anchor();
    let mut playhead = fixture.playhead.clone();
    fixture.in_runtime(move || {
        sections.playback.set(playing());
        sections.queue.set(queue(vec![item(1)]));
        sections.summary.set(summary());
        sections.focus.set(GuiFocus {
            surface: None,
            panel: GuiPanel::Queue,
        });
        playhead.stamp(anchor);
    });
    // The chooser stays closed: no chooser regions join the order.

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Previous),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekBack),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekForward),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
            RegionTarget::Queue(QueueTargetId::Slot(1)),
        ]
    );
}

/// Contract (the Svelte report's `wholeRows`): the whole 30 px rows a list
/// box fits — whole rows only, a partial row counts none.
#[test]
fn the_capacity_counts_whole_30px_rows_only() {
    assert_eq!(super::whole_rows(60.0), 2);
    assert_eq!(super::whole_rows(59.0), 1);
    assert_eq!(super::whole_rows(0.0), 0);
}
