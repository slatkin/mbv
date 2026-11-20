//! Headless paint of the window's screens: no drawing command may carry a
//! non-finite number.
//!
//! Vello drops a path whose flattened geometry contains NaN
//! (`vello_common` `flatten.rs`, "A path contains NaN, ignoring it."), so such
//! an op paints nothing: the element it belongs to is missing from the frame,
//! and every frame logs it again. The live run of this window logged ~200 of
//! them a second on every screen. This test therefore asserts the geometry of
//! a frame instead of leaving it to the renderer's log: it paints the real
//! screens through `blitz_paint::paint_scene` into a recording `PaintScene`
//! (`anyrender`'s `Scene`, which records every layer, path, glyph run, shadow,
//! and brush transform) over the window's own stylesheet, and reads every
//! recorded op back for a NaN or an infinity.
//!
//! The screens are the ones the root composes: the pre-attach screen, the
//! player panel in its four states, the Queue panel with its transport band and
//! footer, the Library panel, and the Settings screen. Each is a case of one
//! table; nothing opens a window, sleeps, or reaches the core.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use anyrender::recording::{RenderCommand, Scene};
use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;

use dioxus_native::{DioxusDocument, DocumentConfig};
use kurbo::{Affine, PathEl, Vec2};
use mbv::NowPlayingMediaKind;
use mbv_config::GuiPanelWidths;
use rstest::rstest;

use crate::art::GuiArtRef;
use crate::bridge::frame::GuiAttachState;
use crate::bridge::frame::playback::{
    GuiPlayerControls, GuiPlayhead, GuiQueuePlayback, GuiTransportAvailability,
};
use crate::bridge::frame::queue::{
    GuiMediaKind, GuiQueue, GuiQueueHeaderFacts, GuiQueueRow, GuiQueueSummary, GuiSemanticState,
};
use crate::bridge::layout::LayoutSize;
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::components::attach_screen::AttachScreen;
use crate::window::components::library_panel::LibraryPanel;
use crate::window::components::library_top_bar::LibraryTopBar;
use crate::window::components::panel::Panel;
use crate::window::components::queue_footer::QueueFooter;
use crate::window::components::queue_panel::QueuePanel;
use crate::window::components::queue_playback_panel::QueuePlaybackPanel;
use crate::window::components::settings_screen::SettingsScreen;
use crate::window::layout::{LayoutEpoch, use_layout_reporter};
use crate::window::library::state::LibraryState;
use crate::window::playhead::{Playhead, Times};
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::state::SettingsState;

/// The viewport the screens paint at: the wide arrangement's realistic size.
const WIDTH: u32 = 1200;
const HEIGHT: u32 = 800;

/// One Emby tick, in 100 ns units.
const SECOND: i64 = 10_000_000;

/// The panel container heights that pick each painted state: the expanded
/// artboard, the compact strip's ceiling, and the rail's height.
const ARTBOARD: LayoutSize = LayoutSize {
    width: 1200.0,
    height: 800.0,
};
const COMPACT: LayoutSize = LayoutSize {
    width: 1200.0,
    height: 400.0,
};
const RAIL: LayoutSize = LayoutSize {
    width: 72.0,
    height: 800.0,
};

/// One screen the window paints: the shape of the tree the root composes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// The pre-attach screen, on the build mismatch that paints the Restart
    /// and Quit buttons.
    AttachMismatch,
    /// The Queue panel between its transport band and its footer, with the
    /// queue's rows, a hovered and an active slot, and card artwork.
    Queue,
    /// The same panel while the artwork is the bundled placeholder.
    QueuePlaceholder,
    /// The player panel expanded, the queue under it.
    PanelArtboard,
    /// The player panel with its queue folded to one line.
    PanelFolded,
    /// The player panel as the compact strip.
    PanelCompactStrip,
    /// The player panel collapsed to the 72 px rail.
    PanelRail,
    /// The Library panel under the window's global header: the Hero column
    /// and the list column.
    Library,
    /// The Settings screen.
    Settings,
}

