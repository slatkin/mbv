//! Headless tests of the scrubber (D16): the contracts of
//! `QueuePlaybackPanel.test.ts` the scrubber owns — the elapsed and
//! `−remaining` clocks, the hover preview's label chip and its
//! two tile forms, and the bar's hover chrome (track, played fill, thumb,
//! position line). The presses are not ported: they are the root input
//! layer's contract (`window/input.rs`), which the scrubber forwards to
//! untouched. The clocks are the injected recorders (D10); no test waits.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use blitz_dom::Document as _;
use blitz_dom::local_name;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::Scrubber;
use crate::art::GuiArtRef;
use crate::bridge::frame::playback::{
    GuiChapter, GuiPlayhead, GuiQueuePlayback, GuiScrubberHover, GuiScrubberTile, GuiSegmentMark,
    GuiTransportAvailability,
};
use crate::window::clock::Clock;
use crate::window::playhead::{Playhead, Times};
use crate::window::sections::Sections;

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`).
const SECOND: i64 = 10_000_000;

/// The sheet image's reference the hovered tile names (the test's constant).
const SHEET_URL: &str = "mbv-art://localhost/trickplay/item%3A320%3A0";

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
/// facts the scrubber reads: the chapters and the two marks).
fn playing() -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: String::new(),
        host_is_remote: false,
        shown: true,
        show_controls: true,
        now_playing_title: None,
        parent_line: None,
        episode_badge: None,
        still_caption: None,
        chapters: vec![
            GuiChapter {
                name: "Chapter One".to_owned(),
                start_ticks: 0,
            },
            GuiChapter {
                name: "Chapter Two".to_owned(),
                start_ticks: 120 * SECOND,
            },
        ],
        intro_mark: Some(GuiSegmentMark {
            start_ticks: 0,
            end_ticks: 30 * SECOND,
        }),
        credits_mark: Some(GuiSegmentMark {
            start_ticks: 180 * SECOND,
            end_ticks: 200 * SECOND,
        }),
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

/// The hovered scrubber preview the `hover` section carries.
fn hovered(fraction: f64, tile: Option<GuiScrubberTile>) -> GuiScrubberHover {
    GuiScrubberHover {
        fraction,
        time_label: "1:40".to_owned(),
        chapter: None,
        chapter_number: None,
        tile,
    }
}

/// One hovered trickplay tile: column two, row one of a sheet whose image
/// reference is `sheet` (the test's fixture).
fn tile(sheet: GuiArtRef) -> GuiScrubberTile {
    GuiScrubberTile {
        sheet_key: "item:320:0".to_owned(),
        width: 320,
        height: 180,
        columns: 10,
        column: 2,
        row: 1,
        sheet,
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
/// clocks, and paints the scrubber, as the panel paints it inside its
/// controls block.
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
    let seekbar_mounted = use_signal(|| None::<Event<MountedData>>);
    handle.borrow_mut().replace((sections, playhead));
    rsx! {
        Scrubber { seekbar_mounted }
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

/// Contract: the slider box carries `relative` — every painted part of the
/// bar is `absolute`, so the box is the only positioned ancestor between
/// them and the app root; without it the bar paints window-wide (the
/// the former client markup's `class="relative"`, `main` audits 9–12).
#[test]
fn the_slider_box_is_positioned_so_the_bar_paints_inside_it() {
    let fixture = built();
    let class = fixture
        .class_of("[data-target=\"seekbar\"]")
        .expect("the slider box paints");
    assert!(
        class.split_whitespace().any(|name| name == "relative"),
        "{class}"
    );
}

/// Contract (`QueuePlaybackPanel.test.ts`, the clocks): the scrubber's two
/// labels paint the playhead's prediction — the elapsed time, and the
/// remaining time behind the canvas minus, with no finish clock (the
/// panel's `ends` hint is gone, 2026-11-16). The wall clock at 20:57:40
/// puts a 200 s item at its 60th second's finish at exactly 21:00.
#[test]
fn the_scrubber_paints_the_elapsed_and_remaining_clocks() {
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

/// Contract (`QueuePlaybackPanel.test.ts`): the scrubber paints the hover
/// preview from the frame — the label chip reads the canvas's `12:31 ·
/// Ch. 2` form; a position the active item offers no tile for paints the
/// label chip alone; and the bar's hover chrome carries the 6 px track, the
/// played fill at its fraction, the 16 px thumb, and the 1 px position line
/// at the hover's fraction (`main` audits 10–12).
#[test]
fn the_scrubber_paints_the_hover_preview_from_the_frame() {
    let mut fixture = built();
    fixture.set_playback(playing());
    fixture.stamp(GuiPlayhead {
        active: true,
        active_slot: Some(1),
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
    });
    fixture.set_hover(Some(GuiScrubberHover {
        fraction: 0.75,
        time_label: "2:30".to_owned(),
        chapter: Some("Chapter Two".to_owned()),
        chapter_number: Some(2),
        tile: None,
    }));

    let text = fixture.text();
    assert!(text.contains("2:30 \u{b7} Ch. 2"), "{text}");
    assert!(!text.contains("[trickplay]"), "{text}");
    assert!(
        !fixture
            .styles()
            .iter()
            .any(|style| style.contains("background-size")),
        "no tile box paints without a tile fact: {:?}",
        fixture.styles()
    );
    assert!(fixture.style_of("[data-track]").contains("height: 6px"));
    assert!(fixture.style_of("[data-played]").contains("width: 30%"));
    assert!(fixture.style_of("[data-thumb]").contains("width: 16px"));
    let line = fixture.style_of("[data-hover-line]");
    assert!(line.contains("width: 1px"), "{line}");
    assert!(line.contains("height: 22px"), "{line}");
    assert!(line.contains("left: 75%"), "{line}");
}

