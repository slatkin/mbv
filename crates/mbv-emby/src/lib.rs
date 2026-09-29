mod error;
pub use error::EmbyError;
mod failure;
pub use failure::{EmbyBootstrap, EmbyFailure, EmbyFailureClass};
mod types;
pub(crate) use types::load_cached_token;
#[cfg(test)]
pub use types::save_cached_token;
pub use types::{
    EmbyClient, MBV_DIRECT_TCP_PORT_PREFIX, PlaybackInfo, SessionAudioStream, SessionInfo,
    SessionMediaInfo, SessionSubtitleStream, clear_cached_token, device_id, device_name,
    gen_session_id, mbv_direct_tcp_port_command, parse_audio_info, parse_item,
    parse_mbv_direct_tcp_port, parse_session_media_info, parse_video_info,
};
mod client_auth;
mod types_parsing;
pub use client_auth::EmbyCredentialExchange;
mod client_library;
pub use client_library::SortedItemsParams;
mod client_playlists;
mod client_reporting;
pub use client_playlists::CastPlaybackInfo;
pub use client_reporting::ProgressReport;
mod client_sessions;

#[cfg(any(test, feature = "test"))]
pub mod test_support;

#[cfg(test)]
mod tests;
