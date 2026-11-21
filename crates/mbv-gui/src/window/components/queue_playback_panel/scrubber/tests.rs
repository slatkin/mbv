//! Headless tests of the scrubber (`gui-queue-panel-redesign` design D4): the
//! seek row's chrome (track, played fill, thumb), its hover time tip, and
//! its clock-free bar. The rows are painted inside the
//! control rows, as the panel paints them, so the elapsed and `−remaining`
//! clocks of the transport row are covered here too. The presses are not
//! ported: they are the root input layer's contract (`window/input.rs`),
//! which the scrubber forwards to untouched. The clocks are the injected
//! recorders (D10); no test waits.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use blitz_dom::Document as _;
use blitz_dom::local_name;
use dioxus_native::prelude::*;

use dioxus_native::{DioxusDocument, DocumentConfig};
use mbv::NowPlayingMediaKind;

use super::super::art_stage::{ControlRow, ControlRowHandles};
use crate::bridge::frame::playback::{
    GuiPlayerControls, GuiPlayhead, GuiQueuePlayback, GuiScrubberHover, GuiTransportAvailability,
};
use crate::window::clock::Clock;
use crate::window::playhead::{Playhead, Times};
use crate::window::sections::Sections;

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`).
const SECOND: i64 = 10_000_000;

/// The times the test drives: a frozen monotonic clock (the prediction
/// stays at its anchor) and the wall clock a test sets (D10).
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

/// The unix milliseconds whose local time-of-day is `secs_of_day`: the
/// finish clocks paint the local zone, so the test picks the wall clock
/// from the zone the process holds.
fn wall_ms_at_local(secs_of_day: i64) -> i64 {
    (secs_of_day - i64::from(crate::window::time::local_offset().whole_seconds())) * 1000
}

/// The playing transport band's facts (the test's fixture, trimmed to the
/// facts the scrubber reads).
fn playing() -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: String::new(),
        host_is_remote: false,
        shown: true,
        show_controls: true,
        now_playing_title: None,
        now_playing_media: NowPlayingMediaKind::default(),
        parent_line: None,
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

/// The hovered scrubber preview the `hover` section carries.
fn hovered(fraction: f64) -> GuiScrubberHover {
    GuiScrubberHover {
        fraction,
        time_label: "1:40".to_owned(),
    }
}

#[derive(Clone)]
struct FixtureProps {
    /// The wall clock the playhead's finish times read.
    wall_ms: i64,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test.
type FixtureHandles = (Sections, Playhead);

/// The root: provides the section stores and the playhead over the injected
/// clocks, and paints the control rows, as the panel paints them inside its
/// controls block at full scale.
fn scrubber_app(props: FixtureProps) -> Element {
    let FixtureProps { wall_ms, handle } = props;
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let clock: Arc<dyn Clock> = Arc::new(crate::window::clock::RecordingClock::new());
    let times: Arc<dyn Times> = Arc::new(FakeTimes {
        wall_ms: Mutex::new(wall_ms),
    });
    let playhead = use_hook(move || Playhead::new(Arc::clone(&clock), times));
    provide_context(playhead.clone());
    let rows = ControlRowHandles::default();
    let availability = playing().availability;
    handle.borrow_mut().replace((sections, playhead));
    rsx! {
        div {
            style: "--player-scale: 1;",
            ControlRow { paused: false, availability, rows }
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    playhead: Playhead,
}

impl Fixture {
    /// The whole text the document paints.
    fn text(&self) -> String {
        use blitz_dom::traversal::TreeTraverser;
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

    /// Every painted node's `style` attribute.
    fn styles(&self) -> Vec<String> {
        use blitz_dom::traversal::TreeTraverser;
        let dom = self.doc.inner();
        TreeTraverser::new(&dom)
            .filter_map(|id| {
                let node = dom.get_node(id)?;
                node.attr(local_name!("style")).map(str::to_owned)
            })
            .collect()
    }

    /// The `style` attribute of the node the selector names.
    fn style_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(local_name!("style"))
            .expect("the node carries a style")
            .to_owned()
    }

    /// The `class` attribute of the node the selector names.
    fn class_of(&self, selector: &str) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")?;
        dom.get_node(id)?
            .attr(local_name!("class"))
            .map(str::to_owned)
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

    /// Write the `playback` section, as the drain task would, and let the
    /// re-render land.
    fn set_playback(&mut self, playback: GuiQueuePlayback) {
        let mut sections = self.sections.clone();
        self.doc
            .vdom
            .in_runtime(move || sections.playback.set(playback));
        self.poll_until_idle();
    }

    /// Write the `hover` section, as the drain task would, and let the
    /// re-render land.
    fn set_hover(&mut self, hover: Option<GuiScrubberHover>) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || sections.hover.set(hover));
        self.poll_until_idle();
    }

    /// Stamp the playhead's anchor (design D5): the clocks start exactly at
    /// it.
    fn stamp(&mut self, anchor: GuiPlayhead) {
        let mut playhead = self.playhead.clone();
        self.doc.vdom.in_runtime(move || playhead.stamp(anchor));
        self.poll_until_idle();
    }
}

fn built() -> Fixture {
    // The wall clock at 20:57:40 local, so a 200 s item at its 60th second
    // finishes at exactly 21:00 (the test's TEST_WALL).
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            scrubber_app,
            FixtureProps {
                wall_ms: wall_ms_at_local(20 * 3600 + 57 * 60 + 40),
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let (sections, playhead) = handle
        .borrow()
        .clone()
        .expect("the root handed the fixtures over");
    let mut fixture = Fixture {
        doc,
        sections,
        playhead,
    };
    fixture.poll_until_idle();
    fixture
}

/// Contract (`gui-queue-panel-redesign` design D3): the seek row is the
/// `seekbar` region and carries `relative` — every painted part of the bar is
/// `absolute`, so the row is the only positioned ancestor between them and
/// the app root; without it the bar paints window-wide (the former client
/// markup's `class="relative"`, `main` audits 9–12).
#[test]
fn the_slider_box_is_positioned_so_the_bar_paints_inside_it() {
    let fixture = built();
    let class = fixture
        .class_of("[data-target=\"seekbar\"]")
        .expect("the seek row paints");
    assert!(
        class.split_whitespace().any(|name| name == "relative"),
        "{class}"
    );
}

/// Contract (`gui-queue-panel-redesign` design D3): the clocks paint on the
/// transport row — the elapsed time, and the remaining time behind the
/// canvas minus, with no finish clock (the panel's `ends` hint is gone,
/// 2026-11-16). The wall clock at 20:57:40 puts a 200 s item at its 60th
/// second's finish at exactly 21:00.
#[test]
fn the_transport_row_paints_the_elapsed_and_remaining_clocks() {
    let mut fixture = built();
    fixture.set_playback(playing());
    fixture.stamp(GuiPlayhead {
        active: true,
        active_slot: Some(1),
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
    });

    let text = fixture.text();
    assert!(text.contains("1:00"), "{text}");
    assert!(text.contains("\u{2212}2:20"), "{text}");
    assert!(!text.contains("ends"), "{text}");
}

/// Contract (`gui-queue-panel-redesign` design D4, task 4.2a): the bar's
/// hover tip reads the hovered time alone, with no trickplay frame and no
/// chapter label; and the hover chrome carries the 6 px track, the played
/// fill at its fraction, and the 16 px thumb (`main` audits 10–12).
#[test]
fn the_scrubber_paints_the_hover_time_tip_with_no_frame_and_no_chapter() {
    let mut fixture = built();
    fixture.set_playback(playing());
    fixture.stamp(GuiPlayhead {
        active: true,
        active_slot: Some(1),
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
    });
    fixture.set_hover(Some(hovered(0.75)));

    let text = fixture.text();
    assert!(text.contains("1:40"), "{text}");
    assert!(
        !fixture
            .styles()
            .iter()
            .any(|style| style.contains("background-image")),
        "no trickplay frame paints: {:?}",
        fixture.styles()
    );
    assert!(!text.contains("Ch."), "no chapter label paints: {text}");
    // The block's lengths carry the control rows' scale (design D3 of
    // `gui-art-player-overlay`), so they multiply by `--player-scale`.
    assert!(
        fixture
            .style_of("[data-track]")
            .contains("height: calc(6px * var(--player-scale))")
    );
    assert!(fixture.style_of("[data-played]").contains("width: 30%"));
    assert!(
        fixture
            .style_of("[data-thumb]")
            .contains("width: calc(16px * var(--player-scale))")
    );
}
