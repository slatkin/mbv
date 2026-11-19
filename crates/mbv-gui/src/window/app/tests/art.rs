//! Headless end-to-end test of the now-playing artwork (D7): the window's own
//! `ArtImage` over the `mbv-art` route, with the core's answer delivered as
//! the net provider delivers it and the answer's natural size forwarded into
//! the art store as the root's drain task forwards it. Nothing opens a
//! window, sleeps, or reaches a core: the core's edge is a recording channel,
//! and the art answer is real PNG bytes.
//!
//! The contract is the still slot's: an `Image` reference paints an `img` that
//! Blitz rasterises, lays out with a box of its own, and paints. A reference
//! that never becomes a painted `img` is the visible failure — the slot shows
//! its caption instead of the now-playing art — so the test asserts the
//! painted frame carries an image fill, not that a `src` attribute exists.
//! The window half of the now-playing art's contract: the live run of
//! 2026-11-14 that showed no art failed on the core's side (the section
//! carried no reference at all), which this test localized by painting the
//! same slot from a reference the window does carry.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use anyrender::Paint;
use anyrender::recording::{RenderCommand, Scene};
use blitz_dom::{Document as _, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use kurbo::Shape as _;
use mbv_config::GuiPanelWidths;

use crate::art::{ArtResponse, GuiArtRef, art_url, card_path};
use crate::bridge::frame::playback::{
    GuiPlayerControls, GuiQueuePlayback, GuiTransportAvailability,
};
use crate::core_thread::CoreInput;
use crate::window::channel::{CoreLink, WindowEvent};
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::components::queue_playback_panel::QueuePlaybackPanel;
use crate::window::layout::{LayoutEpoch, use_layout_reporter};
use crate::window::net::{MbvNetProvider, NetLoads};
use crate::window::playhead::{Playhead, Times};
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::Sections;

/// The viewport the window paints at.
const WIDTH: u32 = 1200;
const HEIGHT: u32 = 800;

/// The card key the stub's playing item carries (`{id}:QB`, the landscape
/// queue-card key a Movie item resolves to).
const CARD_KEY: &str = "616085:QB";

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

/// The playback section the panel paints: a playing item whose artwork is
/// the cached card the core answers.
fn playing(art: GuiArtRef) -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: "poo".to_owned(),
        host_is_remote: true,
        shown: true,
        show_controls: true,
        now_playing_title: Some("Friday Fun Stream".to_owned()),
        parent_line: Some("The Show".to_owned()),
        episode_badge: None,
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

/// What the root hands the test: the section store and the art store the
/// drain task would feed.
struct Handles {
    sections: Sections,
    art_events: ArtEvents,
}

#[derive(Clone)]
struct RootProps {
    handle: Rc<RefCell<Option<Handles>>>,
}

/// The root over the window's real stylesheet and the window's real net
/// provider, mounting the player panel's top block.
fn root(props: RootProps) -> Element {
    let RootProps { handle } = props;
    let (sender, _core) = channel::<CoreInput>();
    provide_context(CoreLink::new(sender));
    provide_context(use_layout_reporter());
    provide_context(use_hook(LayoutEpoch::new));
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let times: Arc<dyn Times> = Arc::new(FakeTimes);
    let playhead_clock = Arc::clone(&clock);
    provide_context(use_hook(move || Playhead::new(playhead_clock, times)));
    let art_events = use_hook(ArtEvents::new);
    provide_context(art_events);
    provide_context(use_hook(FiredTimers::new));
    provide_context(Arc::clone(&clock));
    provide_context(ScaleFactor(Signal::new(1.0)));
    provide_context(use_hook(|| {
        PanelWidthsStore::new(GuiPanelWidths::default(), Arc::new(|_| {}))
    }));
    handle.borrow_mut().replace(Handles {
        sections: sections.clone(),
        art_events,
    });

    let style = super::super::ROOT_STYLE.as_str();
    rsx! {
        style { "{style}" }
        main { class: "h-dvh w-full overflow-hidden bg-background text-foreground",
            div { class: "flex h-full min-h-0 flex-col",
                QueuePlaybackPanel {}
            }
        }
    }
}

/// A window whose document holds the panel, with the core's channel and the
/// art answers' channel as the test's own ends.
struct ArtWindow {
    doc: DioxusDocument,
    sections: Sections,
    art_events: ArtEvents,
    core: Receiver<CoreInput>,
    events: futures_channel::mpsc::UnboundedReceiver<WindowEvent>,
    loads: NetLoads,
}

impl ArtWindow {
    /// The window built with `art` as the playback section's artwork, and one
    /// frame presented.
    fn built(art: GuiArtRef) -> Self {
        let (core, core_receiver) = channel::<CoreInput>();
        let (event_sender, events) = futures_channel::mpsc::unbounded::<WindowEvent>();
        let loads = NetLoads::new();
        let handle = Rc::new(RefCell::new(None));
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                root,
                RootProps {
                    handle: Rc::clone(&handle),
                },
            ),
            DocumentConfig {
                viewport: Some(Viewport::new(WIDTH, HEIGHT, 1.0, ColorScheme::Light)),
                ua_stylesheets: vec![blitz_dom::UserAgentStylesheet::default_sheet()],
                net_provider: Some(Arc::new(MbvNetProvider::new(
                    core,
                    event_sender,
                    loads.clone(),
                ))),
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        let taken = handle
            .borrow_mut()
            .take()
            .expect("the root handed its stores over");
        let mut window = Self {
            doc,
            sections: taken.sections,
            art_events: taken.art_events,
            core: core_receiver,
            events,
            loads,
        };
        window.set_playback(art);
        window.present_frame();
        window
    }

    /// Write the playback section, as the core's batch would.
    fn set_playback(&mut self, art: GuiArtRef) {
        let mut playback = self.sections.playback;
        self.doc.vdom.in_runtime(move || playback.set(playing(art)));
    }

    /// One presented frame as the launch copy runs it: the change fact is read
    /// before the resolve, the resolve applies the net answers the document
    /// holds, and the art answers the drain forwarded reach the store first.
    fn present_frame(&mut self) {
        self.drain_events();
        let _ = self.loads.take();
        self.doc.inner.borrow_mut().resolve(0.0);
        for _ in 0..8 {
            if !self.doc.poll(None) {
                break;
            }
        }
        self.drain_events();
    }

    /// The root's drain task: an art answer reaches the store its `ArtImage`
    /// reads (D7).
    fn drain_events(&mut self) {
        let art_events = self.art_events;
        let mut answered = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            if let WindowEvent::Art(event) = event {
                answered.push(event);
            }
        }
        self.doc.vdom.in_runtime(move || {
            for event in answered {
                let mut art_events = art_events;
                art_events.push(event);
            }
        });
    }

    /// Answer the one art request the load raised with `bytes`, as the core
    /// thread's responder does.
    fn answer_art(&mut self, bytes: Vec<u8>) -> String {
        match self.core.try_recv() {
            Ok(CoreInput::Art(request)) => {
                let path = request.path;
                (request.reply)(ArtResponse::Found {
                    mime: "image/png",
                    bytes,
                });
                path
            }
            other => {
                panic!("the window's image load reaches the core as an art request: {other:?}")
            }
        }
    }

    /// The `img`'s `src`, `None` while the panel paints no `img`.
    fn img_src(&self) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom.query_selector("img").expect("the selector is valid")?;
        let node = dom.get_node(id).expect("the node is in the document");
        node.attr(local_name!("src")).map(str::to_owned)
    }

    /// The `img`'s laid-out box, `None` while it has none to give.
    fn img_box(&self) -> Option<(f64, f64)> {
        let dom = self.doc.inner();
        let id = dom.query_selector("img").expect("the selector is valid")?;
        let rect = dom.get_client_bounding_rect(id)?;
        Some((rect.width, rect.height))
    }

    /// The laid-out client rect of the first node the selector names.
    fn rect_of(&self, selector: &str) -> blitz_dom::BoundingRect {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_client_bounding_rect(id)
            .expect("the node is laid out")
    }

    /// Whether the `img` carries a raster: the bytes Blitz decoded for it.
    fn img_has_raster(&self) -> bool {
        let dom = self.doc.inner();
        let Some(id) = dom.query_selector("img").expect("the selector is valid") else {
            return false;
        };
        let node = dom.get_node(id).expect("the node is in the document");
        node.element_data()
            .and_then(blitz_dom::ElementData::raster_image_data)
            .is_some()
    }

    /// The image fills of one painted frame, as their painted boxes.
    fn image_fills(&mut self) -> Vec<(f64, f64)> {
        let mut scene = Scene::new();
        let mut dom = self.doc.inner.borrow_mut();
        blitz_paint::paint_scene(&mut scene, &mut dom, 1.0, WIDTH, HEIGHT, 0, 0);
        scene
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::Fill(fill) => match fill.brush {
                    Paint::Image(_) => {
                        let bounds = fill.shape.bounding_box();
                        Some((bounds.width(), bounds.height()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }
}

/// A PNG `width` by `height`, the bytes an art answer carries.
fn source_png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("the test source encodes");
    out.into_inner()
}

/// The card reference the playback section carries for `CARD_KEY`.
fn card_art() -> GuiArtRef {
    GuiArtRef::Image {
        src: art_url(&card_path(CARD_KEY)),
    }
}

/// Contract: the now-playing still slot paints its card artwork. The window
/// requests the card URL, the core's answer reaches the document as the net
/// provider delivers it, and the still slot paints the decoded image: an `img`
/// with a raster, a box of its own, and an image fill in the frame. A card art
/// that never paints leaves the slot showing its caption.
#[test]
fn the_still_slot_paints_its_card_artwork() {
    let mut window = ArtWindow::built(card_art());
    let path = window.answer_art(source_png(640, 360));
    assert_eq!(
        path,
        card_path(CARD_KEY),
        "the window requests the card URL"
    );
    window.present_frame();

    assert_eq!(
        window.img_src(),
        Some(art_url(&path)),
        "the panel paints the img"
    );
    assert!(window.img_has_raster(), "the decoded bytes reach the node");
    let (width, height) = window.img_box().expect("the img is laid out");
    assert!(
        width > 0.0 && height > 0.0,
        "the img has a box of its own: {width}x{height}"
    );
    assert!(
        !window.image_fills().is_empty(),
        "the frame paints the image"
    );
}

/// Contract (`gui-queue-panel-redesign` design D3, task 4.3): the player
/// controls paint as the canvas's two rows — the seek row alone across the
/// panel, then the transport row under it with the 44 px play control, and
/// the elapsed and remaining clocks after the next button at the row's
/// trailing edge.
#[test]
fn the_transport_row_paints_under_the_seek_row_before_the_clocks() {
    let mut window = ArtWindow::built(card_art());
    window.answer_art(source_png(640, 360));
    window.present_frame();

    let seek = window.rect_of("[data-target=\"seekbar\"]");
    let play = window.rect_of("[data-target=\"playPause\"]");
    let next = window.rect_of("[data-target=\"next\"]");
    let clocks = window.rect_of("[data-clocks]");
    assert!(
        (seek.height - 14.0).abs() < 0.5,
        "the seek row is 14 px tall: {}",
        seek.height
    );
    assert!(
        (play.width - 44.0).abs() < 0.5 && (play.height - 44.0).abs() < 0.5,
        "the play control is the canvas's 44 px circle: {}x{}",
        play.width,
        play.height
    );
    assert!(
        play.y >= seek.y + seek.height,
        "the transport row starts under the seek row: play {} at {}, seek ends at {}",
        play.y,
        play.height,
        seek.y + seek.height
    );
    assert!(
        clocks.y >= play.y && clocks.y + clocks.height <= play.y + play.height + 0.5,
        "the clocks paint on the transport row: clocks {} at {}, play {} at {}",
        clocks.height,
        clocks.y,
        play.height,
        play.y
    );
    assert!(
        next.x + next.width <= clocks.x,
        "the clocks follow the next button: next ends at {}, clocks start at {}",
        next.x + next.width,
        clocks.x
    );
}
