//! Headless tests of the track chips row (D16): the contracts of
//! `TrackChips.test.ts` the window's port keeps — the video, audio, and
//! subtitle chips paint from the frame's `trackChips` datum, the chips
//! report their `audioChip` and `subtitleChip` regions (the video chip is
//! label-only and reports none), the open track popover paints the canvas
//! dialog below its chip and marks the chosen row, and the popover's rows
//! report their `trackPopoverChoice` indices last. The presses are not
//! ported: they are the root input layer's contract (`window/input.rs`),
//! which the chips forward to untouched. The core channel is a mock pair;
//! no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, local_name};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::TrackChips;
use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::{GuiOverlayFrame, GuiTrackChipKind, GuiTrackPopoverChoice};
use crate::bridge::frame::playback::{
    GuiPlayerControls, GuiQueuePlayback, GuiTrackChip, GuiTrackChips, GuiTrackChoice,
    GuiTransportAvailability,
};
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::components::art_image::ArtEvents;
use crate::window::components::queue_playback_panel::QueuePlaybackPanel;
use crate::window::layout::{LayoutEpoch, use_layout_reporter};
use crate::window::playhead::{Playhead, Times};
use crate::window::sections::Sections;

/// The chips the frame carries over a playing video item: a video label,
/// two audio tracks with the first chosen, and one forced subtitle with
/// none chosen (the popover's Off row is then chosen).
fn chips() -> GuiTrackChips {
    GuiTrackChips {
        video: Some("1080p".to_owned()),
        audio: GuiTrackChip {
            choices: vec![
                GuiTrackChoice {
                    id: 1,
                    label: "English".to_owned(),
                    forced: false,
                },
                GuiTrackChoice {
                    id: 2,
                    label: "Japanese".to_owned(),
                    forced: false,
                },
            ],
            chosen: Some(1),
        },
        subtitles: GuiTrackChip {
            choices: vec![GuiTrackChoice {
                id: 1,
                label: "English".to_owned(),
                forced: true,
            }],
            chosen: None,
        },
        audio_lang_code: "EN".to_owned(),
    }
}

/// The open subtitle popover the frame carries: Off first, then the tracks,
/// the chosen one marked.
fn subtitle_popover() -> GuiOverlayFrame {
    GuiOverlayFrame::TrackPopover {
        chip: GuiTrackChipKind::Subtitles,
        cursor: 0,
        choices: vec![
            GuiTrackPopoverChoice {
                id: 0,
                label: "Off".to_owned(),
                forced: false,
                chosen: true,
            },
            GuiTrackPopoverChoice {
                id: 1,
                label: "English".to_owned(),
                forced: true,
                chosen: false,
            },
        ],
    }
}

/// The open audio popover: the two audio tracks, the first chosen.
fn audio_popover() -> GuiOverlayFrame {
    GuiOverlayFrame::TrackPopover {
        chip: GuiTrackChipKind::Audio,
        cursor: 0,
        choices: vec![
            GuiTrackPopoverChoice {
                id: 1,
                label: "English".to_owned(),
                forced: false,
                chosen: true,
            },
            GuiTrackPopoverChoice {
                id: 2,
                label: "Japanese".to_owned(),
                forced: false,
                chosen: false,
            },
        ],
    }
}

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    chips: Option<GuiTrackChips>,
    overlay: Option<GuiOverlayFrame>,
    /// The slot the root hands the overlay signal back out through, so a
    /// test drives the frame's overlay the way the core's overlay section
    /// would.
    handle: Rc<RefCell<Option<Signal<Option<GuiOverlayFrame>>>>>,
}

/// The root: provides the reporter's contexts and paints the chips row, as
/// the player panel paints it inside its controls block. The overlay rides
/// a signal, so a test closes the popover by re-rendering the row with new
/// props.
fn chips_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        chips,
        overlay,
        handle,
    } = props;
    provide_context(CoreLink::new(sender));
    let reporter = use_layout_reporter();
    provide_context(reporter);
    provide_context(use_hook(LayoutEpoch::new));
    let overlay = use_signal(move || overlay);
    handle.borrow_mut().replace(overlay);
    rsx! {
        TrackChips { chips, overlay: overlay.cloned() }
    }
}

struct Fixture {
    doc: DioxusDocument,
    receiver: Receiver<CoreInput>,
    /// The overlay signal the root renders the row's props from, `None` for a
    /// fixture whose overlay rides the section store (the stage's).
    overlay: Option<Signal<Option<GuiOverlayFrame>>>,
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

