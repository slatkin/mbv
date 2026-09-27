//! Service / setup request type. Split from `msg.rs` (task 8.3) to keep the
//! central `Msg` file below the 800-line cap.

// TODO(migrate-tui-to-tuirealm): flesh out as service-driven surfaces convert
// (browse fetch / search / session / cast ops; tasks 3.x/4.x).
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceRequest {
    /// Dispatch a debounced search query to the Emby client. The shell owns
    /// the Emby client and spawns the search thread (task 3.2).
    SearchQuery(String),
    ActivateService(usize),
    RemoveEmby,
    TestAudiobookshelfConnection,
    ReplaceAudiobookshelf,
    RemoveAudiobookshelf,
    SubmitEmbySetup {
        server_url: String,
        username: String,
        password: String,
    },
    SubmitAudiobookshelfSetup {
        server_url: String,
        api_key: String,
    },
    CancelSetup,
}