/// The wall clock the playhead paints against.
struct FakeTimes;

impl Times for FakeTimes {
    fn now_ms(&self) -> f64 {
        0.0
    }

    fn wall_now_ms(&self) -> i64 {
        1_700_000_000_000
    }
}

/// The queue the panel screens paint: three rows, the cursor on the first, and
/// the second playing and hovered.
fn queue() -> GuiQueue {
    GuiQueue {
        rows: vec![
            row(1, GuiSemanticState::Ordinary),
            row(2, GuiSemanticState::NowPlaying),
            row(3, GuiSemanticState::Played),
        ],
        cursor: Some(1),
        marks: Vec::new(),
        active: Some(2),
        hover: Some(2),
        drag: None,
        scroll: None,
    }
}

/// One item row of the queue.
fn row(slot: u64, state: GuiSemanticState) -> GuiQueueRow {
    GuiQueueRow::Item {
        slot,
        ordinal: usize::try_from(slot).unwrap_or(1),
        primary: format!("Track {slot}"),
        secondary: None,
        trailing: None,
        duration: Some("3:12".to_owned()),
        kind: GuiMediaKind::Media,
        state,
    }
}

/// The queue summary the header paints: the remaining count, the tail offset,
/// and the queue's total time, with one local queue and no peer.
fn summary() -> GuiQueueSummary {
    GuiQueueSummary {
        remaining: 2,
        next_title: Some("Track 2".to_owned()),
        tail_secs: Some(43 * 60),
        filter: String::new(),
        filter_active: false,
        total_secs: 9 * 60 * 60,
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

/// The transport band's now-playing facts, painting `art`.
fn playing(art: GuiArtRef) -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: "poo".to_owned(),
        host_is_remote: true,
        shown: true,
        show_controls: true,
        now_playing_title: Some("Track title".to_owned()),
        now_playing_media: NowPlayingMediaKind::default(),
        parent_line: Some("The Show".to_owned()),
        episode_badge: Some("S2 · E5".to_owned()),
        availability: GuiTransportAvailability {
            stop: true,
            next: true,
            previous: false,
        },
        status_indicators: None,
        artwork: Some(art),
        track_chips: None,
        target_choices: Vec::new(),
        controls: GuiPlayerControls::default(),
    }
}

/// The playhead anchor the band and the scrubber paint.
fn anchor() -> GuiPlayhead {
    GuiPlayhead {
        active: true,
        active_slot: Some(2),
        position_ticks: 60 * SECOND,
        runtime_ticks: 200 * SECOND,
        paused: false,
    }
}

/// The stores the fixture's screens hand their children, as the root's own
/// contexts provide them.
type Handles = (Sections, Playhead);

/// The fixture root's props: the screen to paint, and the cell the root hands
/// its stores back through.
#[derive(Clone)]
struct RootProps {
    screen: Screen,
    handle: std::rc::Rc<std::cell::RefCell<Option<Handles>>>,
}

/// The root over the window's real stylesheet: the same contexts `App`
/// provides its panels, then the screen the case names.
fn root(props: RootProps) -> Element {
    let RootProps { screen, handle } = props;
    let (sender, _core) = channel::<CoreInput>();
    let link = CoreLink::new(sender);
    provide_context(CoreLink::clone(&link));
    provide_context(use_layout_reporter());
    provide_context(use_hook(LayoutEpoch::new));
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let times: Arc<dyn Times> = Arc::new(FakeTimes);
    let playhead_clock = Arc::clone(&clock);
    let playhead = use_hook(move || Playhead::new(playhead_clock, times));
    provide_context(playhead.clone());
    provide_context(use_hook(ArtEvents::new));
    provide_context(use_hook(FiredTimers::new));
    provide_context(Arc::clone(&clock));
    provide_context(ScaleFactor(Signal::new(1.0)));
    let widths = use_hook(|| PanelWidthsStore::new(GuiPanelWidths::default(), Arc::new(|_| {})));
    provide_context(widths);
    let library = use_hook(|| {
        LibraryState::new(
            sections.clone(),
            CoreLink::clone(&link),
            playhead.position,
            Arc::clone(&clock),
        )
    });
    provide_context(library);
    provide_context(use_hook(|| SettingsState::new(link)));
    handle.borrow_mut().replace((sections, playhead));

    let style = super::super::ROOT_STYLE.as_str();
    rsx! {
        style { "{style}" }
        main { class: "h-dvh w-full overflow-hidden bg-background text-foreground",
            div { class: "relative flex h-full min-h-0 w-full min-w-0 overflow-hidden",
                {screen_element(screen)}
            }
        }
    }
}