    /// The text content of the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .map(blitz_dom::Node::text_content)
            .unwrap_or_default()
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
    /// landed.
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Close the open popover: write `None` into the overlay signal, as
    /// the drain task would carry the next overlay section, and let the
    /// re-render land.
    fn close_popover(&mut self) {
        let mut overlay = self
            .overlay
            .expect("the fixture renders its overlay from a signal");
        self.doc.vdom.in_runtime(move || overlay.set(None));
        self.poll_until_idle();
    }

    /// The region targets of the last merged layout report.
    fn last_reported_targets(&self) -> Vec<RegionTarget> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Layout(report) => Some(
                    report
                        .regions
                        .into_iter()
                        .map(|region| region.target)
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .last()
            .unwrap_or_default()
    }
}

fn built(chips: Option<GuiTrackChips>, overlay: Option<GuiOverlayFrame>) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            chips_app,
            FixtureProps {
                sender,
                chips,
                overlay,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let overlay = handle
        .borrow_mut()
        .take()
        .expect("the root handed the overlay signal over");
    let mut fixture = Fixture {
        doc,
        receiver,
        overlay: Some(overlay),
    };
    fixture.poll_until_idle();
    fixture
}

/// Contract (`TrackChips.test.ts`): a frame renders the video label and the
/// audio and subtitle chips — the audio chip's chosen choice's label, and
/// `Off` while the subtitle chip has no chosen choice.
#[test]
fn a_frame_renders_the_video_label_and_the_audio_and_subtitle_chips() {
    let fixture = built(Some(chips()), None);
    let text = fixture.text();
    assert!(text.contains("1080p"), "{text}");
    assert!(text.contains("Off"), "{text}");
}

/// The audio pill paints the chosen track's language code, never its full
/// label (`English`); the label stays in the popover's rows. A track that
/// names no language falls back to the choice's label.
#[test]
fn the_audio_pill_paints_the_language_code_not_the_label() {
    let fixture = built(Some(chips()), None);
    assert_eq!(fixture.text_of("[data-chip=\"audio\"]"), "EN");

    let mut no_language = chips();
    no_language.audio_lang_code = String::new();
    let fixture = built(Some(no_language), None);
    assert_eq!(fixture.text_of("[data-chip=\"audio\"]"), "English");
}

/// Contract (`TrackChips.test.ts`): an audio-only frame paints no video
/// chip.
#[test]
fn an_audio_only_frame_paints_no_video_chip() {
    let mut audio_only = chips();
    audio_only.video = None;
    let fixture = built(Some(audio_only), None);
    assert!(!fixture.text().contains("1080p"));
}

/// Contract (`TrackChips.test.ts`): no chips paint while the local player
/// is inactive — the row paints nothing.
#[test]
fn no_chips_paint_while_the_local_player_is_inactive() {
    let fixture = built(None, None);
    assert!(!fixture.presents("[data-chip]"));
}

/// Contract (`TrackChips.test.ts`): the chips report their region ids —
/// `audioChip` then `subtitleChip` — and the video chip, label-only by the
/// D4a ruling, reports none.
#[test]
fn the_chips_report_their_region_ids_the_video_chip_none() {
    let fixture = built(Some(chips()), None);
    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::AudioChip),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SubtitleChip),
        ]
    );
}

/// The popover rows' paint: 36 px rows, the chosen row filled and checked
/// in the canvas's check, the unchosen row in the canvas's row text.
fn assert_row_paints(fixture: &Fixture) {
    let rows = fixture.all("[data-popover-row]");
    assert_eq!(rows.len(), 2, "the frame's two choices paint");
    let chosen_style = fixture.style_of("[data-popover-row=\"0\"]");
    assert!(chosen_style.contains("height: 36px"), "{chosen_style}");
    assert!(chosen_style.contains("padding: 0 10px"), "{chosen_style}");
    assert!(chosen_style.contains("font-size: 13px"), "{chosen_style}");
    assert!(
        chosen_style.contains("border-radius: 4px"),
        "{chosen_style}"
    );
    assert!(
        chosen_style.contains("background: var(--canvas-selected)"),
        "{chosen_style}"
    );
    assert!(
        chosen_style.contains("color: var(--canvas-heading)"),
        "{chosen_style}"
    );
    let unchosen_style = fixture.style_of("[data-popover-row=\"1\"]");
    assert!(
        unchosen_style.contains("background: none"),
        "{unchosen_style}"
    );
    assert!(
        unchosen_style.contains("color: var(--canvas-text)"),
        "{unchosen_style}"
    );

    // The chosen row paints the canvas's check at its trailing edge; the
    // unchosen row paints none.
    let checks = fixture.all("[data-popover-row=\"0\"] svg.lucide-check");
    assert_eq!(checks.len(), 1, "the chosen row paints the check");
    let check = fixture
        .attr_of(
            "[data-popover-row=\"0\"] svg.lucide-check",
            local_name!("width"),
        )
        .expect("the check paints");
    assert_eq!(check, "14");
    assert!(!fixture.presents("[data-popover-row=\"1\"] svg.lucide-check"));
}

