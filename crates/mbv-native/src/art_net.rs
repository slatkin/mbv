//! The in-process net provider (D6 and D12 of `blitz-queue-panel-spike`).
//! `mbv-art` artwork comes from the GUI core thread, and `mbv-asset` fonts come
//! from the binary. Any other request is dropped unanswered.

use std::io::Cursor;
use std::sync::mpsc::{SendError, Sender};

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request, Url};
use mbv_gui::art::{ArtRequest, ArtResponse};
use mbv_gui::core_thread::CoreInput;
use mbv_images::QUEUE_CARD_PLACEHOLDER_BYTES;

/// The longest edge an artwork image keeps (D6). Blitz decodes the PNG the
/// reply returns, so this cap bounds the RGBA8 copy the document holds.
const LONG_EDGE_PX: u32 = 512;

/// The path prefix of a vendored font under `mbv-asset://localhost` (D12).
const FONT_PREFIX: &str = "/font/";

/// The in-process net provider. It holds the core thread's sender, so an art
/// request reaches the core, and the core's answer returns through the
/// request's `reply`.
#[derive(Debug)]
pub struct MbvNetProvider {
    core: Sender<CoreInput>,
}

impl MbvNetProvider {
    /// A provider that sends art requests to `core`.
    #[must_use]
    pub fn new(core: Sender<CoreInput>) -> Self {
        Self { core }
    }

    fn fetch_art(&self, url: &Url, handler: Box<dyn NetHandler>) {
        let resolved = url.to_string();
        let path = url.path().trim_start_matches('/').to_owned();
        let reply = Box::new(move |answer: ArtResponse| {
            handler.bytes(resolved, Bytes::from(art_bytes(answer)));
        });
        // The core thread is gone when the send fails (the window is closing).
        // The unsent request comes back in the error, and its reply answers
        // here, so the image still paints the placeholder.
        if let Err(SendError(CoreInput::Art(request))) =
            self.core.send(CoreInput::Art(ArtRequest { path, reply }))
        {
            (request.reply)(ArtResponse::Missing);
        }
    }
}

impl NetProvider for MbvNetProvider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        match request.url.scheme() {
            "mbv-art" => self.fetch_art(&request.url, handler),
            "mbv-asset" => fetch_font(&request.url, handler),
            _ => {}
        }
    }
}

/// The bytes an art answer shows: the source as a PNG with its long edge at
/// most `LONG_EDGE_PX`. A missing source, or one that does not decode, shows
/// the bundled placeholder.
fn art_bytes(answer: ArtResponse) -> Vec<u8> {
    let scaled = match answer {
        ArtResponse::Found { bytes, .. } => downscale_png(&bytes),
        ArtResponse::Missing => None,
    };
    scaled.unwrap_or_else(|| QUEUE_CARD_PLACEHOLDER_BYTES.to_vec())
}

/// Answer a font request from the vendored bytes (D12). A font with no
/// vendored bytes drops its handler unanswered: `NetHandler` in
/// `blitz-traits` 0.3.0-beta.4 has no error callback (gap G8).
fn fetch_font(url: &Url, handler: Box<dyn NetHandler>) {
    if let Some(bytes) = url.path().strip_prefix(FONT_PREFIX).and_then(font_bytes) {
        handler.bytes(url.to_string(), Bytes::from_static(bytes));
    }
}

/// The vendored WOFF2 bytes for `file`: the six Latin faces of `IBM Plex Sans`
/// and `JetBrains Mono` at weights 400, 500, and 600 (D12). Any other file has
/// no bytes.
fn font_bytes(file: &str) -> Option<&'static [u8]> {
    match file {
        "ibm-plex-sans-400.woff2" => {
            Some(include_bytes!("../assets/fonts/ibm-plex-sans-400.woff2"))
        }
        "ibm-plex-sans-500.woff2" => {
            Some(include_bytes!("../assets/fonts/ibm-plex-sans-500.woff2"))
        }
        "ibm-plex-sans-600.woff2" => {
            Some(include_bytes!("../assets/fonts/ibm-plex-sans-600.woff2"))
        }
        "jetbrains-mono-400.woff2" => {
            Some(include_bytes!("../assets/fonts/jetbrains-mono-400.woff2"))
        }
        "jetbrains-mono-500.woff2" => {
            Some(include_bytes!("../assets/fonts/jetbrains-mono-500.woff2"))
        }
        "jetbrains-mono-600.woff2" => {
            Some(include_bytes!("../assets/fonts/jetbrains-mono-600.woff2"))
        }
        _ => None,
    }
}

