//! Shell-owned feeds-management popup effects (tasks 5.3c / 5.3d).
//!
//! The `FeedsManageComponent` owns the interaction state (stage/cursor/form
//! edits): the two-way per-tick mirror (`sync_feeds_manage` /
//! `sync_feeds_manage_to_app`) is deleted. The fetch, confirm, config
//! persistence, and background add-feed channel effects live on `App`
//! (add-gui-settings-screen design D7, `App.feeds_manage` and its
//! `begin_feed_add`/`edit_feed`/`request_feed_removal`/
//! `drain_feed_add_results`); this module owns only the component mounting
//! and the projection of the App-owned outcomes onto the component.

use mbv_components::FeedsManageComponent;
use mbv_ui_model::feeds_manage::{FeedAddDrain, FeedForm, FeedsManageStage};
use mbv_ui_msg::FeedsManageIntent;
use mbv_ui_msg::{ComponentId, PopupId};

impl super::Model {
    /// Mount the feeds-management component and seed it (task 5.3c/5.3d).
    /// The component owns the stage/cursor; `App.feeds_manage` carries
    /// only the background add-feed channel, the pending-add marker and the
    /// add-attempt id counter.
    pub(in crate::app) fn open_feeds_manage(&mut self) {
        if self.app.feeds_manage.is_none() {
            self.app.feeds_manage = Some(mbv_ui_model::feeds_manage::FeedsManagePopup::new());
        }
        let id = ComponentId::Popup(PopupId::FeedManage);
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(FeedsManageComponent::new()), vec![])
                .expect("mount FeedManage");
        }
        // Focus routes through the core focus stack (design D3 of
        // `extract-client-core`): the push takes focus over the Settings
        // sidebar beneath it.
        self.push_focus_overlay(&id);
        self.push_feeds_manage_content();
        // A fresh component starts with no stage; open the popup at the List
        // stage (task 5.3d — no per-tick stage mirror).
        if let Some(component) = self.feeds_manage_component_mut()
            && component.stage_clone().is_none()
        {
            component.set_stage(FeedsManageStage::List);
        }
    }

    /// Push the shell-owned content the component paints (config feeds and
    /// the pending-add marker). Stage/cursor/form edits stay component-local.
    fn push_feeds_manage_content(&mut self) {
        let feeds = self.app.config.lock().unwrap().feeds.clone();
        let pending = self.app.feeds_manage.as_ref().and_then(|p| p.pending_add);
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
            .and_then(|component| {
                component
                    .as_any_mut()
                    .downcast_mut::<FeedsManageComponent>()
            })
    }

    fn feeds_manage_stage(&mut self) -> Option<FeedsManageStage> {
        self.feeds_manage_component_mut()?.stage_clone()
    }

    fn feeds_manage_cursor(&mut self) -> usize {
        self.feeds_manage_component_mut()
            .map_or(0, |component| component.cursor())
    }

    /// Route a semantic feeds-management intent to the existing shell effects.
    pub(in crate::app) fn handle_feeds_manage_intent(&mut self, intent: FeedsManageIntent) {
        match intent {
            FeedsManageIntent::Dismiss => self.dismiss_feeds_manage(),
            FeedsManageIntent::Add => self.start_add_feed(),
            FeedsManageIntent::Edit => {
                if self.config_feed_count() > 0 {
                    self.start_edit_feed();
                }
            }
            FeedsManageIntent::Remove => {
                if self.config_feed_count() > 0 {
                    self.confirm_remove_feed();
                }
            }
            FeedsManageIntent::Cancel => self.cancel_feed_form(),
            FeedsManageIntent::Submit => self.submit_feed_form(),
        }
    }

    fn config_feed_count(&self) -> usize {
        self.app.config.lock().unwrap().feeds.len()
    }

    pub(in crate::app) fn dismiss_feeds_manage(&mut self) {
        let id = ComponentId::Popup(PopupId::FeedManage);
        self.dismiss_focus_overlay(&id);
        self.app.feeds_manage = None;
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
        let url = self
            .app
            .config
            .lock()
            .unwrap()
            .feeds
            .get(index)
            .map(|s| s.url.clone());
        let Some(url) = url else {
            return;
        };
        self.app.request_feed_removal(&url);
    }

    /// Esc from the add/edit form (§6.2): discards unsaved input and, for
    /// an in-flight add, invalidates its pending id so the eventual fetch
    /// result is dropped as stale by `drain_feed_add_results`.
    pub(in crate::app) fn cancel_feed_form(&mut self) {
        if let Some(popup) = &mut self.app.feeds_manage {
            popup.pending_add = None;
        }
        if let Some(component) = self.feeds_manage_component_mut() {
            component.set_pending_add(None);
            component.set_stage(FeedsManageStage::List);
        }
    }

    pub(in crate::app) fn submit_feed_form(&mut self) {
        let Some(form) = self.feeds_manage_stage().and_then(|stage| match stage {
            FeedsManageStage::Form(form) => Some(form),
            FeedsManageStage::List => None,
        }) else {
            return;
        };
        match form.editing_index {
            Some(_) => {
                if self.app.edit_feed(&form.url, &form.name, form.kind) {
                    let feeds = self.app.config.lock().unwrap().feeds.clone();
                    if let Some(component) = self.feeds_manage_component_mut() {
                        component.set_stage(FeedsManageStage::List);
                        component.set_feeds(feeds);
                    }
                }
            }
            None => {
                if let Some(id) = self.app.begin_feed_add(&form.name, &form.url, form.kind)
                    && let Some(component) = self.feeds_manage_component_mut()
                {
                    component.set_pending_add(Some(id));
                }
            }
        }
    }

    /// Drain the in-flight add-feed fetch result, if any (§6.2): the
    /// `App` drain owns the channel and the config write; this half only
    /// projects the outcome onto the component's stage/pending/feeds.
    pub(in crate::app) fn drain_feed_add_results(&mut self) -> bool {
        match self.app.drain_feed_add_results() {
            FeedAddDrain::Idle => false,
            FeedAddDrain::Discarded => true,
            FeedAddDrain::Applied(feeds) => {
                if let Some(component) = self.feeds_manage_component_mut() {
                    component.set_stage(FeedsManageStage::List);
                    component.set_feeds(feeds);
                    component.set_pending_add(None);
                }
                true
            }
            FeedAddDrain::Failed => {
                if let Some(component) = self.feeds_manage_component_mut() {
                    component.set_pending_add(None);
                }
                true
            }
        }
    }
}