/// The screen's own markup, as the root composes it.
fn screen_element(screen: Screen) -> Element {
    match screen {
        Screen::AttachMismatch => rsx! {
            AttachScreen {}
        },
        Screen::Queue => rsx! {
            QueueColumn {
                div { class: "flex h-full min-h-0 flex-col flex-1",
                    QueuePlaybackPanel {}
                    QueuePanel {}
                    QueueFooter { library_visible: false }
                }
            }
        },
        Screen::QueuePlaceholder => rsx! {
            QueueColumn {
                div { class: "flex h-full min-h-0 flex-col flex-1",
                    QueuePlaybackPanel {}
                    QueuePanel {}
                    QueueFooter { library_visible: false }
                }
            }
        },
        Screen::PanelArtboard => rsx! {
            Panel { container: ARTBOARD }
        },
        Screen::PanelFolded => rsx! {
            Panel { container: ARTBOARD }
        },
        Screen::PanelCompactStrip => rsx! {
            Panel { container: COMPACT }
        },
        Screen::PanelRail => rsx! {
            Panel { container: RAIL }
        },
        Screen::Library => rsx! {
            div { class: "flex h-full min-h-0 w-full min-w-0 flex-col",
                // The global header above the Library column, as the root
                // paints it (design D7 of
                // `move-playback-header-into-queue-footer`).
                LibraryTopBar {
                    tabs: Vec::new(),
                    current_tab: None,
                    home_active: false,
                }
                div { class: "flex min-h-0 flex-1 flex-col",
                    LibraryPanel {
                        arrangement: GuiArrangement::Wide,
                        list_width: 640.0,
                        library_column: 900.0,
                    }
                }
            }
        },
        Screen::Settings => rsx! {
            SettingsScreen { arrangement: GuiArrangement::Wide }
        },
    }
}

/// The queue column's backdrop, as the root paints it under the panel.
#[component]
fn QueueColumn(children: Element) -> Element {
    let canvas = "var(--canvas-queue-column)";
    rsx! {
        div {
            class: "flex h-full min-h-0 flex-col flex-1",
            style: "background: {canvas};",
            {children}
        }
    }
}

/// A fixture: the document and the stores the case's sections are written
/// through.
struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    playhead: Playhead,
    _core: Receiver<CoreInput>,
}

impl Fixture {
    /// Build the screen's document over a `WIDTH` × `HEIGHT` viewport and the
    /// window's real stylesheet.
    fn built(screen: Screen) -> Self {
        let (sender, core) = channel();
        let handle = std::rc::Rc::new(std::cell::RefCell::new(None));
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                root,
                RootProps {
                    screen,
                    handle: std::rc::Rc::clone(&handle),
                },
            ),
            DocumentConfig {
                viewport: Some(Viewport::new(WIDTH, HEIGHT, 1.0, ColorScheme::Light)),
                ua_stylesheets: vec![blitz_dom::UserAgentStylesheet::default_sheet()],
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        let _ = sender;
        let (sections, playhead) = handle
            .borrow_mut()
            .take()
            .expect("the root handed its stores over");
        let mut fixture = Self {
            doc,
            sections,
            playhead,
            _core: core,
        };
        fixture.write(screen);
        fixture
    }