/// The dialog chrome (menus G4) and its placement below the chip row,
/// right-aligned (design DR8), with the canvas's dialog semantics naming
/// the chip's track kind.
fn assert_dialog_chrome(fixture: &Fixture, label: &str) {
    let dialog_class = fixture
        .attr_of("[role=\"dialog\"]", local_name!("class"))
        .expect("the dialog paints");
    assert!(dialog_class.contains("top-full"), "{dialog_class}");
    assert!(dialog_class.contains("right-0"), "{dialog_class}");
    assert!(!dialog_class.contains("bottom-full"), "{dialog_class}");
    let dialog_label = fixture
        .attr_of("[role=\"dialog\"]", local_name!("aria-label"))
        .expect("the dialog paints");
    assert_eq!(dialog_label, label);
    let dialog_style = fixture.style_of("[role=\"dialog\"]");
    assert!(dialog_style.contains("width: 260px"), "{dialog_style}");
    assert!(dialog_style.contains("padding: 6px"), "{dialog_style}");
    assert!(dialog_style.contains("gap: 2px"), "{dialog_style}");
    assert!(
        dialog_style.contains("background: var(--canvas-popover)"),
        "{dialog_style}"
    );
    assert!(
        dialog_style.contains("border: 1px solid var(--canvas-popover-border)"),
        "{dialog_style}"
    );
    assert!(
        dialog_style.contains("border-radius: 8px"),
        "{dialog_style}"
    );
    assert!(
        dialog_style.contains("box-shadow: var(--canvas-popover-shadow)"),
        "{dialog_style}"
    );
}

/// The open chip paints the expanded state (menus G3) and nothing else.
fn assert_chip_states(fixture: &Fixture) {
    let subtitle_chip_style = fixture.style_of("[data-chip=\"subtitles\"]");
    assert!(
        subtitle_chip_style.contains("background: var(--canvas-action)"),
        "{subtitle_chip_style}"
    );
    assert!(
        subtitle_chip_style.contains("border-color: var(--canvas-open-border)"),
        "{subtitle_chip_style}"
    );
    assert_eq!(
        fixture
            .attr_of("[data-chip=\"subtitles\"]", local_name!("aria-expanded"))
            .as_deref(),
        Some("true")
    );
    let audio_chip_style = fixture.style_of("[data-chip=\"audio\"]");
    assert!(
        audio_chip_style.contains("background: var(--mbv-secondary)"),
        "{audio_chip_style}"
    );
    assert_eq!(
        fixture
            .attr_of("[data-chip=\"audio\"]", local_name!("aria-expanded"))
            .as_deref(),
        Some("false")
    );
}

/// Contract (`TrackChips.test.ts`, the Menus artboard B.4 and design DR8):
/// the subtitle popover paints the 260 px canvas dialog below the chip row,
/// right-aligned, 36 px rows with the chosen row filled and checked and the
/// unchosen row in the canvas's row text, and the open chip paints the
/// expanded state while the closed one does not.
#[test]
fn the_subtitle_popover_paints_the_canvas_dialog_below_its_chip_and_marks_the_chosen_row() {
    let fixture = built(Some(chips()), Some(subtitle_popover()));

    assert_row_paints(&fixture);
    assert_dialog_chrome(&fixture, "Subtitles");
    assert_chip_states(&fixture);
}

/// Contract (`TrackChips.test.ts`): the audio popover is the same dialog
/// naming its own chip's kind.
#[test]
fn the_audio_popover_names_its_own_chip_kind() {
    let fixture = built(Some(chips()), Some(audio_popover()));
    let dialog_label = fixture
        .attr_of("[role=\"dialog\"]", local_name!("aria-label"))
        .expect("the dialog paints");
    assert_eq!(dialog_label, "Audio");
}

/// Contract (`TrackChips.test.ts`): an open popover reports one
/// `trackPopoverChoice` region per row, after the chips' own regions in the
/// merged report (the hit map's last push wins where a row paints over the
/// transport band beneath it).
#[test]
fn an_open_popover_reports_one_track_popover_choice_region_per_row_last() {
    let fixture = built(Some(chips()), Some(subtitle_popover()));
    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::AudioChip),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SubtitleChip),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TrackPopoverChoice(0)),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TrackPopoverChoice(1)),
        ]
    );
}

