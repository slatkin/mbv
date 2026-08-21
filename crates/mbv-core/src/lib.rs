pub mod api;
pub mod applog;
pub mod audiobookshelf;
pub mod audiobookshelf_socket;
pub(crate) mod bounded;
pub mod config;
pub mod ctrl;
pub mod daemon;
pub mod id_types;
pub use id_types::{EmbySessionId, ItemId, MediaSourceId};
pub mod playback_queue;
pub mod player;
pub mod remote_player;
pub(crate) mod remote_player_connect;
pub mod remote_reconciliation;
pub mod service_runtime;
pub mod shared_client;
pub mod shared_protocol;
pub mod shared_service;
pub mod shared_state;
pub mod shared_store;
pub mod shared_worker;
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
