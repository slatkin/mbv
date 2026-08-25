//! Shell-owned feeds-management popup effects (tasks 5.3c / 5.3d).
//!
//! The `FeedsManageComponent` owns the interaction state (stage/cursor/form
//! edits): the two-way per-tick mirror (`sync_feeds_manage` /
//! `sync_feeds_manage_to_app`) is deleted. This module owns only the effects
//! the component's key forwarding cannot perform — network fetch, confirm
//! dialog, config persistence — and the background add-feed channel that
//! cannot live in the component (`Model::feeds_manage`).

use super::components::{ComponentId, FeedsManageComponent, PopupId};
use super::types_feeds_manage::{
    FeedAddResult, FeedForm, FeedsManagePopup, FeedsManageStage,
};
use super::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::{FeedKind, FeedSubscription};

impl App {
    /// Opens the feeds management overlay (§6.1), from the `Manage feeds`
    /// Settings row. Now a shell handoff target (task 5.3c): the shell mounts
    /// the component and seeds it from `Model::feeds_manage`.
    pub(crate) fn open_feeds_manage_popup(&mut self) {
        self.pending_overlay = Some(super::types_overlay::OverlayRequest::OpenFeedsManage);
    }
}

impl super::shell::Model {
    /// Mount the feeds-management component and seed it (task 5.3c/5.3d).
    /// The component owns the stage/cursor; `Model::feeds_manage` carries
    /// only the background add-feed channel, the pending-add marker and the
    /// add-attempt id counter.
    pub(in crate::app) fn open_feeds_manage(&mut self) {
        if self.feeds_manage.is_none() {
            self.feeds_manage = Some(FeedsManagePopup::new());
        }
        let id = ComponentId::Popup(PopupId::FeedManage);
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(FeedsManageComponent::new()), vec![])
                .expect("mount FeedManage");
            self.application.active(&id).expect("activate FeedManage");
        }
        self.push_feeds_manage_content();
        // A fresh component starts with no stage; open the popup at the List
        // stage (task 5.3d — no per-tick stage mirror).
        if let Some(component) = self.feeds_manage_component_mut() {
            if component.stage_clone().is_none() {
                component.set_stage(FeedsManageStage::List);
            }
        }
    }

    /// Push the shell-owned content the component paints (config feeds and
    /// the pending-add marker). Stage/cursor/form edits stay component-local.
    fn push_feeds_manage_content(&mut self) {
        let feeds = self.app.config.lock().unwrap().feeds.clone();
        let pending = self.feeds_manage.as_ref().and_then(|p| p.pending_add);
        if let Some(component) = self.feeds_manage_component_mut() {
            component.set_feeds(feeds);
            component.set_pending_add(pending);
        }
    }

    fn feeds_manage_component_mut(&mut self) -> Option<&mut FeedsManageComponent> {
        let id = ComponentId::Popup(PopupId::FeedManage);
        if !self.application.mounted(&id) {
            return None;
        }
        self.application
            .get_component_mut(&id)
            .and_then(|component| component.as_any_mut().downcast_mut::<FeedsManageComponent>())
    }

    fn feeds_manage_stage(&mut self) -> Option<FeedsManageStage> {
        self.feeds_manage_component_mut()?.stage_clone()
    }

    fn feeds_manage_cursor(&mut self) -> usize {
        self.feeds_manage_component_mut()
            .map(|component| component.cursor())
            .unwrap_or(0)
    }

    /// Route a forwarded feeds-management key against the component's live
    /// stage (task 5.3d — no `sync_feeds_manage_to_app` mirror first).
    pub(in crate::app) fn handle_feeds_manage_key(&mut self, key: KeyEvent) {
        let Some(stage) = self.feeds_manage_stage() else {
            return;
        };
        match stage {
            FeedsManageStage::List => self.handle_feeds_manage_list_key(key),
            FeedsManageStage::Form(form) => self.handle_feeds_manage_form_key(key, form),
        }
    }

    fn handle_feeds_manage_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.dismiss_feeds_manage(),
            KeyCode::Char('a') => self.start_add_feed(),
            KeyCode::Enter | KeyCode::Char('e') if self.config_feed_count() > 0 => {
                self.start_edit_feed()
            }
            KeyCode::Char('d') if self.config_feed_count() > 0 => self.confirm_remove_feed(),
            _ => {}
        }
    }

    fn handle_feeds_manage_form_key(&mut self, key: KeyEvent, _form: FeedForm) {
        let submitting = self
            .feeds_manage
            .as_ref()
            .is_some_and(|p| p.pending_add.is_some());
        match key.code {
            KeyCode::Esc => self.cancel_feed_form(),
            KeyCode::Enter if !submitting => self.submit_feed_form(),
            _ => {}
        }
    }

    fn config_feed_count(&self) -> usize {
        self.app.config.lock().unwrap().feeds.len()
    }

    fn dismiss_feeds_manage(&mut self) {
        let id = ComponentId::Popup(PopupId::FeedManage);
        if self.application.mounted(&id) {
            let _ = self.application.umount(&id);
        }
        self.feeds_manage = None;
    }

    fn start_add_feed(&mut self) {
        if let Some(component) = self.feeds_manage_component_mut() {
            component.set_stage(FeedsManageStage::Form(FeedForm::new_add()));
        }
    }

    fn start_edit_feed(&mut self) {
        let index = self.feeds_manage_cursor();
        let sub = self.app.config.lock().unwrap().feeds.get(index).cloned();
        let Some(sub) = sub else {
            return;
        };
        if let Some(component) = self.feeds_manage_component_mut() {
            component.set_stage(FeedsManageStage::Form(FeedForm::new_edit(index, &sub)));
        }
    }

    fn confirm_remove_feed(&mut self) {
        let index = self.feeds_manage_cursor();
        let name = self
            .app
            .config
            .lock()
            .unwrap()
            .feeds
            .get(index)
            .map(|s| s.name.clone());
        let Some(name) = name else {
            return;
        };
        self.app.ask_confirm(super::ConfirmModal {
            title: " Remove Feed ".into(),
            message: format!(
                "Remove subscription '{}'?",
                super::ui_util::trunc_str(&name, 40)
            ),
            hint: "[y] Confirm    [Esc] Cancel".into(),
            on_confirm: super::ConfirmAction::RemoveFeedSubscription(index),
        });
    }

    /// Esc from the add/edit form (§6.2): discards unsaved input and, for
    /// an in-flight add, invalidates its pending id so the eventual fetch
    /// result is dropped as stale by `drain_feed_add_results`.
    pub(in crate::app) fn cancel_feed_form(&mut self) {
        if let Some(popup) = &mut self.feeds_manage {
            popup.pending_add = None;
        }
        if let Some(component) = self.feeds_manage_component_mut() {
            component.set_pending_add(None);
            component.set_stage(FeedsManageStage::List);
        }
    }

    pub(in crate::app) fn submit_feed_form(&mut self) {
        if self
            .feeds_manage
            .as_ref()
            .is_some_and(|p| p.pending_add.is_some())
        {
            return;
        }
        let Some(form) = self.feeds_manage_stage().and_then(|stage| match stage {
            FeedsManageStage::Form(form) => Some(form),
            FeedsManageStage::List => None,
        }) else {
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
            self.app.flash(
                "Feed name can't be empty".into(),
                super::notify_actions::ToastSeverity::Error,
            );
            return;
        }
        let feeds: Vec<FeedSubscription> = {
            let c = self.app.config.lock().unwrap();
            c.feeds
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
        self.app.persist_feeds(feeds.clone());
        if let Some(component) = self.feeds_manage_component_mut() {
            component.set_stage(FeedsManageStage::List);
            component.set_feeds(feeds);
        }
        self.app.flash(
            format!("Updated '{name}'"),
            super::notify_actions::ToastSeverity::Success,
        );
    }

    /// Add fetches+parses the feed first (§6.2), off the UI thread; only a
    /// successful result -- matched against `pending_add` by id -- appends
    /// to `config.feeds` and persists. Failure surfaces via `flash` and
    /// does not save.
    fn submit_feed_add(&mut self, form: &FeedForm) {
        let name = form.name.trim().to_string();
        let url = form.url.trim().to_string();
        if name.is_empty() || url.is_empty() {
            self.app.flash(
                "Feed name and URL are required".into(),
                super::notify_actions::ToastSeverity::Error,
            );
            return;
        }
        let kind = form.kind;
        let Some(popup) = &mut self.feeds_manage else {
            return;
        };
        let id = popup.next_add_id;
        popup.next_add_id += 1;
        popup.pending_add = Some(id);
        let tx = popup.add_tx.clone();
        std::thread::spawn(move || {
            let (resolved_url, result) = match super::feed_parse::normalize_feed_url(&url) {
                Ok(resolved_url) => {
                    let result = super::feed_parse::fetch_and_parse_entries(
                        &resolved_url,
                        kind,
                        &resolved_url,
                    )
                    .and_then(|entries| {
                        if entries.is_empty() {
                            Err("response did not contain any valid RSS or Atom entries"
                                .to_string())
                        } else {
                            Ok(())
                        }
                    });
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
        if let Some(component) = self.feeds_manage_component_mut() {
            component.set_pending_add(Some(id));
        }
        self.app.flash(
            "Fetching feed…".into(),
            super::notify_actions::ToastSeverity::Neutral,
        );
    }

    /// Drain the in-flight add-feed fetch result, if any (§6.2). A result
    /// whose id no longer matches the popup's current `pending_add` -- the
    /// add was cancelled (Esc) or superseded by a later submission -- is
    /// discarded without touching config.
    pub(in crate::app) fn drain_feed_add_results(&mut self) -> bool {
        let Some(popup) = &self.feeds_manage else {
            return false;
        };
        let Ok(result) = popup.add_rx.try_recv() else {
            return false;
        };
        let current_pending = self.feeds_manage.as_ref().and_then(|p| p.pending_add);
        if current_pending != Some(result.id) {
            return true;
        }
        if let Some(popup) = &mut self.feeds_manage {
            popup.pending_add = None;
        }
        match result.result {
            Ok(()) => {
                let feeds = {
                    let c = self.app.config.lock().unwrap();
                    let mut feeds = c.feeds.clone();
                    feeds.push(FeedSubscription {
                        name: result.name.clone(),
                        url: result.url,
                        kind: result.kind,
                    });
                    feeds
                };
                self.app.persist_feeds(feeds.clone());
                self.app.flash(
                    format!("Added '{}'", result.name),
                    super::notify_actions::ToastSeverity::Success,
                );
                if let Some(component) = self.feeds_manage_component_mut() {
                    component.set_stage(FeedsManageStage::List);
                    component.set_feeds(feeds);
                    component.set_pending_add(None);
                }
            }
            Err(e) => {
                self.app.flash(
                    format!("Couldn't add feed: {e}"),
                    super::notify_actions::ToastSeverity::Error,
                );
                if let Some(component) = self.feeds_manage_component_mut() {
                    component.set_pending_add(None);
                }
            }
        }
        true
    }
}