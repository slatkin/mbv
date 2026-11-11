use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use mbv_config::FeedSubscription;
use mbv_queue::FeedKind;
use mbv_ui_model::feeds_manage::{FeedAddDrain, FeedAddResult};
use mbv_ui_model::tab_selection::TabSelection;

fn require_feed_entries<T>(
    result: Result<Vec<T>, mbv_ui_model::UiModelError>,
) -> Result<(), mbv_ui_model::UiModelError> {
    result.and_then(|entries| {
        if entries.is_empty() {
            Err(mbv_ui_model::UiModelError::operation(
                "response did not contain any valid RSS or Atom entries",
            ))
        } else {
            Ok(())
        }
    })
}

impl App {
    /// Start a background add-feed fetch+parse (§6.2), off the UI thread;
    /// only a successful result -- matched against the pending id by the
    /// drain -- appends to `config.feeds` and persists. Failure surfaces via
    /// the drain. Returns the pending id when the fetch started, `None` for
    /// an in-flight add (one at a time), a rejected form, or no open feed
    /// state.
    pub(in crate::app) fn begin_feed_add(
        &mut self,
        name: &str,
        url: &str,
        kind: FeedKind,
    ) -> Option<u64> {
        if self
            .feeds_manage
            .as_ref()
            .is_some_and(|p| p.pending_add.is_some())
        {
            return None;
        }
        let name = name.trim().to_string();
        let url = url.trim().to_string();
        if name.is_empty() || url.is_empty() {
            self.flash(
                "Feed name and URL are required".into(),
                ToastSeverity::Error,
            );
            return None;
        }
        let Some(popup) = &mut self.feeds_manage else {
            return None;
        };
        let id = popup.next_add_id;
        popup.next_add_id += 1;
        popup.pending_add = Some(id);
        let tx = popup.add_tx.clone();
        std::thread::spawn(move || {
            let (resolved_url, result) = match mbv_feed::normalize_feed_url(&url)
                .map_err(mbv_ui_model::UiModelError::operation_source)
            {
                Ok(resolved_url) => {
                    let result = require_feed_entries(
                        mbv_feed::fetch_and_parse_entries(&resolved_url, kind, &resolved_url)
                            .map_err(mbv_ui_model::UiModelError::operation_source),
                    );
                    (resolved_url, result)
                }
                Err(error) => (url, Err(error)),
            };
            let _ = tx.send(FeedAddResult {
                id,
                name,
                url: resolved_url,
                kind,
                result,
            });
        });
        self.flash("Fetching feed…".into(), ToastSeverity::Neutral);
        Some(id)
    }

    /// Edit changes only name and kind (§6.3) -- applied synchronously,
    /// unlike add, since the URL (and therefore feed validity) never
    /// changes here. The subscription is addressed by its URL, the config
    /// list's only stable identity. Returns whether the edit applied.
    pub(in crate::app) fn edit_feed(&mut self, url: &str, name: &str, kind: FeedKind) -> bool {
        let name = name.trim().to_string();
        if name.is_empty() {
            self.flash("Feed name can't be empty".into(), ToastSeverity::Error);
            return false;
        }
        let feeds: Vec<FeedSubscription> = {
            let c = self.config.lock().unwrap();
            let mut found = false;
            let feeds = c
                .feeds
                .iter()
                .map(|s| {
                    if s.url == url {
                        found = true;
                        FeedSubscription {
                            name: name.clone(),
                            url: s.url.clone(),
                            kind,
                        }
                    } else {
                        s.clone()
                    }
                })
                .collect();
            if !found {
                return false;
            }
            feeds
        };
        self.persist_feeds(feeds);
        self.flash(format!("Updated '{name}'"), ToastSeverity::Success);
        true
    }

