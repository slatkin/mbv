//! The `mbv-art` URI scheme's vocabulary (design D7 of
//! `gui-change-driven-sections`): the paths the window's image loads name,
//! the request and answer that cross the core channel, and the art reference
//! the section DTOs carry.
//!
//! The Tauri handler never touches the `App`: it pushes an [`ArtRequest`]
//! into the core channel with the `UriSchemeResponder` wired into the
//! request's `reply`, and the tick host answers through the `App`'s
//! read-only cache queries. The scheme name and the path encoding live here,
//! so no other call site builds or parses an art URL.

use std::fmt;
use std::fmt::Write as _;

use serde::Serialize;

/// The scheme host the window loads images through (design D7). On Linux,
/// `WebKitGTK` serves the scheme as `mbv-art://localhost/<path>`.
const SCHEME_HOST: &str = "mbv-art://localhost";

/// The path that names the queue card's bundled placeholder bytes.
pub const PLACEHOLDER_PATH: &str = "placeholder";

/// The image reference a section DTO carries (design D7): the window paints
/// `src` as the image's URL, and a `loading` reference paints the loading
/// fill. `src` is built only by [`art_url`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GuiArtRef {
    /// The cached image the URL names.
    Image { src: String },
    /// The bundled placeholder image the URL names.
    Placeholder { src: String },
    /// No URL: a fetch is in flight or awaits its retry.
    Loading,
}

/// The URL for `path`, as the webview requests it (design D7): one place owns
/// the scheme name, so the wire spelling lives here alone.
#[must_use]
pub fn art_url(path: &str) -> String {
    format!("{SCHEME_HOST}/{path}")
}

/// The request path for the image-cache entry `cache_key` names (design D7).
#[must_use]
pub fn card_path(cache_key: &str) -> String {
    format!("card/{}", percent_encode(cache_key))
}

/// The request path for the trickplay sheet `wire_key` names (design D7).
#[must_use]
pub fn trickplay_path(wire_key: &str) -> String {
    format!("trickplay/{}", percent_encode(wire_key))
}

/// One request the window's image load raised (design D7): the percent-encoded
/// request path and the one-shot edge that answers it. Production wires the
/// `reply` to a Tauri `UriSchemeResponder`; the tests mock it, so
/// `core_thread` stays free of Tauri types.
pub struct ArtRequest {
    /// The request path as the webview sent it, percent-encoded and without
    /// the leading slash.
    pub path: String,
    /// The edge the one answer travels through. Every request is answered
    /// exactly once; the pre-attach campaign answers `Missing`, so no
    /// responder is left open before the attach.
    pub reply: Box<dyn FnOnce(ArtResponse) + Send>,
}

impl fmt::Debug for ArtRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArtRequest")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// The answer one [`ArtRequest`] carries (design D7). `Found` becomes a 200
/// response with the source's own MIME type and an immutable-cache header;
/// `Missing` a 404 the webview does not keep, so a later load of the same URL
/// asks the core again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtResponse {
    /// The cached source: its bytes and the MIME type those bytes declare.
    Found {
        /// The MIME type the cached source's bytes declare.
        mime: &'static str,
        /// The cached bytes, as the server sent them.
        bytes: Vec<u8>,
    },
    /// The cache holds nothing under the path's key, or the path is malformed.
    Missing,
}

/// The path an art request names (design D7): the image-cache key a card
/// names, the trickplay wire key a sheet names, or the placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtPath {
    Card(String),
    Trickplay(String),
    Placeholder,
}

/// Parse a request path (design D7): the three forms only. A card or
/// trickplay segment is percent-decoded; another prefix, a missing segment,
/// or an undecodable one is malformed, and the request answers `Missing`.
pub(crate) fn parse_path(path: &str) -> Option<ArtPath> {
    if path == PLACEHOLDER_PATH {
        return Some(ArtPath::Placeholder);
    }
    let (kind, rest) = path.split_once('/')?;
    match kind {
        "card" => Some(ArtPath::Card(percent_decode(rest)?)),
        "trickplay" => Some(ArtPath::Trickplay(percent_decode(rest)?)),
        _ => None,
    }
}

/// Percent-encode everything outside the RFC 3986 unreserved set, so a cache
/// key's `:` and `,` cannot split the path and no raw space or `%` reaches the
/// URL.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char);
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

/// Decode a percent-encoded path segment: each `%XX` pair to its byte,
/// everything else as itself. `None` for a truncated or non-hex escape or a
/// body that is not UTF-8 — the path is malformed, not silently repaired.
fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_digit(*bytes.get(index + 1)?)?;
            let low = hex_digit(*bytes.get(index + 2)?)?;
            out.push(high * 16 + low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The value of one ASCII hex digit, `None` for anything else.
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
