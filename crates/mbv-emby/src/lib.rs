//! `Emby` Service client: library, playback, sessions, and progress reporting.
//!
//! `EmbyClient`, credential exchange, setup validation, and session control live
//! here. Item models and tick units live in `mbv-emby-model`; the websocket
//! transport itself lives in `mbv-ws`.

mod error;
#[doc(inline)]
pub use error::EmbyError;
mod failure;
#[doc(inline)]
pub use failure::{EmbyBootstrap, EmbyFailure, EmbyFailureClass};
mod types;
pub(crate) use types::load_cached_token;
#[cfg(test)]
#[doc(inline)]
pub use types::save_cached_token;
#[doc(inline)]
pub use types::{
    EmbyClient, MBV_DIRECT_TCP_PORT_PREFIX, PlaybackInfo, SessionAudioStream, SessionInfo,
    SessionMediaInfo, SessionSubtitleStream, clear_cached_token, device_id, device_name,
    gen_session_id, mbv_direct_tcp_port_command, parse_audio_info, parse_item,
    parse_mbv_direct_tcp_port, parse_session_media_info, parse_video_info,
};
mod client_auth;
mod types_parsing;
#[doc(inline)]
pub use client_auth::EmbyCredentialExchange;
mod client_library;
#[doc(inline)]
pub use client_library::SortedItemsParams;
mod client_playlists;
mod client_reporting;
#[doc(inline)]
pub use client_playlists::CastPlaybackInfo;
#[doc(inline)]
pub use client_reporting::ProgressReport;
mod client_sessions;

#[cfg(any(test, feature = "test"))]
pub mod test_support;

#[cfg(test)]
mod tests;