    /// Raise the remove confirmation for the subscription at `url` (§6.3),
    /// addressed by the config list's only stable identity.
    pub(in crate::app) fn request_feed_removal(&mut self, url: &str) {
        let (index, name) = {
            let c = self.config.lock().unwrap();
            match c.feeds.iter().enumerate().find(|(_, s)| s.url == url) {
                Some((index, sub)) => (index, sub.name.clone()),
                None => return,
            }
        };
        self.ask_confirm(crate::app::ConfirmModal::two_button(
            format!(
                "Remove subscription '{}'?",
                mbv_ui_model::ui_util::trunc_str(&name, 40)
            ),
            "Confirm",
            "Cancel",
            crate::app::ConfirmAction::RemoveFeedSubscription(index),
        ));
    }

    /// Drain the in-flight add-feed fetch result, if any (§6.2). A result
    /// whose id no longer matches the current pending id -- the add was
    /// cancelled (Esc) or superseded by a later submission -- is discarded
    /// without touching config.
    pub(in crate::app) fn drain_feed_add_results(&mut self) -> FeedAddDrain {
        let Some(popup) = &self.feeds_manage else {
            return FeedAddDrain::Idle;
        };
        let Ok(result) = popup.add_rx.try_recv() else {
            return FeedAddDrain::Idle;
        };
        let current_pending = self.feeds_manage.as_ref().and_then(|p| p.pending_add);
        if current_pending != Some(result.id) {
            return FeedAddDrain::Discarded;
        }
        if let Some(popup) = &mut self.feeds_manage {
            popup.pending_add = None;
        }
        match result.result {
            Ok(()) => {
                let feeds = {
                    let c = self.config.lock().unwrap();
                    let mut feeds = c.feeds.clone();
                    feeds.push(FeedSubscription {
                        name: result.name.clone(),
                        url: result.url,
                        kind: result.kind,
                    });
                    feeds
                };
                self.persist_feeds(feeds.clone());
                self.flash(format!("Added '{}'", result.name), ToastSeverity::Success);
                FeedAddDrain::Applied(feeds)
            }
            Err(e) => {
                self.flash(format!("Couldn't add feed: {e}"), ToastSeverity::Error);
                FeedAddDrain::Failed
            }
        }
    }

    /// Effect for `ConfirmAction::RemoveFeedSubscription`'s accept answer
    /// (§6.3): rewrites `config.feeds` without the removed entry.
    pub(in crate::app) fn remove_feed_confirmed(&mut self, index: usize) {
        let feeds: Vec<FeedSubscription> = {
            let c = self.config.lock().unwrap();
            if index >= c.feeds.len() {
                return;
            }
            c.feeds
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, s)| s.clone())
                .collect()
        };
        self.persist_feeds(feeds);
    }

    /// Writes `feeds` into App's general config and persists it via the
    /// existing read-modify-write toml merge (§6.3), then runs the §6.4
    /// post-mutation resync.
    pub(in crate::app) fn persist_feeds(&mut self, feeds: Vec<FeedSubscription>) {
        let cfg = {
            let mut c = self.config.lock().unwrap();
            c.feeds = feeds;
            c.clone()
        };
        if let Err(e) = crate::config::save_config_section(&cfg, mbv_config::ConfigSection::Feeds) {
            tracing::warn!(name: "config.save.failed", target: "config", error = %e, "config save failed");
            self.flash(
                format!("Feed change saved but config save failed ({e})"),
                ToastSeverity::Error,
            );
        }
        self.after_feeds_mutation();
    }

    /// After every subscription mutation (§6.4): resync shell-owned Feed
    /// data, clear fetched entries (no auto-fetch), and fall back to Home if
    /// the last subscription was removed while Feeds is selected. The Feeds
    /// content owner resets its local selection when the subscription
    /// identity changes during the next shell sync.
    pub(in crate::app) fn after_feeds_mutation(&mut self) {
        self.sync_feed_subscriptions();
        let n = self.feed_tab.subscriptions.len();
        self.feed_tab.entries = vec![Vec::new(); n];
        self.feed_tab.all_entries.clear();
        if n == 0 && self.tab.is_feeds() {
            self.tab = TabSelection::Home;
        }
    }
}
