use rand::RngExt;

pub mod api;
pub mod applog;
pub mod audiobookshelf;
pub mod audiobookshelf_socket;
pub(crate) mod bounded;
pub mod cast_client;
pub mod cast_discovery;
pub mod cast_dispatch;
pub mod config;
pub mod ctrl;
pub mod daemon;
pub(crate) mod daemon_ctrl;
pub mod feed_entry_state;
pub mod id_types;
pub use id_types::{EmbySessionId, ItemId, MediaSourceId};
pub mod keybinds;
#[cfg(any(test, feature = "test-support"))]
pub mod mock_http;
pub mod playback;
pub use playback::execution_sequence as playback_execution_sequence;
pub use playback::queue as playback_queue;
pub use playback::transition as playback_transition;
pub mod player;
/// Compatibility re-export for callers that used the former flat module path.
pub mod player_owner_state {
    pub use crate::player::owner_state::*;
}
pub mod remote_player;
pub mod service_runtime;
pub(crate) mod stream;
pub mod ws;

/// Characters outside RFC 3986's unreserved set (`ALPHA / DIGIT / "-" / "." /
/// "_" / "~"`), which must be percent-encoded before going into a URL path
/// segment. ureq 3.x builds requests via `http::Uri` and rejects invalid
/// characters outright rather than encoding them (unlike ureq 2.x's more
/// lenient URL builder), so any server-returned ID or user-entered string
/// interpolated into a path must be encoded explicitly.
const PATH_SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Percent-encode a single URL path segment (an ID, name, or search term),
/// not a full path -- do not pass a string containing `/`.
pub(crate) fn encode_path_segment(value: &str) -> percent_encoding::PercentEncode<'_> {
    percent_encoding::utf8_percent_encode(value, PATH_SEGMENT)
}

/// Sleep out one reconnect attempt: exponential backoff with 0–1s jitter,
/// doubling `backoff_secs` up to 60s. Shared by the Emby and Audiobookshelf
/// websocket reconnect loops so the incantation lives in one place.
pub(crate) fn reconnect_backoff_sleep(backoff_secs: &mut u64, target: &str) {
    let jitter: f64 = rand::rng().random_range(0.0..1.0);
    let delay = std::time::Duration::from_secs_f64(*backoff_secs as f64 + jitter);
    log::info!(
        target: target,
        "reconnecting in {:.1}s (backoff={}s)",
        delay.as_secs_f64(),
        backoff_secs
    );
    std::thread::sleep(delay);
    *backoff_secs = (*backoff_secs * 2).min(60);
}

/// Build an agent using the explicit native-tls provider (ureq 3 no longer
/// auto-enables native-tls from the feature flag). Shared by the Emby and
/// Audiobookshelf clients so the TLS-config incantation lives in one place.
pub fn native_tls_agent(
    connect_timeout: Option<std::time::Duration>,
    global_timeout: Option<std::time::Duration>,
) -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .timeout_connect(connect_timeout)
        .timeout_global(global_timeout)
        .build()
        .into()
}
