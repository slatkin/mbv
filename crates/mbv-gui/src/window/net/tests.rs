//! Headless tests of the net provider (D16): one per contract of task 1.5 —
//! a `Found` answer gives the `img` node a raster and reports the source
//! size, a `Missing` answer leaves the node unloaded and reports the
//! failure, and an inline `@font-face` source is requested through the
//! provider. No test opens a window or sleeps.

use std::io::Cursor;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use futures_channel::mpsc::unbounded;

use super::{ArtEvent, MbvNetProvider};
use crate::art::{ArtRequest, ArtResponse};
use crate::core_thread::CoreInput;
use crate::window::channel::WindowEvent;

/// The font URL the inline `@font-face` rule requests.
const FONT_URL: &str = "mbv-asset://localhost/font/ibm-plex-sans-400.woff2";

/// The art URL the `img` element requests.
const ART_URL: &str = "mbv-art://localhost/card/abc";

#[component]
fn art_app() -> Element {
    rsx! { img { src: ART_URL } }
}

#[component]
fn font_app() -> Element {
    rsx! {
        style { "@font-face {{ font-family: 'IBM Plex Sans'; src: url('{FONT_URL}') format('woff2'); font-weight: 400; }}" }
        div { "text" }
    }
}

/// A PNG of the given size, the source an art answer carries.
fn source_png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("the test source encodes");
    out.into_inner()
}

/// A document that loads through a fresh provider: the core channel's two
/// ends come back, and the window channel's receiver comes back, so the test
/// can answer the request and read the reported events.
fn built_document(
    app: fn() -> Element,
) -> (
    DioxusDocument,
    Receiver<CoreInput>,
    UnboundedReceiver<WindowEvent>,
) {
    let (core, core_receiver) = channel();
    let (events, event_receiver) = unbounded();
    let mut doc = DioxusDocument::new(
        VirtualDom::new(app),
        DocumentConfig {
            net_provider: Some(Arc::new(MbvNetProvider::new(core, events))),
            ..Default::default()
        },
    );
    doc.initial_build();
    (doc, core_receiver, event_receiver)
}

/// The one art request the core received.
fn art_request(core: &Receiver<CoreInput>) -> ArtRequest {
    match core.try_recv() {
        Ok(CoreInput::Art(request)) => request,
        other => panic!("the image load reached the core as an art request: {other:?}"),
    }
}

/// The raster the `img` node holds, if any, as its width and height.
fn image_raster(doc: &DioxusDocument) -> Option<(u32, u32)> {
    let inner = doc.inner();
    let node = inner
        .query_selector("img")
        .expect("the selector is valid")
        .expect("the img node exists");
    inner
        .get_node(node)
        .expect("the img node is in the document")
        .element_data()
        .expect("the img node is an element")
        .raster_image_data()
        .map(|raster| (raster.width, raster.height))
}

/// The one event the window channel received.
fn reported(events: &mut UnboundedReceiver<WindowEvent>) -> ArtEvent {
    match events.try_recv() {
        Ok(WindowEvent::Art(event)) => event,
        other => panic!("the provider reported one art event on the window channel: {other:?}"),
    }
}

/// Contract 1 (task 1.5): a `Found` answer gives the `img` node a raster at
/// the source's own size — the bytes stay unchanged (D7) — and reports
/// `Loaded` with the source size the image header names.
#[test]
fn found_artwork_gives_the_img_node_a_raster_and_reports_the_source_size() {
    let (mut doc, core, mut events) = built_document(art_app);
    let request = art_request(&core);
    assert_eq!(request.path, "card/abc");
    (request.reply)(ArtResponse::Found {
        mime: "image/png",
        bytes: source_png(64, 48),
    });
    doc.inner_mut().handle_messages();

    assert_eq!(image_raster(&doc), Some((64, 48)));
    assert_eq!(
        reported(&mut events),
        ArtEvent::Loaded {
            url: ART_URL.to_owned(),
            width: 64,
            height: 48,
        }
    );
}

/// Contract 2 (task 1.5): a `Missing` answer leaves the node unloaded — the
/// handler drops, so no placeholder raster lands (D7) — and reports `Failed`.
#[test]
fn missing_artwork_leaves_the_node_unloaded_and_reports_the_failure() {
    let (mut doc, core, mut events) = built_document(art_app);
    (art_request(&core).reply)(ArtResponse::Missing);
    doc.inner_mut().handle_messages();

    assert_eq!(image_raster(&doc), None);
    assert_eq!(
        reported(&mut events),
        ArtEvent::Failed {
            url: ART_URL.to_owned(),
        }
    );
}

/// Records each URL the document asks the provider for, then delegates.
struct Recording {
    inner: MbvNetProvider,
    requested: Arc<Mutex<Vec<String>>>,
}

impl blitz_traits::net::NetProvider for Recording {
    fn fetch(
        &self,
        doc_id: usize,
        request: blitz_traits::net::Request,
        handler: Box<dyn blitz_traits::net::NetHandler>,
    ) {
        self.requested
            .lock()
            .expect("the request log is not poisoned")
            .push(request.url.to_string());
        self.inner.fetch(doc_id, request, handler);
    }
}

/// Contract 3 (task 1.5): an inline `@font-face` source is requested through
/// the provider, so a font the binary bundles (task 2.1) loads in process.
#[test]
fn inline_font_face_is_requested_through_the_provider() {
    let (core, _core_receiver) = channel();
    let (events, _event_receiver) = unbounded();
    let requested = Arc::new(Mutex::new(Vec::new()));
    let provider = Recording {
        inner: MbvNetProvider::new(core, events),
        requested: Arc::clone(&requested),
    };
    let mut doc = DioxusDocument::new(
        VirtualDom::new(font_app),
        DocumentConfig {
            net_provider: Some(Arc::new(provider)),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.inner_mut().handle_messages();

    let urls = requested
        .lock()
        .expect("the request log is not poisoned")
        .clone();
    assert!(urls.iter().any(|url| url == FONT_URL), "{urls:?}");
}