/// Contract: closing the popover retracts the popover key's row regions
/// from the merged report — a stale popover report would keep merging last
/// and swallow presses on the surfaces beneath its rows (the hit map's
/// last push wins).
#[test]
fn closing_the_popover_retracts_its_row_regions() {
    let mut fixture = built(Some(chips()), Some(subtitle_popover()));
    assert!(
        fixture
            .last_reported_targets()
            .contains(&RegionTarget::QueuePlayback(
                QueuePlaybackTargetId::TrackPopoverChoice(1)
            )),
        "the open popover reports its rows: {:?}",
        fixture.last_reported_targets()
    );

    fixture.close_popover();

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::AudioChip),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SubtitleChip),
        ]
    );
}

/// The stage fixture's props: the mock core's sender and the section store the
/// test writes the frame through.
#[derive(Clone)]
struct StageProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<Sections>>>,
}

/// The clocks the stage's scrubber reads: a frozen monotonic clock and a fixed
/// wall clock, as the panel fixtures use.
struct StageTimes;

impl Times for StageTimes {
    fn now_ms(&self) -> f64 {
        0.0
    }

    fn wall_now_ms(&self) -> i64 {
        1_700_000_000_000
    }
}

/// The playback frame the stage fixture paints: a loading artwork, so the
/// media box holds no image and the stage paints both overlay layers, with the
/// chips the frame carries and a title.
fn stage_frame() -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: "poo".to_owned(),
        host_is_remote: true,
        shown: true,
        show_controls: true,
        now_playing_title: Some("Track title".to_owned()),
        parent_line: None,
        episode_badge: None,
        availability: GuiTransportAvailability {
            stop: false,
            next: true,
            previous: false,
        },
        status_indicators: None,
        artwork: Some(GuiArtRef::Loading),
        track_chips: Some(chips()),
        target_choices: Vec::new(),
        controls: GuiPlayerControls::default(),
    }
}

/// The root of the stage's paint contract: the player panel as the expanded
/// artboard mounts it, with one queue row under it, so an open popover has the
/// panel's own content and the rows below to paint over.
fn stage_app(props: StageProps) -> Element {
    provide_context(CoreLink::new(props.sender));
    provide_context(use_layout_reporter());
    provide_context(use_hook(LayoutEpoch::new));
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    provide_context(use_hook(ArtEvents::new));
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let times: Arc<dyn Times> = Arc::new(StageTimes);
    let playhead = use_hook(move || Playhead::new(clock, times));
    provide_context(playhead);
    props.handle.borrow_mut().replace(sections);
    rsx! {
        div { class: "flex h-full min-h-0 flex-col",
            QueuePlaybackPanel {}
            div { class: "flex min-h-0 flex-1 flex-col",
                div { "data-row": "1", "Track 1" }
            }
        }
    }
}

/// A fixture over the player panel with one queue row under it, painting the
/// chips frame and the open subtitle popover.
fn stage_built() -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            stage_app,
            StageProps {
                sender,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let sections = handle
        .borrow_mut()
        .take()
        .expect("the root handed the section store over");
    let mut playback = sections.playback;
    let mut overlay = sections.overlay;
    doc.vdom.in_runtime(move || {
        playback.set(stage_frame());
        overlay.set(Some(subtitle_popover()));
    });
    let mut fixture = Fixture {
        doc,
        receiver,
        overlay: None,
    };
    fixture.poll_until_idle();
    fixture
}

/// Contract (`gui-art-player-overlay` D1, task 3.3): an open chip popover
/// paints in full where it reaches past the artwork's bottom edge. The media
/// box owns the clip, so the popover anchors in the stage's top layer outside
/// that box and carries the above-panel stacking class: it paints over the
/// panel's content and the queue row under it, never cut and never under them.
#[test]
fn an_open_popover_paints_outside_the_clipped_media_box_above_the_panel() {
    let fixture = stage_built();
    let media_box = "[data-target=\"visualSlot\"] > .overflow-hidden";
    assert!(fixture.presents(media_box), "the media box owns the clip");
    assert!(
        !fixture.presents(&format!("{media_box} [role=\"dialog\"]")),
        "the popover is not inside the clipped media box"
    );
    assert!(
        fixture.presents("[data-target=\"visualSlot\"] > .top-0 [role=\"dialog\"]"),
        "the popover anchors in the stage's top layer"
    );
    let dialog_class = fixture
        .attr_of("[role=\"dialog\"]", local_name!("class"))
        .expect("the popover paints");
    assert!(
        dialog_class.contains("z-30"),
        "the popover carries the above-panel stacking class: {dialog_class}"
    );
}
