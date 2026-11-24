//! The in-process net provider (D7 of `replace-tauri-with-blitz`):
//! `mbv-art` artwork comes from the GUI core thread, `mbv-asset` fonts come
//! from the binary, and an art answer reports to the root through the window
//! channel. Any other request drops with a debug log, and a missed art answer
//! has no handler to call — `NetHandler` has no error callback (gap G8).
//!
//! Both routes deliver their bytes into the document's message channel, and
//! Blitz drains that channel only inside `BaseDocument::resolve` (D12): a
//! font registers and re-shapes text, an image clears the node's layout cache
//! and inserts `ALL_DAMAGE`, then the same resolve lays out again and clears
//! the damage. So a net answer changes layout with nothing for a frame's
//! change fact to read before its resolve, and each delivery notes one
//! [`NetLoads`] arrival that the launch copy reports on the frame it
//! presents.

use std::io::{self, Cursor};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SendError, Sender};

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request, Url};

use crate::art::{ArtRequest, ArtResponse};
use crate::core_thread::CoreInput;
use crate::window::channel::{WindowEvent, WindowEventSender};

/// The path prefix of a vendored font under `mbv-asset://localhost` (D7).
const FONT_PREFIX: &str = "/font/";

/// The art route's answer for the window (D7). Blitz has no image error or load
/// handler on the element (D8): the natural size arrives
/// only as `Loaded`, and a failure only as `Failed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtEvent {
    /// The art request found its source: the image header's size, in device
    /// pixels, of the bytes the document now holds.
    Loaded {
        /// The requested URL, as the document named it.
        url: String,
        /// The source's header width, in device pixels.
        width: u32,
        /// The source's header height, in device pixels.
        height: u32,
    },
    /// The art request found nothing (or a source whose image header does not
    /// read): the node stays unloaded, and `ArtImage`'s retry rules apply.
    Failed {
        /// The requested URL, as the document named it.
        url: String,
    },
}

/// The last fact an art URL reported (D7): the store
/// (`window/components/art_image.rs`) holds one per URL, and a mounted
/// `ArtImage` reads its painted URL's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtStatus {
    /// The source loaded: its natural size, in device pixels.
    Loaded {
        /// The natural width, in device pixels.
        width: u32,
        /// The natural height, in device pixels.
        height: u32,
    },
    /// The source missed once more: the misses since its last load.
    Failed {
        /// The misses since the URL's last load.
        misses: u64,
    },
}

/// The net answers the document has to lay out again (D7, D12), the provider's
/// one flag for the launch copy.
///
/// A net answer changes layout without a tree change: Blitz applies the bytes
/// inside a resolve (`BaseDocument::handle_message`), so the change fact a
/// frame reads before its resolve cannot see it. A font registers and re-shapes
/// the measured text; an image gives its node an intrinsic size, which clears
/// that node's layout cache and inserts `ALL_DAMAGE` inside the same resolve.
/// The provider sets this flag, the launch copy presents one frame for it
/// ([`crate::window::launch`]), and that frame reports the change so the
/// measure runs after the resolve that laid the answer out. A plain atomic
/// flag, not a signal: nothing in the tree subscribes to it.
#[derive(Clone, Debug, Default)]
pub struct NetLoads(Arc<AtomicBool>);

impl NetLoads {
    /// A flag with no answer waiting.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One answer arrived for the document to lay out again (D7): the next
    /// presented frame reports the change and measures the new layout.
    pub fn note(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Take the pending answer, clearing it: the frame reports one change per
    /// answer, so the frame it drives reports nothing and asks for no other.
    #[must_use]
    pub fn take(&self) -> bool {
        self.0.swap(false, Ordering::Relaxed)
    }

    /// Whether an answer is waiting for the frame that reports it.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Deliver one answer into the document's message channel (D7, D12), noting
    /// the arrival on both sides of the send.
    ///
    /// The queue write and this flag are two facts a frame observes separately,
    /// and an art answer is delivered from the core thread, so the window's
    /// thread can enter a frame between them. A frame that reads the flag after
    /// the send resolves the answer and measures it; a frame that read before
    /// the send leaves the second note for the frame after, which then resolves
    /// the answer the first frame did not see. Either way one frame reports it
    /// after the resolve, and the extra frame costs one measure that writes
    /// nothing.
    fn deliver(loads: &Self, handler: Box<dyn NetHandler>, url: String, bytes: Bytes) {
        loads.note();
        handler.bytes(url, bytes);
        loads.note();
    }
}

/// The in-process net provider (D7). It holds the core thread's sender, so an
/// art request reaches the core, and the window channel's sender, so the art
/// answers report to the root.
#[derive(Debug)]
pub struct MbvNetProvider {
    core: Sender<CoreInput>,
    events: WindowEventSender,
    loads: NetLoads,
}

impl MbvNetProvider {
    /// A provider that sends art requests to `core`, art answers to `events`,
    /// and the net answers the document lays out to `loads`.
    #[must_use]
    pub fn new(core: Sender<CoreInput>, events: WindowEventSender, loads: NetLoads) -> Self {
        Self {
            core,
            events,
            loads,
        }
    }