    /// Write the screen's sections, as the core's batch would.
    fn write(&mut self, screen: Screen) {
        let mut sections = self.sections.clone();
        let mut playhead = self.playhead.clone();
        self.doc.vdom.in_runtime(move || {
            match screen {
                Screen::AttachMismatch => sections
                    .attach
                    .set(Some(GuiAttachState::Mismatch("mbv 0.26.10".to_owned()))),
                Screen::Queue | Screen::PanelArtboard | Screen::PanelFolded => {
                    sections.queue.set(SectionRef::new(queue()));
                    sections.summary.set(summary());
                    sections.playback.set(playing(GuiArtRef::Image {
                        src: "mbv-art://localhost/card/1".to_owned(),
                    }));
                }
                Screen::QueuePlaceholder | Screen::PanelCompactStrip | Screen::PanelRail => {
                    sections.queue.set(SectionRef::new(queue()));
                    sections.summary.set(summary());
                    sections.playback.set(playing(GuiArtRef::Placeholder {
                        src: "mbv-art://localhost/card/1".to_owned(),
                    }));
                }
                Screen::Library | Screen::Settings => {}
            }
            playhead.stamp(anchor());
        });
    }

    /// Poll the document until its effects have run, then resolve its style and
    /// layout, as the renderer's frame does before it paints.
    fn settle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                break;
            }
        }
        self.doc.inner.borrow_mut().resolve(0.0);
    }

    /// Paint one frame into the recorder.
    fn painted(&mut self) -> Scene {
        self.settle();
        let mut scene = Scene::new();
        let mut dom = self.doc.inner.borrow_mut();
        blitz_paint::paint_scene(&mut scene, &mut dom, 1.0, WIDTH, HEIGHT, 0, 0);
        scene
    }
}

/// Every op of `scene` whose geometry is not finite, as one line each.
fn non_finite_geometry(scene: &Scene) -> Vec<String> {
    let mut faults = Vec::new();
    for (index, command) in scene.commands.iter().enumerate() {
        let mut fault =
            |field: &str| faults.push(format!("op {index} ({}): {field}", kind(command)));
        match command {
            RenderCommand::PushLayer(cmd) => {
                finite_affine(&mut fault, "transform", cmd.transform);
                finite_path(&mut fault, "clip", &cmd.clip);
                finite(&mut fault, "alpha", f64::from(cmd.alpha));
            }
            RenderCommand::PushClipLayer(cmd) => {
                finite_affine(&mut fault, "transform", cmd.transform);
                finite_path(&mut fault, "clip", &cmd.clip);
            }
            RenderCommand::PopLayer => {}
            RenderCommand::Stroke(cmd) => {
                finite_affine(&mut fault, "transform", cmd.transform);
                if let Some(transform) = cmd.brush_transform {
                    finite_affine(&mut fault, "brush transform", transform);
                }
                finite_path(&mut fault, "shape", &cmd.shape);
                finite(&mut fault, "stroke width", cmd.style.width);
                for dash in &cmd.style.dash_pattern {
                    finite(&mut fault, "dash length", *dash);
                }
            }
            RenderCommand::Fill(cmd) => {
                finite_affine(&mut fault, "transform", cmd.transform);
                if let Some(transform) = cmd.brush_transform {
                    finite_affine(&mut fault, "brush transform", transform);
                }
                finite_path(&mut fault, "shape", &cmd.shape);
            }
            RenderCommand::GlyphRun(cmd) => {
                finite_affine(&mut fault, "transform", cmd.transform);
                if let Some(transform) = cmd.glyph_transform {
                    finite_affine(&mut fault, "glyph transform", transform);
                }
                finite(&mut fault, "font size", f64::from(cmd.font_size));
                finite(&mut fault, "brush alpha", f64::from(cmd.brush_alpha));
                finite_vec(&mut fault, "embolden", cmd.embolden);
                for glyph in &cmd.glyphs {
                    finite(&mut fault, "glyph x", f64::from(glyph.x));
                    finite(&mut fault, "glyph y", f64::from(glyph.y));
                }
                for coord in &cmd.normalized_coords {
                    finite(&mut fault, "normalized coord", f64::from(*coord));
                }
            }
            RenderCommand::BoxShadow(cmd) => {
                finite_affine(&mut fault, "transform", cmd.transform);
                finite(&mut fault, "rect x0", cmd.rect.x0);
                finite(&mut fault, "rect y0", cmd.rect.y0);
                finite(&mut fault, "rect x1", cmd.rect.x1);
                finite(&mut fault, "rect y1", cmd.rect.y1);
                finite(&mut fault, "radius", cmd.radius);
                finite(&mut fault, "std dev", cmd.std_dev);
            }
        }
    }
    faults
}

