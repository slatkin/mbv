use super::feed_parse::{fetch_and_parse_entries, normalize_feed_url};
use super::notify_actions::ToastSeverity;
use super::types_feeds_manage::{
    FeedAddResult, FeedForm, FeedFormField, FeedsManagePopup, FeedsManageStage,
};
use super::types_tab_selection::TabSelection;
use super::{App, ConfirmAction, ConfirmModal};
use mbv_core::config::{FeedKind, FeedSubscription};

impl App {
    /// Opens the feeds management overlay (§6.1), from the `Manage feeds`
    /// F2 Settings row. Called from `render/overlays/settings.rs`, hence
    /// `pub(crate)` like its `open_multiselect_popup`/
    /// `open_library_routes_popup` siblings.
    pub(crate) fn open_feeds_manage_popup(&mut self) {
        self.feeds_manage_popup = Some(FeedsManagePopup::new());
    }

    pub(super) fn start_add_feed(&mut self) {
        if let Some(popup) = &mut self.feeds_manage_popup {
            popup.stage = FeedsManageStage::Form(FeedForm::new_add());
        }
    }

    pub(super) fn start_edit_feed(&mut self) {
        let Some(popup) = &self.feeds_manage_popup else {
            return;
        };
        let index = popup.cursor;
        let sub = self.client.lock().unwrap().config.feeds.get(index).cloned();
        let Some(sub) = sub else {
            return;
        };
        if let Some(popup) = &mut self.feeds_manage_popup {
            popup.stage = FeedsManageStage::Form(FeedForm::new_edit(index, &sub));
        }
    }

    pub(super) fn confirm_remove_feed(&mut self) {
        let Some(popup) = &self.feeds_manage_popup else {
            return;
        };
        let index = popup.cursor;
        let name = self
            .client
            .lock()
            .unwrap()
            .config
            .feeds
            .get(index)
            .map(|s| s.name.clone());
        let Some(name) = name else {
            return;
        };
        self.confirm_modal = Some(ConfirmModal {
            title: " Remove Feed ".into(),
            message: format!(
                "Remove subscription '{}'?",
                super::ui_util::trunc_str(&name, 40)
            ),
            hint: "[y] Confirm    [Esc] Cancel".into(),
            on_confirm: ConfirmAction::RemoveFeedSubscription(index),
        });
    }

