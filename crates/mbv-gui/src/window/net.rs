//! The in-process net provider (D7 of `replace-tauri-with-blitz`):
//! `mbv-art` artwork comes from the GUI core thread, `mbv-asset` fonts come
//! from the binary, and an art answer reports to the root through the window
//! channel. Any other request drops with a debug log, and a missed art answer
//! has no handler to call — `NetHandler` has no error callback (gap G8).

use std::io::{self, Cursor};
use std::sync::mpsc::{SendError, Sender};

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request, Url};

use crate::art::{ArtRequest, ArtResponse};
use crate::core_thread::CoreInput;
use crate::window::channel::{WindowEvent, WindowEventSender};

/// The path prefix of a vendored font under `mbv-asset://localhost` (D7).
const FONT_PREFIX: &str = "/font/";

/// The art route's answer for the window (D7). The webview's `onerror` and
/// `onload` handlers have no equivalent here (D8): the natural size arrives
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

/// The in-process net provider (D7). It holds the core thread's sender, so an
/// art request reaches the core, and the window channel's sender, so the art
/// answers report to the root.
#[derive(Debug)]
pub struct MbvNetProvider {
    core: Sender<CoreInput>,
    events: WindowEventSender,
}

impl MbvNetProvider {
    /// A provider that sends art requests to `core` and art answers to
    /// `events`.
    #[must_use]
    pub fn new(core: Sender<CoreInput>, events: WindowEventSender) -> Self {
        Self { core, events }
    }

    /// The `mbv-art` route (D7): the request goes to the core, and the reply
    /// answers the handler in place. The bytes stay unchanged; only the
    /// header size travels to the root as [`ArtEvent::Loaded`].
    fn fetch_art(&self, url: &Url, handler: Box<dyn NetHandler>) {
        let resolved = url.to_string();
        let path = url.path().trim_start_matches('/').to_owned();
        let events = self.events.clone();
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
                        handler.bytes(resolved, Bytes::from(bytes));
                    }
                    // A source whose header does not read cannot carry a size,
                    // so it answers as a miss.
                    Err(error) => {
                        tracing::debug!(
                            target: "gui_art",
                            url = %resolved,
                            error = %error,
                            "the art source's image header does not read"
                        );
                        drop(handler);
                        let _ = events
                            .unbounded_send(WindowEvent::Art(ArtEvent::Failed { url: resolved }));
                    }
                }
            }
            ArtResponse::Missing => {
                // Gap G8: no error callback, so the handler drops unanswered
                // and the node stays unloaded (D7).
                drop(handler);
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
            "mbv-asset" => fetch_font(&request.url, handler),
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
/// bytes. A file with no vendored bytes drops its handler unanswered (gap
/// G8).
fn fetch_font(url: &Url, handler: Box<dyn NetHandler>) {
    if let Some(bytes) = url.path().strip_prefix(FONT_PREFIX).and_then(font_bytes) {
        handler.bytes(url.to_string(), Bytes::from_static(bytes));
    }
}

/// The vendored WOFF2 bytes for `file` (D7, D11): the six Latin faces (weights
/// 400, 500, 600) of `IBM Plex Sans` and `JetBrains Mono` 5.3.0.
fn font_bytes(file: &str) -> Option<&'static [u8]> {
    match file {
        "ibm-plex-sans-latin-400-normal.woff2" => Some(FONT_IBM_PLEX_SANS_400),
        "ibm-plex-sans-latin-500-normal.woff2" => Some(FONT_IBM_PLEX_SANS_500),
        "ibm-plex-sans-latin-600-normal.woff2" => Some(FONT_IBM_PLEX_SANS_600),
        "jetbrains-mono-latin-400-normal.woff2" => Some(FONT_JETBRAINS_MONO_400),
        "jetbrains-mono-latin-500-normal.woff2" => Some(FONT_JETBRAINS_MONO_500),
        "jetbrains-mono-latin-600-normal.woff2" => Some(FONT_JETBRAINS_MONO_600),
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

#[cfg(test)]
mod tests;
