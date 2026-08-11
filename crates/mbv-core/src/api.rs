use crate::id_types::{EmbySessionId, ItemId, MediaSourceId};

include!("api_types.rs");
include!("api_client_auth.rs");
include!("api_client_library.rs");
include!("api_client_reporting.rs");
include!("api_client_playlists.rs");
include!("api_client_sessions.rs");

#[cfg(test)]
mod tests {
    include!("api_tests.rs");
    include!("api_failure_tests.rs");
}
