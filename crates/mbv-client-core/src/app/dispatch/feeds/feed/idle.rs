//! The idle-feed link surface (`feed.rs`'s split tail): what the resting
//! playback panel paints while no queue plays, the fetch that fills it, and the
//! rotation that advances it. The item state lives on `App`; these calls read
//! and advance it.

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use mbv_feed::fetch_and_parse_rss;
use std::time::Instant;

impl App {
    /// Spawn a background thread to fetch and parse the idle RSS feed.
    ///
    /// # Panics
    ///
    /// Panics while reading the configuration when its lock is poisoned.
    pub(in crate::app) fn spawn_idle_feed_fetch(&self) {
        let Some(ref idle_feed) = self.idle_feed else {
            return;
        };
        let rss_url = self.config.lock().unwrap().idle_feed_rss_url.clone();
        let tx = idle_feed.items_tx.clone();
        std::thread::spawn(move || {
            let items = match fetch_and_parse_rss(&rss_url) {
                Ok(items) => items,
                Err(e) => {
                    tracing::warn!(name: "idle_feed.fetch.failed", target: "idle_feed", error = %e, "RSS feed fetch failed");
                    Vec::new()
                }
            };
            let _ = tx.send(items);
        });
    }

    pub fn idle_feed_link_available(&self) -> bool {
        self.idle_feed
            .as_ref()
            .and_then(|feed| feed.items.get(feed.current_index))
            .and_then(|item| item.link.as_deref())
            .is_some_and(|link| !link.is_empty())
    }

    /// Open the currently displayed idle-feed item in the user's browser.
    pub(in crate::app) fn open_idle_feed_link(&mut self) {
        let Some(link) = self
            .idle_feed
            .as_ref()
            .and_then(|feed| feed.items.get(feed.current_index))
            .and_then(|item| item.link.as_deref())
            .filter(|link| !link.is_empty())
            .map(str::to_owned)
        else {
            return;
        };

        if let Err(error) = crate::app::open_url(&link) {
            tracing::warn!(name: "idle_feed.link_open.failed", target: "idle_feed", { url.path = %mbv_core::applog::UrlPath(&link), error = %error }, "feed link open failed");
            self.flash(
                format!("Unable to open feed link: {error}"),
                ToastSeverity::Neutral,
            );
        }
    }

    /// Advance the idle feed rotation if enough time has elapsed.
    ///
    /// # Panics
    ///
    /// Panics while reading the configuration when its lock is poisoned.
    pub(in crate::app) fn advance_idle_feed_rotation(&mut self) {
        let Some(ref mut idle_feed) = self.idle_feed else {
            return;
        };
        if idle_feed.items.is_empty() {
            return;
        }
        let rotation_secs = self.config.lock().unwrap().idle_feed_rotation_secs;
        if idle_feed.last_rotation.elapsed() >= std::time::Duration::from_secs(rotation_secs) {
            idle_feed.current_index = (idle_feed.current_index + 1) % idle_feed.items.len();
            idle_feed.last_rotation = Instant::now();
        }
    }
}