/// The source decoded and scaled to the long-edge cap, as PNG. `None` when the
/// source does not decode or does not encode.
fn downscale_png(source: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(source).ok()?;
    let fitted = if image.width().max(image.height()) > LONG_EDGE_PX {
        image.thumbnail(LONG_EDGE_PX, LONG_EDGE_PX)
    } else {
        image
    };
    let mut out = Cursor::new(Vec::new());
    fitted.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{Receiver, channel};
    use std::sync::{Arc, Mutex};

    use blitz_dom::Document as _;
    use dioxus_native::prelude::*;
    use dioxus_native::{DioxusDocument, DocumentConfig};

    use super::*;

    const FONT_URL: &str = "mbv-asset://localhost/font/ibm-plex-sans-400.woff2";

    #[component]
    fn art_app() -> Element {
        rsx! { img { src: "mbv-art://localhost/card/abc" } }
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

    /// A document that loads through `provider`, after its first build.
    fn built_document(app: fn() -> Element, provider: Arc<dyn NetProvider>) -> DioxusDocument {
        let mut doc = DioxusDocument::new(
            VirtualDom::new(app),
            DocumentConfig {
                net_provider: Some(provider),
                ..Default::default()
            },
        );
        doc.initial_build();
        doc
    }

    /// The one art request the core received.
    fn art_request(core: &Receiver<CoreInput>) -> ArtRequest {
        match core.try_recv() {
            Ok(CoreInput::Art(request)) => request,
            other => panic!("the image load reached the core as an art request: {other:?}"),
        }
    }

    /// The width and height of the raster the `img` node holds.
    fn image_size(doc: &DioxusDocument) -> (u32, u32) {
        let inner = doc.inner();
        let node = inner
            .query_selector("img")
            .expect("the selector is valid")
            .expect("the img node exists");
        let raster = inner
            .get_node(node)
            .expect("the img node is in the document")
            .element_data()
            .expect("the img node is an element")
            .raster_image_data()
            .expect("the image node holds a decoded raster");
        (raster.width, raster.height)
    }

    fn placeholder_size() -> (u32, u32) {
        let placeholder = image::load_from_memory(QUEUE_CARD_PLACEHOLDER_BYTES)
            .expect("the bundled placeholder decodes");
        (placeholder.width(), placeholder.height())
    }

    /// Records each URL the document asks the provider for, then delegates.
    struct Recording {
        inner: MbvNetProvider,
        requested: Arc<Mutex<Vec<String>>>,
    }

    impl NetProvider for Recording {
        fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
            self.requested
                .lock()
                .expect("the request log is not poisoned")
                .push(request.url.to_string());
            self.inner.fetch(doc_id, request, handler);
        }
    }

    #[test]
    fn found_artwork_reaches_the_image_node_downscaled() {
        let (tx, rx) = channel();
        let mut doc = built_document(art_app, Arc::new(MbvNetProvider::new(tx)));
        let request = art_request(&rx);
        assert_eq!(request.path, "card/abc");
        (request.reply)(ArtResponse::Found {
            mime: "image/png",
            bytes: source_png(1024, 768),
        });
        doc.inner_mut().handle_messages();
        assert_eq!(image_size(&doc), (512, 384));
    }

    #[test]
    fn missing_artwork_shows_the_placeholder_raster() {
        let (tx, rx) = channel();
        let mut doc = built_document(art_app, Arc::new(MbvNetProvider::new(tx)));
        (art_request(&rx).reply)(ArtResponse::Missing);
        doc.inner_mut().handle_messages();
        assert_eq!(image_size(&doc), placeholder_size());
    }

    #[test]
    fn inline_font_face_is_requested_through_the_provider() {
        let (tx, _rx) = channel();
        let requested = Arc::new(Mutex::new(Vec::new()));
        let provider = Recording {
            inner: MbvNetProvider::new(tx),
            requested: Arc::clone(&requested),
        };
        let mut doc = built_document(font_app, Arc::new(provider));
        doc.inner_mut().handle_messages();
        let urls = requested
            .lock()
            .expect("the request log is not poisoned")
            .clone();
        assert!(urls.iter().any(|url| url == FONT_URL), "{urls:?}");
    }
}