    /// The `mbv-art` route (D7): the request goes to the core, and the reply
    /// answers the handler in place. The bytes stay unchanged; only the
    /// header size travels to the root as [`ArtEvent::Loaded`].
    fn fetch_art(&self, url: &Url, handler: Box<dyn NetHandler>) {
        let resolved = url.to_string();
        let path = url.path().trim_start_matches('/').to_owned();
        let events = self.events.clone();
        let loads = self.loads.clone();
        let reply = Box::new(move |answer: ArtResponse| match answer {
            ArtResponse::Found { bytes, .. } => {
                let read = image::ImageReader::new(Cursor::new(&bytes))
                    .with_guessed_format()
                    .and_then(|reader| {
                        reader
                            .into_dimensions()
                            .map_err(|error| io::Error::other(error.to_string()))
                    });
                match read {
                    Ok((width, height)) => {
                        let _ = events.unbounded_send(WindowEvent::Art(ArtEvent::Loaded {
                            url: resolved.clone(),
                            width,
                            height,
                        }));
                        NetLoads::deliver(&loads, handler, resolved, Bytes::from(bytes));
                    }
                    // A source whose header does not read cannot carry a size,
                    // so it answers as a miss — the same answer a miss gives,
                    // so Blitz clears the URL's pending-images entry and the
                    // node stays unloaded.
                    Err(error) => {
                        tracing::debug!(
                            target: "gui_art",
                            url = %resolved,
                            error = %error,
                            "the art source's image header does not read"
                        );
                        // Bytes Blitz cannot parse apply no raster
                        // (`load_resource` returns on the parse error), so
                        // nothing is laid out again and no arrival is noted:
                        // the answer only clears the URL's pending entry, which
                        // no layout reads.
                        handler.bytes(resolved.clone(), Bytes::new());
                        let _ = events
                            .unbounded_send(WindowEvent::Art(ArtEvent::Failed { url: resolved }));
                    }
                }
            }
            ArtResponse::Missing => {
                // Gap G8: no error callback, so the handler answers with empty
                // bytes — `ImageHandler::parse` fails on them, so Blitz clears
                // the URL's pending-images entry and the node stays unloaded.
                // A dropped handler would leave the entry pending forever: the
                // retry's recreated `img` would join a dead waiting list and
                // never ask the core again (D7's retry contract, invariant 27).
                // Nothing is laid out again, so no arrival is noted; the
                // retry's recreated node is a tree change of its own.
                handler.bytes(resolved.clone(), Bytes::new());
                let _ = events.unbounded_send(WindowEvent::Art(ArtEvent::Failed { url: resolved }));
            }
        });
        // The core thread is gone when the send fails (the window is closing).
        // The unsent request comes back in the error, and its reply answers
        // here, so the image does not wait for an answer that never comes.
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
            "mbv-asset" => fetch_font(&request.url, handler, &self.loads),
            _ => {
                tracing::debug!(
                    target: "gui_art",
                    url = %request.url,
                    "the net provider names no route for the URL; the request drops"
                );
            }
        }
    }
}

/// The `mbv-asset` route (D7): a bundled font answers from the binary's
/// bytes, and the load is noted so the window presents the frame that measures
/// the re-shaped text (D12). A file with no vendored bytes drops its handler
/// unanswered (gap G8).
fn fetch_font(url: &Url, handler: Box<dyn NetHandler>, loads: &NetLoads) {
    if let Some(bytes) = url.path().strip_prefix(FONT_PREFIX).and_then(font_bytes) {
        NetLoads::deliver(loads, handler, url.to_string(), Bytes::from_static(bytes));
    }
}

/// The vendored WOFF2 bytes for `file` (D7, D11): the seven Latin faces of
/// `IBM Plex Sans` (weights 400, 500, 600) and `JetBrains Mono` (weights 400,
/// 500, 600, 700) 5.3.0. The ellipsis measure (`window/truncate.rs`) registers
/// the same bytes.
pub(crate) fn font_bytes(file: &str) -> Option<&'static [u8]> {
    match file {
        "ibm-plex-sans-latin-400-normal.woff2" => Some(FONT_IBM_PLEX_SANS_400),
        "ibm-plex-sans-latin-500-normal.woff2" => Some(FONT_IBM_PLEX_SANS_500),
        "ibm-plex-sans-latin-600-normal.woff2" => Some(FONT_IBM_PLEX_SANS_600),
        "jetbrains-mono-latin-400-normal.woff2" => Some(FONT_JETBRAINS_MONO_400),
        "jetbrains-mono-latin-500-normal.woff2" => Some(FONT_JETBRAINS_MONO_500),
        "jetbrains-mono-latin-600-normal.woff2" => Some(FONT_JETBRAINS_MONO_600),
        "jetbrains-mono-latin-700-normal.woff2" => Some(FONT_JETBRAINS_MONO_700),
        _ => None,
    }
}

const FONT_IBM_PLEX_SANS_400: &[u8] =
    include_bytes!("../../assets/fonts/ibm-plex-sans-latin-400-normal.woff2");
const FONT_IBM_PLEX_SANS_500: &[u8] =
    include_bytes!("../../assets/fonts/ibm-plex-sans-latin-500-normal.woff2");
const FONT_IBM_PLEX_SANS_600: &[u8] =
    include_bytes!("../../assets/fonts/ibm-plex-sans-latin-600-normal.woff2");
const FONT_JETBRAINS_MONO_400: &[u8] =
    include_bytes!("../../assets/fonts/jetbrains-mono-latin-400-normal.woff2");
const FONT_JETBRAINS_MONO_500: &[u8] =
    include_bytes!("../../assets/fonts/jetbrains-mono-latin-500-normal.woff2");
const FONT_JETBRAINS_MONO_600: &[u8] =
    include_bytes!("../../assets/fonts/jetbrains-mono-latin-600-normal.woff2");
const FONT_JETBRAINS_MONO_700: &[u8] =
    include_bytes!("../../assets/fonts/jetbrains-mono-latin-700-normal.woff2");

#[cfg(test)]
mod tests;