/// Contract (`QueuePlaybackPanel.test.ts`, task 6.7): the preview paints the
/// trickplay thumbnail the frame's tile names, cropped from the sheet's
/// sprite grid — the sheet loads by its URL, and the crop's place and size
/// name the tile's sheet cell (column two, row one of ten-column sheets).
#[test]
fn the_hover_preview_paints_the_trickplay_tile_the_frame_carries() {
    let mut fixture = built();
    fixture.set_playback(playing());
    fixture.stamp(GuiPlayhead {
        active: true,
        active_slot: Some(1),
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
    });
    fixture.set_hover(Some(hovered(
        0.5,
        Some(tile(GuiArtRef::Image {
            src: SHEET_URL.to_owned(),
        })),
    )));

    let sprite = fixture
        .styles()
        .iter()
        .find(|style| style.contains("background-size"))
        .expect("the sprite paints")
        .clone();
    assert!(
        sprite.contains(&format!("background-image: url('{SHEET_URL}')")),
        "{sprite}"
    );
    assert!(sprite.contains("background-size: 1280px auto"), "{sprite}");
    assert!(
        sprite.contains("background-position: -256px -72px"),
        "{sprite}"
    );
    assert!(sprite.contains("height: 72px"), "{sprite}");
}

/// Regression (8dd4badf0's sibling): a 2-hour book's chapter tick and
/// credits mark paint at their real bar fractions. The u32-bound
/// `i64_to_f64_saturating` conversion clamped both the chapter's start and
/// the runtime to ~7.15 minutes, so every boundary past it painted at 100 %.
#[test]
fn a_two_hour_book_paints_its_chapter_and_credits_at_their_real_fractions() {
    let mut fixture = built();
    let mut long_book = playing();
    long_book.chapters = vec![
        GuiChapter {
            name: "Opening".to_owned(),
            start_ticks: 0,
        },
        GuiChapter {
            name: "Half".to_owned(),
            start_ticks: 90 * 60 * SECOND,
        },
    ];
    long_book.credits_mark = Some(GuiSegmentMark {
        start_ticks: 60 * 60 * SECOND,
        end_ticks: 110 * 60 * SECOND,
    });
    fixture.set_playback(long_book);
    fixture.stamp(GuiPlayhead {
        active: true,
        active_slot: Some(1),
        position_ticks: 60 * SECOND,
        runtime_ticks: 2 * 3600 * SECOND,
        paused: false,
    });

    let styles = fixture.styles();
    let chapter = styles
        .iter()
        .find(|style| style.contains("height: 14px"))
        .expect("the chapter tick paints");
    assert!(chapter.contains("left: 75%"), "{chapter}");
    let credits = styles
        .iter()
        .find(|style| style.contains("width: 4px"))
        .expect("the credits mark paints");
    assert!(credits.contains("left: 50%"), "{credits}");
}

/// Contract (`QueuePlaybackPanel.test.ts`): a sheet still loading paints the
/// canvas's in-flight box, not a sprite — the placeholder paints only while
/// the frame carries the tile fact.
#[test]
fn the_hover_preview_paints_the_in_flight_box_while_its_sheet_loads() {
    let mut fixture = built();
    fixture.set_playback(playing());
    fixture.stamp(GuiPlayhead {
        active: true,
        active_slot: Some(1),
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
    });
    fixture.set_hover(Some(hovered(0.5, Some(tile(GuiArtRef::Loading)))));

    assert!(fixture.text().contains("[trickplay]"));
    assert!(
        !fixture
            .styles()
            .iter()
            .any(|style| style.contains("background-size")),
        "no sprite paints while the sheet loads: {:?}",
        fixture.styles()
    );
}
