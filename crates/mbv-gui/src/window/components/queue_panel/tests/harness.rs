//! The shared fixture of the Queue panel's headless tests: the context
//! providers the root hands a mounted panel (the section stores, the
//! playhead, the layout epoch, and the art painter's), the mock core pair,
//! the frame builders, and the document wrapper's text, style, and report
//! readers. The layout rules are the handful of utilities the queue tree
//! sizes its cells with; the styles the assertions read are the component's
//! inline ones.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use blitz_dom::{Document as _, UserAgentStylesheet, local_name, traversal::TreeTraverser};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::super::QueuePanel;
use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::{
    GuiOverlayFrame, GuiQueueChooserEntry, GuiQueueChooserRename, GuiRowMenuEntry,
};
use crate::bridge::frame::playback::{
    GuiNowPlayingStatus, GuiPlayhead, GuiQueuePlayback, GuiTargetChoice, GuiTargetKey,
    GuiTransportAvailability,
};
use crate::bridge::frame::queue::{
    GuiMediaKind, GuiQueue, GuiQueueDrag, GuiQueueHeaderFacts, GuiQueueInsertion, GuiQueueRow,
    GuiQueueSummary, GuiSemanticState, GuiTrailing,
};
use crate::bridge::frame::theme::GuiTheme;
use crate::bridge::layout::{LayoutReport, RegionTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::components::queue_footer::QueueFooter;
use crate::window::components::queue_playback_panel::QueuePlaybackPanel;
use crate::window::layout::{LayoutEpoch, use_layout_reporter};
use crate::window::playhead::{Playhead, Times};
use crate::window::sections::{SectionRef, Sections};
use crate::window::theme::theme_style_rule;

pub(super) const SECOND: i64 = 10_000_000;

/// The fixed wall clock the tests stamp the playhead over: the port of the
/// TypeScript fixture's test clock. The expected finish strings derive from
/// it with the shared formatter, so the assertion holds in every zone.
pub(super) const WALL_MS: i64 = 1_700_000_000_000;

/// A queue of `rows`, no cursor, marks, active slot, start slot, hover, or
/// drag.
pub(super) fn queue(rows: Vec<GuiQueueRow>) -> GuiQueue {
    GuiQueue {
        rows,
        cursor: None,
        marks: Vec::new(),
        active: None,
        hover: None,
        start: None,
        playing: false,
        // A queue with no start mark reports its start slot as a queue with
        // one does: the window paints no outlined mark from an unreported
        // slot, so the fixture's rows carry no mark either way.
        start_slot_reported: true,
        drag: None,
        scroll: None,
    }
}

/// The summary the fixture paints: two remaining rows, the queue's finish
/// 43 minutes past the transport's finish, no filter, and the header's "One
/// queue and no peer" facts (spec `gui-queue-chooser`) — one entry, so no
/// chooser and a disabled close control.
pub(super) fn summary() -> GuiQueueSummary {
    GuiQueueSummary {
        remaining: 2,
        next_title: None,
        tail_secs: Some(43 * 60),
        filter: String::new(),
        filter_active: false,
        total_secs: 9 * 60 * 60,
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

/// The summary the header's "Several entries" contract paints (spec
/// `gui-queue-chooser`): `Default` and `Kids` are the local Owner's queues and
/// no peer connects, so the chooser shows and the close control acts.
pub(super) fn several_entries() -> GuiQueueSummary {
    GuiQueueSummary {
        header: GuiQueueHeaderFacts {
            entries: 2,
            shown_name: "Kids".to_owned(),
            close_disabled: false,
        },
        ..summary()
    }
}

/// The summary the Queue chooser's contracts paint (spec `gui-queue-chooser`):
/// `Default` and `Kids` are local queues and a peer connects, so the header
/// paints its chooser and its close control acts.
pub(super) fn chooser_summary() -> GuiQueueSummary {
    GuiQueueSummary {
        header: GuiQueueHeaderFacts {
            entries: 3,
            shown_name: "Kids".to_owned(),
            close_disabled: false,
        },
        ..summary()
    }
}

/// One entry of the open Queue chooser: its label, the local queue it shows
/// (`None` for the peer), and its shown and current marks.
pub(super) fn chooser_entry(
    label: &str,
    queue: Option<u64>,
    shown: bool,
    current: bool,
) -> GuiQueueChooserEntry {
    GuiQueueChooserEntry {
        label: label.to_owned(),
        queue,
        shown,
        current,
        playing: false,
    }
}

/// The open Queue chooser the fixture paints (task 8.3): `Default` is current
/// and `Kids` is shown, then the connected peer. `renaming` carries the draft
/// the chooser holds.
pub(super) fn queue_chooser(renaming: Option<GuiQueueChooserRename>) -> GuiOverlayFrame {
    GuiOverlayFrame::QueueChooser {
        cursor: 1,
        entries: vec![
            chooser_entry("Default", Some(1), false, true),
            chooser_entry("Kids", Some(2), true, false),
            chooser_entry("living-room", None, false, false),
        ],
        renaming,
    }
}

/// The open Queue chooser while the local Owner plays its current queue:
/// `queue_chooser` with `Default` marked as playing.
pub(super) fn queue_chooser_playing() -> GuiOverlayFrame {
    GuiOverlayFrame::QueueChooser {
        cursor: 1,
        entries: vec![
            GuiQueueChooserEntry {
                playing: true,
                ..chooser_entry("Default", Some(1), false, true)
            },
            chooser_entry("Kids", Some(2), true, false),
        ],
        renaming: None,
    }
}

/// One item row for `slot` (the TypeScript `item` helper): `Track <slot>`
/// with a 3:12 duration.
pub(super) fn item(slot: u64) -> GuiQueueRow {
    GuiQueueRow::Item {
        slot,
        ordinal: usize::try_from(slot).unwrap_or(0),
        primary: format!("Track {slot}"),
        secondary: None,
        trailing: None,
        duration: Some("3:12".to_owned()),
        kind: GuiMediaKind::Media,
        state: GuiSemanticState::Ordinary,
        striped: false,
    }
}

/// The item row with its title replaced.
pub(super) fn renamed(mut row: GuiQueueRow, primary: &str) -> GuiQueueRow {
    if let GuiQueueRow::Item { primary: field, .. } = &mut row {
        *field = primary.to_owned();
    }
    row
}

/// The item row with its semantic state replaced.
pub(super) fn in_state(mut row: GuiQueueRow, state: GuiSemanticState) -> GuiQueueRow {
    if let GuiQueueRow::Item { state: field, .. } = &mut row {
        *field = state;
    }
    row
}

/// The item row with its duration gone.
pub(super) fn untimed(mut row: GuiQueueRow) -> GuiQueueRow {
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
pub(super) fn with_details(mut row: GuiQueueRow, secondary: &str, trailing: &str) -> GuiQueueRow {
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
pub(super) fn entry(label: &str, key_hint: Option<&str>, destructive: bool) -> GuiRowMenuEntry {
    GuiRowMenuEntry {
        label: label.to_owned(),
        key_hint: key_hint.map(str::to_owned),
        destructive,
        // The queue's entries carry none, so the paint order stands.
        index: None,
    }
}

/// The open row menu the fixture paints: five entries, the cursor on the
/// second, the destructive `Remove from queue` last.
pub(super) fn row_menu() -> GuiOverlayFrame {
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
pub(super) fn drag(ghost: GuiQueueRow, insertion: GuiQueueInsertion) -> GuiQueueDrag {
    GuiQueueDrag {
        slot: 1,
        ghost,
        drop_index: 2,
        insertion,
    }
}

/// The times the tests drive: a frozen monotonic clock (the prediction stays
/// at its anchor) and the fixed wall clock (D10).
pub(super) struct FakeTimes {
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
pub(super) fn anchor() -> GuiPlayhead {
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
pub(super) type FixtureHandles = (Sections, Playhead, LayoutEpoch);

#[derive(Clone)]
pub(super) struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The fixture's shared context set, as the expanded artboard's root
/// provides it.
pub(super) fn provide_fixture_contexts(
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
pub(super) fn queue_panel_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    provide_fixture_contexts(sender, &handle);
    rsx! {
        QueuePanel {}
    }
}

/// The root of the theme-token contracts: the window's `:root` theme rule, as
/// the `ThemeStyle` element writes it, beside the Queue panel.
pub(super) fn themed_queue_panel_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    provide_fixture_contexts(sender, &handle);
    let rule = theme_style_rule(Some(&GuiTheme::current()));
    rsx! {
        style { "{rule}" }
        QueuePanel {}
    }
}

/// The playing transport band's facts the chooser test paints (the
/// TypeScript `playing()` fixture, trimmed to what the top block reads).
pub(super) fn playing() -> GuiQueuePlayback {
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
pub(super) fn chooser() -> GuiOverlayFrame {
    GuiOverlayFrame::TargetChooser {
        cursor: 0,
        choices: vec![
            GuiTargetChoice {
                key: GuiTargetKey::Emby("session-1".to_owned()),
                label: "Living room".to_owned(),
                is_active: true,
                is_direct_peer: false,
            },
            GuiTargetChoice {
                key: GuiTargetKey::Cast("receiver-1".to_owned()),
                label: "Kitchen speaker".to_owned(),
                is_active: false,
                is_direct_peer: false,
            },
        ],
    }
}

/// The root of the chooser's region-order contracts: the player panel's top
/// block and the Queue panel under one reporter, as the expanded artboard
/// composes them (`QueueFooter` joins with its port, task 5.6).
pub(super) fn panel_tree_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    provide_fixture_contexts(sender, &handle);
    rsx! {
        QueuePlaybackPanel {}
        QueuePanel {}
        QueueFooter {
            library_visible: false,
        }
    }
}

pub(super) struct Fixture {
    pub(super) doc: DioxusDocument,
    pub(super) receiver: Receiver<CoreInput>,
    pub(super) sections: Sections,
    pub(super) playhead: Playhead,
    pub(super) epoch: LayoutEpoch,
}

impl Fixture {
    /// The whole text the document paints.
    pub(super) fn text(&self) -> String {
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

    /// Whether the selector names a node in the painted tree.
    pub(super) fn presents(&self, selector: &str) -> bool {
        let dom = self.doc.inner();
        matches!(dom.query_selector(selector), Ok(Some(_)))
    }

    /// The `style` attribute of the first node the selector names.
    pub(super) fn style_of(&self, selector: &str) -> String {
        self.attr_of(selector, local_name!("style"))
            .unwrap_or_else(|| panic!("{selector} paints"))
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
        let node = dom.get_node(id)?;
        node.attr(attribute).map(str::to_owned)
    }

    /// The node ids the selector names, in tree order.
    pub(super) fn all(&self, selector: &str) -> Vec<blitz_dom::NodeId> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .collect()
    }

    /// The laid-out client rect of the first node the selector names.
    pub(super) fn rect_of(&self, selector: &str) -> blitz_dom::BoundingRect {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_client_bounding_rect(id)
            .expect("the node is laid out")
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    pub(super) fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Run `f` inside the document's runtime, as the drain task does: a
    /// signal write outside it reaches no subscriber.
    pub(super) fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

    /// The region targets of the last merged layout report.
    pub(super) fn last_reported_targets(&self) -> Vec<RegionTarget> {
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
    pub(super) fn last_report(&self) -> Option<LayoutReport> {
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
    pub(super) fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Paint the frame's queue and summary (and `overlay` when one is up),
    /// the frame the TypeScript fixture sets, and let the re-render land.
    pub(super) fn show(
        &mut self,
        rows: GuiQueue,
        summary: GuiQueueSummary,
        overlay: Option<GuiOverlayFrame>,
    ) {
        let mut sections = self.sections.clone();
        let anchor = anchor();
        let mut playhead = self.playhead.clone();
        self.doc.vdom.in_runtime(move || {
            sections.queue.set(SectionRef::new(rows));
            sections.summary.set(summary);
            sections.overlay.set(overlay);
            playhead.stamp(anchor);
        });
        self.poll_until_idle();
        self.lay_out();
    }
}

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the queue tree
/// sizes its cells with, and the positioning pair the hovered row's chrome is
/// placed by, so a title that fits its box paints whole and the chrome's
/// laid-out box is the box it paints. The styles the row assertions read are
/// the component's inline styles, which the document applies regardless.
pub(super) const FIXTURE_CSS: &str = concat!(
    "[data-slot] { width: 100%; }",
    "button { padding-block: 0; border: 0; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
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
pub(super) fn built_at(app: fn(FixtureProps) -> Element, width: u32, height: u32) -> Fixture {
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
pub(super) fn built_sized(app: fn(FixtureProps) -> Element) -> Fixture {
    built_at(app, 640, 900)
}

/// A fixture whose document lays out into a zero viewport: every rect is
/// zero, so the list's whole-row capacity is its zero-height value.
pub(super) fn built_unsized(app: fn(FixtureProps) -> Element) -> Fixture {
    built_at(app, 0, 0)
}

/// A fixture whose painted frame is `rows` under `summary` (and `overlay`
/// when one is up).
pub(super) fn built_with_frame(
    app: fn(FixtureProps) -> Element,
    rows: GuiQueue,
    summary: GuiQueueSummary,
    overlay: Option<GuiOverlayFrame>,
) -> Fixture {
    let mut fixture = built_sized(app);
    fixture.show(rows, summary, overlay);
    fixture
}