/// The name of an op, as the report names it.
fn kind(command: &RenderCommand) -> &'static str {
    match command {
        RenderCommand::PushLayer(_) => "PushLayer",
        RenderCommand::PushClipLayer(_) => "PushClipLayer",
        RenderCommand::PopLayer => "PopLayer",
        RenderCommand::Stroke(_) => "Stroke",
        RenderCommand::Fill(_) => "Fill",
        RenderCommand::GlyphRun(_) => "GlyphRun",
        RenderCommand::BoxShadow(_) => "BoxShadow",
    }
}

fn finite(fault: &mut impl FnMut(&str), field: &str, value: f64) {
    if !value.is_finite() {
        fault(field);
    }
}

fn finite_affine(fault: &mut impl FnMut(&str), field: &str, affine: Affine) {
    if affine.as_coeffs().iter().any(|value| !value.is_finite()) {
        fault(field);
    }
}

fn finite_vec(fault: &mut impl FnMut(&str), field: &str, vector: Vec2) {
    if !vector.x.is_finite() || !vector.y.is_finite() {
        fault(field);
    }
}

fn finite_path(fault: &mut impl FnMut(&str), field: &str, path: &kurbo::BezPath) {
    for element in path.elements() {
        let finite = match *element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => finite_point(point),
            PathEl::QuadTo(one, two) => finite_point(one) && finite_point(two),
            PathEl::CurveTo(one, two, three) => {
                finite_point(one) && finite_point(two) && finite_point(three)
            }
            PathEl::ClosePath => true,
        };
        if !finite {
            fault(field);
            return;
        }
    }
}

fn finite_point(point: kurbo::Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

/// Contract (the live run of `replace-tauri-with-blitz` that logged ~200
/// `A path contains NaN, ignoring it.` lines a second): a painted frame carries
/// no non-finite geometry. Vello drops such a path, so its element is missing
/// from the frame on every frame that paints it; and this test names the op
/// that carries it instead of leaving it to a renderer's log line.
#[rstest]
#[case::attach_mismatch(Screen::AttachMismatch)]
#[case::queue(Screen::Queue)]
#[case::queue_placeholder(Screen::QueuePlaceholder)]
#[case::panel_artboard(Screen::PanelArtboard)]
#[case::panel_folded(Screen::PanelFolded)]
#[case::panel_compact_strip(Screen::PanelCompactStrip)]
#[case::panel_rail(Screen::PanelRail)]
#[case::library(Screen::Library)]
#[case::settings(Screen::Settings)]
fn the_window_paints_no_non_finite_geometry(#[case] screen: Screen) {
    let mut fixture = Fixture::built(screen);
    let scene = fixture.painted();
    let faults = non_finite_geometry(&scene);
    assert!(
        faults.is_empty(),
        "{screen:?} paints {} non-finite ops of {}: {faults:?}",
        faults.len(),
        scene.commands.len()
    );
}
