//! The Service startup step every Client runs once after its attach
//! (add-gui-library-panel design D11): the two Remote Service startup workers,
//! the startup feed fetch, and capability registration. The GUI Client calls
//! it in place of the deleted minimal Emby client fill; the TUI Client calls
//! it from `Model::spawn_startup_services` at the same point it always did.

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::session::service_startup;
use crate::app::state::playback::HomeContent;
use mbv_ui_model::feed::IdleFeed;
use std::time::Instant;

impl App {
    /// Start the configured Remote Service workers, kick off the startup feed
    /// fetch, and register client capabilities (design D11). Both Clients run
    /// this once after the attach. The selected Player owner and UI never wait
    /// for Emby setup, authentication, or connectivity.
    ///
    /// # Panics
    ///
    /// Panics if the installed Emby client's mutex is poisoned.
    pub fn start_client_services(&mut self) {
        if let Some(request) = self.setup.emby_startup_request.take() {
            self.setup.emby_startup_rx =
                Some(service_startup::start(request.config, request.generation));
        }
        if let Some(request) = self.setup.audiobookshelf_startup_request.take() {
            self.setup.audiobookshelf_startup_rx = Some(service_startup::start_audiobookshelf(
                request.config,
                request.generation,
                service_startup::AudiobookshelfCompletionKind::Startup,
            ));
        }

        // Auto-fetch configured feeds asynchronously so the Feeds tab and the
        // Home "Feeds" pill are populated shortly after startup instead of
        // staying empty until the user presses the manual refresh key.
        self.start_feed_fetch();

        if let Some(client) = self.emby_client() {
            client.lock().unwrap().register_capabilities();
        }

        // Create the idle-feed channel pair and start its first fetch when an
        // RSS URL is configured (row 6.1): the feed is Client-neutral data the
        // drains read, so the shared step initializes it for both Clients.
        self.init_idle_feed();
    }

    fn init_idle_feed(&mut self) {
        // Initialize idle feed if configured
        if self.config.lock().unwrap().idle_feed_rss_url.is_empty() {
            // No RSS URL configured, skip idle feed
        } else {
            let (items_tx, items_rx) = std::sync::mpsc::channel();
            self.idle_feed = Some(IdleFeed {
                items: Vec::new(),
                current_index: 0,
                last_rotation: Instant::now(),
                last_fetch: Instant::now(),
                items_tx,
                items_rx,
            });
            self.spawn_idle_feed_fetch();
        }
    }

    /// The `App` half of the launch Home load (design D11): fetch Home, and on
    /// success re-arm the Emby `Recently added` fetch and clear a stale status
    /// row, or on failure flash the "Couldn't load home" toast. The Client
    /// that presents Home keeps its own half — the loading flag and the
    /// content assignment. A launch with a cached Emby client brings the
    /// Service to Ready at construction, so the bootstrap completion's seam
    /// never runs on that path; this is that launch's one Home load, with the
    /// eligible libraries already known, so it arms the fetch here (spec
    /// `home-latest-sections`, "Recently added loads at launch independent of
    /// destinations": "when that Service becomes ready").
    pub fn load_home_at_startup(&mut self) -> Option<HomeContent> {
        match self.fetch_home() {
            Ok(content) => {
                // The App stores the snapshot (row 6.1): the presenting
                // Client's shell assignment and a GUI Home projection read
                // the same facts.
                self.home_content = content.clone();
                self.spawn_home_recently_added(mbv_queue::ServiceKind::Emby);
                if !self.status_toast_live() {
                    self.status.clear();
                }
                Some(content)
            }
            Err(error) => {
                self.flash(
                    format!("Couldn't load home: {error}"),
                    ToastSeverity::Warning,
                );
                None
            }
        }
    }

    /// Start the playback-target discovery a ready Emby client feeds
    /// (design D11): the `/Sessions` fetch behind the session list and the
    /// cast receiver browse. The full Emby startup starts neither, so the GUI
    /// Client runs this for a client the attach brought and for a startup its
    /// drain reports.
    pub fn start_playback_target_discovery(&mut self) {
        self.spawn_sessions_load();
        self.spawn_cast_discovery();
    }
}
