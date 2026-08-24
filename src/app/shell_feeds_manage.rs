//! Shell-owned feeds-management popup behaviour (task 5.3c).
//!
//! The `FeedsManageComponent` owns the interaction state (stage/cursor/form
//! edits) and mirrors `stage`/`cursor`/`feeds`/`pending_add` from
//! `Model::feeds_manage` each tick. This module owns the effect dispatch the
//! component's key forwarding cannot perform (network fetch, confirm dialog,
//! config persistence) and the background add-feed channel that cannot live
//! in the component.

use super::components::FeedsManageComponent;
use super::types_feeds_manage::{
    FeedAddResult, FeedForm, FeedFormField, FeedsManagePopup, FeedsManageStage,
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
    /// Mount the feeds-management component and seed it from a fresh
    /// `FeedsManagePopup` (task 5.3c). The component owns the stage/cursor;
    /// `Model::feeds_manage` carries the background add-feed channel.
    pub(in crate::app) fn open_feeds_manage(&mut self) {
        if self.feeds_manage.is_none() {
            self.feeds_manage = Some(FeedsManagePopup::new());
        }
        let id = super::components::ComponentId::Popup(super::components::PopupId::FeedManage);
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(FeedsManageComponent::new()), vec![])
                .expect("mount FeedManage");
            self.application.active(&id).expect("activate FeedManage");
        }
        self.sync_feeds_manage();
    }

    /// Route a forwarded feeds-management key after mirroring the component's
    /// live draft into `Model::feeds_manage` (task 5.3c).
    pub(in crate::app) fn handle_feeds_manage_key(&mut self, key: KeyEvent) {
        self.sync_feeds_manage_to_app();
        let Some(popup) = self.feeds_manage.as_ref() else {
            return;
        };
        let stage = popup.stage.clone();
        match stage {
            FeedsManageStage::List => self.handle_feeds_manage_list_key(key),
            FeedsManageStage::Form(form) => self.handle_feeds_manage_form_key(key, form),
        }
        self.sync_feeds_manage();
    }

    fn handle_feeds_manage_list_key(&mut self, key: KeyEvent) {
        let count = self.app.config.lock().unwrap().feeds.len();
        match key.code {
            KeyCode::Esc => self.dismiss_feeds_manage(),
            KeyCode::Up => {
                if let Some(popup) = &mut self.feeds_manage {
                    popup.cursor = popup.cursor.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if count > 0 {
                    if let Some(popup) = &mut self.feeds_manage {
                        popup.cursor = (popup.cursor + 1).min(count - 1);
                    }
                }
            }
            KeyCode::Char('a') => self.start_add_feed(),
            KeyCode::Enter | KeyCode::Char('e') if count > 0 => self.start_edit_feed(),
            KeyCode::Char('d') if count > 0 => self.confirm_remove_feed(),
            _ => {}
        }
    }

    fn handle_feeds_manage_form_key(&mut self, key: KeyEvent, form: FeedForm) {
        let submitting = self
            .feeds_manage
            .as_ref()
            .map(|p| p.pending_add.is_some())
            .unwrap_or(false);
        match key.code {
            KeyCode::Esc => self.cancel_feed_form(),
            KeyCode::Tab if !submitting => self.feed_form_next_field(),
            KeyCode::BackTab if !submitting => self.feed_form_prev_field(),
            KeyCode::Enter if !submitting => self.submit_feed_form(),
            KeyCode::Left | KeyCode::Right if !submitting && form.focus == FeedFormField::Kind => {
                self.toggle_feed_form_kind();
            }
            KeyCode::Backspace if !submitting => self.feed_form_backspace(),
            KeyCode::Char(c)
                if !submitting
                    && (key.modifiers == KeyModifiers::NONE
                        || key.modifiers == KeyModifiers::SHIFT) =>
            {
                self.feed_form_push_char(c);
            }
            _ => {}
        }
    }

    fn dismiss_feeds_manage(&mut self) {
        let id = super::components::ComponentId::Popup(super::components::PopupId::FeedManage);
        if self.application.mounted(&id) {
            let _ = self.application.umount(&id);
        }
        self.feeds_manage = None;
    }

    fn start_add_feed(&mut self) {
        if let Some(popup) = &mut self.feeds_manage {
            popup.stage = FeedsManageStage::Form(FeedForm::new_add());
        }
    }

    fn start_edit_feed(&mut self) {
        let Some(popup) = &self.feeds_manage else {
            return;
        };
        let index = popup.cursor;
        let sub = self.app.config.lock().unwrap().feeds.get(index).cloned();
        let Some(sub) = sub else {
            return;
        };
        if let Some(popup) = &mut self.feeds_manage {
            popup.stage = FeedsManageStage::Form(FeedForm::new_edit(index, &sub));
        }
    }

    fn confirm_remove_feed(&mut self) {
        let Some(popup) = &self.feeds_manage else {
            return;
        };
        let index = popup.cursor;
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
            popup.stage = FeedsManageStage::List;
        }
    }

    fn feed_form_mut(&mut self) -> Option<&mut FeedForm> {
        match &mut self.feeds_manage {
            Some(popup) => match &mut popup.stage {
                FeedsManageStage::Form(form) => Some(form),
                FeedsManageStage::List => None,
            },
            None => None,
        }
    }

    /// Cycles focus among the form's fields. Edit mode (`editing_index`
    /// set) skips the read-only URL field (§6.3, design.md decision 10).
    fn feed_form_next_field(&mut self) {
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

    fn feed_form_prev_field(&mut self) {
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

    fn toggle_feed_form_kind(&mut self) {
        let Some(form) = self.feed_form_mut() else {
            return;
        };
        form.kind = match form.kind {
            FeedKind::Audio => FeedKind::Video,
            FeedKind::Video => FeedKind::Audio,
        };
    }

    fn feed_form_push_char(&mut self, c: char) {
        let Some(form) = self.feed_form_mut() else {
            return;
        };
        match form.focus {
            FeedFormField::Name => form.name.push(c),
            FeedFormField::Url if form.editing_index.is_none() => form.url.push(c),
            _ => {}
        }
    }

    fn feed_form_backspace(&mut self) {
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

    pub(in crate::app) fn submit_feed_form(&mut self) {
        let Some(popup) = &self.feeds_manage else {
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
        self.app.persist_feeds(feeds);
        if let Some(popup) = &mut self.feeds_manage {
            popup.stage = FeedsManageStage::List;
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
                self.app.persist_feeds(feeds);
                self.app.flash(
                    format!("Added '{}'", result.name),
                    super::notify_actions::ToastSeverity::Success,
                );
                if let Some(popup) = &mut self.feeds_manage {
                    popup.stage = FeedsManageStage::List;
                }
            }
            Err(e) => {
                self.app.flash(
                    format!("Couldn't add feed: {e}"),
                    super::notify_actions::ToastSeverity::Error,
                );
            }
        }
        true
    }
}