    /// Effect for `ConfirmAction::RemoveFeedSubscription`'s "yes" answer
    /// (§6.3): rewrites `config.feeds` without the removed entry.
    pub(super) fn remove_feed_confirmed(&mut self, index: usize) {
        let feeds: Vec<FeedSubscription> = {
            let c = self.client.lock().unwrap();
            if index >= c.config.feeds.len() {
                return;
            }
            c.config
                .feeds
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, s)| s.clone())
                .collect()
        };
        self.persist_feeds(feeds);
    }

    /// Esc from the add/edit form (§6.2): discards unsaved input and, for
    /// an in-flight add, invalidates its pending id so the eventual fetch
    /// result is dropped as stale by `drain_feed_add_results`.
    pub(super) fn cancel_feed_form(&mut self) {
        if let Some(popup) = &mut self.feeds_manage_popup {
            popup.pending_add = None;
            popup.stage = FeedsManageStage::List;
        }
    }

    fn feed_form_mut(&mut self) -> Option<&mut FeedForm> {
        match &mut self.feeds_manage_popup {
            Some(popup) => match &mut popup.stage {
                FeedsManageStage::Form(form) => Some(form),
                FeedsManageStage::List => None,
            },
            None => None,
        }
    }

    /// Cycles focus among the form's fields. Edit mode (`editing_index`
    /// set) skips the read-only URL field (§6.3, design.md decision 10).
    pub(super) fn feed_form_next_field(&mut self) {
        let Some(form) = self.feed_form_mut() else {
            return;
        };
        let editing = form.editing_index.is_some();
        form.focus = match (form.focus, editing) {
            (FeedFormField::Name, true) => FeedFormField::Kind,
            (FeedFormField::Name, false) => FeedFormField::Url,
            (FeedFormField::Url, _) => FeedFormField::Kind,
            (FeedFormField::Kind, _) => FeedFormField::Name,
        };
    }

    pub(super) fn feed_form_prev_field(&mut self) {
        let Some(form) = self.feed_form_mut() else {
            return;
        };
        let editing = form.editing_index.is_some();
        form.focus = match (form.focus, editing) {
            (FeedFormField::Name, _) => FeedFormField::Kind,
            (FeedFormField::Url, _) => FeedFormField::Name,
            (FeedFormField::Kind, true) => FeedFormField::Name,
            (FeedFormField::Kind, false) => FeedFormField::Url,
        };
    }

    pub(super) fn toggle_feed_form_kind(&mut self) {
        let Some(form) = self.feed_form_mut() else {
            return;
        };
        form.kind = match form.kind {
            FeedKind::Audio => FeedKind::Video,
            FeedKind::Video => FeedKind::Audio,
        };
    }

    pub(super) fn feed_form_push_char(&mut self, c: char) {
        let Some(form) = self.feed_form_mut() else {
            return;
        };
        match form.focus {
            FeedFormField::Name => form.name.push(c),
            FeedFormField::Url if form.editing_index.is_none() => form.url.push(c),
            _ => {}
        }
    }

    pub(super) fn feed_form_backspace(&mut self) {
        let Some(form) = self.feed_form_mut() else {
            return;
        };
        match form.focus {
            FeedFormField::Name => {
                form.name.pop();
            }
            FeedFormField::Url if form.editing_index.is_none() => {
                form.url.pop();
            }
            _ => {}
        }
    }

    pub(super) fn submit_feed_form(&mut self) {
        let Some(popup) = &self.feeds_manage_popup else {
            return;
        };
        if popup.pending_add.is_some() {
            return;
        }
        let FeedsManageStage::Form(form) = popup.stage.clone() else {
            return;
        };
        match form.editing_index {
            Some(index) => self.submit_feed_edit(index, &form),
            None => self.submit_feed_add(&form),
        }
    }

    /// Edit changes only name and kind (§6.3) -- applied synchronously,
    /// unlike add, since the URL (and therefore feed validity) never
    /// changes here.
    fn submit_feed_edit(&mut self, index: usize, form: &FeedForm) {
        let name = form.name.trim().to_string();
        if name.is_empty() {
            self.flash("Feed name can't be empty".into(), ToastSeverity::Error);
            return;
        }
        let feeds: Vec<FeedSubscription> = {
            let c = self.client.lock().unwrap();
            c.config
                .feeds
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    if i == index {
                        FeedSubscription {
                            name: name.clone(),
                            url: s.url.clone(),
                            kind: form.kind,
                        }
                    } else {
                        s.clone()
                    }
                })
                .collect()
        };
        self.persist_feeds(feeds);
        if let Some(popup) = &mut self.feeds_manage_popup {
            popup.stage = FeedsManageStage::List;
        }
        self.flash(format!("Updated '{name}'"), ToastSeverity::Success);
    }

    /// Add fetches+parses the feed first (§6.2), off the UI thread; only a
    /// successful result -- matched against `pending_add` by id -- appends
    /// to `config.feeds` and persists. Failure surfaces via `flash` and
    /// does not save.
    fn submit_feed_add(&mut self, form: &FeedForm) {
        let name = form.name.trim().to_string();
        let url = form.url.trim().to_string();
        if name.is_empty() || url.is_empty() {
            self.flash(
                "Feed name and URL are required".into(),
                ToastSeverity::Error,
            );
            return;
        }
        let kind = form.kind;
        let Some(popup) = &mut self.feeds_manage_popup else {
            return;
        };
        let id = popup.next_add_id;
        popup.next_add_id += 1;
        popup.pending_add = Some(id);
        let tx = popup.add_tx.clone();
        std::thread::spawn(move || {
            let (resolved_url, result) = match normalize_feed_url(&url) {
                Ok(resolved_url) => {
                    let result = fetch_and_parse_entries(&resolved_url, kind).and_then(|entries| {
                        if entries.is_empty() {
                            Err("response did not contain any valid RSS or Atom entries"
                                .to_string())
                        } else {
                            Ok(())
                        }
                    });
                    (resolved_url, result)
                }
                Err(error) => (url.clone(), Err(error)),
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
    }

    /// Drain the in-flight add-feed fetch result, if any (§6.2). A result
    /// whose id no longer matches the popup's current `pending_add` -- the
    /// add was cancelled (Esc) or superseded by a later submission -- is
    /// discarded without touching config.
    pub(super) fn drain_feed_add_results(&mut self) -> bool {
        let Some(popup) = &self.feeds_manage_popup else {
            return false;
        };
        let Ok(result) = popup.add_rx.try_recv() else {
            return false;
        };
        let current_pending = self.feeds_manage_popup.as_ref().and_then(|p| p.pending_add);
        if current_pending != Some(result.id) {
            return true;
        }
        if let Some(popup) = &mut self.feeds_manage_popup {
            popup.pending_add = None;
        }
        match result.result {
            Ok(()) => {
                let feeds = {
                    let c = self.client.lock().unwrap();
                    let mut feeds = c.config.feeds.clone();
                    feeds.push(FeedSubscription {
                        name: result.name.clone(),
                        url: result.url,
                        kind: result.kind,
                    });
                    feeds
                };
                self.persist_feeds(feeds);
                self.flash(format!("Added '{}'", result.name), ToastSeverity::Success);
                if let Some(popup) = &mut self.feeds_manage_popup {
                    popup.stage = FeedsManageStage::List;
                }
            }
            Err(e) => {
                self.flash(format!("Couldn't add feed: {e}"), ToastSeverity::Error);
            }
        }
        true
    }

    /// Writes `feeds` into the client's config and persists it via the
    /// existing read-modify-write toml merge (§6.3), then runs the §6.4
    /// post-mutation resync.
    fn persist_feeds(&mut self, feeds: Vec<FeedSubscription>) {
        let cfg = {
            let mut c = self.client.lock().unwrap();
            c.config.feeds = feeds;
            c.config.clone()
        };
        if let Err(e) = crate::config::save_config_settings(&cfg) {
            log::warn!(target: "config", "config save failed: {e}");
            self.flash(
                format!("Feed change saved but config save failed ({e})"),
                ToastSeverity::Error,
            );
        }
        self.after_feeds_mutation();
    }

    /// After every subscription mutation (§6.4): resync `feed_tab` from
    /// config, clear its fetched entry data (no auto-fetch), clamp
    /// group/cursor/scroll, and fall back to Home if the last subscription
    /// was removed while Feeds is selected.
    pub(super) fn after_feeds_mutation(&mut self) {
        self.sync_feed_subscriptions();
        let n = self.feed_tab.subscriptions.len();
        self.feed_tab.entries = vec![Vec::new(); n];
        self.feed_tab.all_entries.clear();
        self.feed_tab.selected_group = self
            .feed_tab
            .selected_group
            .min(self.feed_tab.group_count() - 1);
        self.feed_tab.clamp_state();
        if n == 0 && self.tab.is_feeds() {
            self.tab = TabSelection::Home;
        }
        if let Some(popup) = &mut self.feeds_manage_popup {
            popup.cursor = if n == 0 { 0 } else { popup.cursor.min(n - 1) };
        }
    }
}
