//! The settings store's form methods (the port of `state.ts`'s form
//! half): the drafts of the Services setup forms, the feed form, and the
//! rename field, their field updates, and the submit and cancel that hold
//! the keyboard (design D9, D11 step 3). The drafts live in
//! [`super::SettingsUiState`]; these methods move them.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::frame::settings::feeds::GuiSettingsFeed;
use crate::bridge::frame::settings::services::GuiSetupService;
use crate::bridge::settings::GuiSettingsCommand;

use super::{
    AudiobookshelfSetupDraft, EmbySetupDraft, FeedFormDraft, RenameDraft, SettingsForm,
    SettingsState,
};

impl SettingsState {
    /// A form's text field holds the keyboard (`state.ts`'s
    /// `focusSettingsField`, the field's DOM `focus`): the key router's step
    /// 3 gate.
    pub fn focus_settings_field(&self, form: SettingsForm) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if ui.text_field != Some(form) {
                ui.text_field = Some(form);
            }
        });
    }

    /// A form's field lost the keyboard (`state.ts`'s `blurSettingsField`):
    /// the `focusout` whose target leaves the form clears the hold.
    pub fn blur_settings_field(&self, form: SettingsForm) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if ui.text_field == Some(form) {
                ui.text_field = None;
            }
        });
    }

    /// Open the feed form (`state.ts`'s `openFeedForm`): an add (`None`)
    /// starts empty, an edit seeds the subscription's name, URL, and kind
    /// with its URL as the identity.
    pub fn open_feed_form(&self, feed: Option<&GuiSettingsFeed>) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.feed_form = Some(match feed {
                None => FeedFormDraft {
                    identity: None,
                    name: String::new(),
                    url: String::new(),
                    kind: mbv_queue::FeedKind::Audio,
                },
                Some(feed) => FeedFormDraft {
                    identity: Some(feed.url.clone()),
                    name: feed.name.clone(),
                    url: feed.url.clone(),
                    kind: feed.kind,
                },
            });
        });
    }

    /// Change one feed-form field (the input's own event, `state.ts`'s
    /// `setFeedForm`): the update applies while the form is open.
    pub fn update_feed_form(&self, update: impl FnOnce(&mut FeedFormDraft)) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if let Some(form) = &mut ui.feed_form {
                update(form);
            }
        });
    }

    /// Open the playlist rename field on `id`, seeded with the shown name
    /// (`state.ts`'s `openRenameDraft`).
    pub fn open_rename_draft(&self, id: &str, name: &str) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.rename = Some(RenameDraft {
                id: id.to_owned(),
                name: name.to_owned(),
            });
        });
    }

    /// Change the rename draft's text (`state.ts`'s `setRenameDraft`).
    pub fn set_rename_draft(&self, name: &str) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if let Some(draft) = &mut ui.rename {
                name.clone_into(&mut draft.name);
            }
        });
    }

    /// Change one Emby setup field (the input's own event, `state.ts`'s
    /// `setEmbySetupDraft`).
    pub fn update_emby_setup(&self, update: impl FnOnce(&mut EmbySetupDraft)) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if let Some(draft) = &mut ui.emby_setup {
                update(draft);
            }
        });
    }

    /// Change one Audiobookshelf setup field (`state.ts`'s
    /// `setAudiobookshelfSetupDraft`).
    pub fn update_audiobookshelf_setup(&self, update: impl FnOnce(&mut AudiobookshelfSetupDraft)) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if let Some(draft) = &mut ui.audiobookshelf_setup {
                update(draft);
            }
        });
    }

    /// Submit the form that holds the keyboard (`state.ts`'s
    /// `submitSettingsForm`, design D11, step 3): the typed secret rides the
    /// command and its field clears at once, so no draft keeps it past the
    /// send. A setup submit while the form is busy sends nothing.
    pub fn submit_settings_form(&self, content: &GuiSettingsContent) {
        let ui = self.ui.peek().clone();
        match ui.text_field {
            Some(SettingsForm::EmbySetup) => {
                let Some(draft) = ui.emby_setup.clone() else {
                    return;
                };
                if setup_busy(content, GuiSetupService::Emby) {
                    return;
                }
                self.link.settings(GuiSettingsCommand::EmbySetup {
                    server_url: draft.server_url,
                    username: draft.username,
                    password: draft.password,
                });
                let mut ui = self.ui;
                ui.with_mut(|state| {
                    if let Some(draft) = &mut state.emby_setup {
                        draft.password.clear();
                    }
                });
            }
            Some(SettingsForm::AudiobookshelfSetup) => {
                let Some(draft) = ui.audiobookshelf_setup.clone() else {
                    return;
                };
                if setup_busy(content, GuiSetupService::Audiobookshelf) {
                    return;
                }
                self.link.settings(GuiSettingsCommand::AudiobookshelfSetup {
                    server_url: draft.server_url,
                    api_key: draft.api_key,
                });
                let mut ui = self.ui;
                ui.with_mut(|state| {
                    if let Some(draft) = &mut state.audiobookshelf_setup {
                        draft.api_key.clear();
                    }
                });
            }
            Some(SettingsForm::Feed) => {
                let Some(form) = ui.feed_form.clone() else {
                    return;
                };
                match &form.identity {
                    None => {
                        self.link.settings(GuiSettingsCommand::FeedAdd {
                            name: form.name,
                            url: form.url,
                            kind: form.kind,
                        });
                    }
                    Some(identity) => {
                        self.link.settings(GuiSettingsCommand::FeedEdit {
                            url: identity.clone(),
                            name: form.name,
                            kind: form.kind,
                        });
                    }
                }
                let mut ui = self.ui;
                ui.with_mut(|state| {
                    state.feed_form = None;
                    state.text_field = None;
                });
            }
            Some(SettingsForm::Rename) => {
                let Some(draft) = ui.rename.clone() else {
                    return;
                };
                self.link.settings(GuiSettingsCommand::PlaylistRename {
                    id: draft.id,
                    name: draft.name,
                });
                let mut ui = self.ui;
                ui.with_mut(|state| {
                    state.rename = None;
                    state.text_field = None;
                });
            }
            None => {}
        }
    }

    /// Cancel the form that holds the keyboard (`state.ts`'s
    /// `cancelSettingsForm`, design D11, step 3): Esc in a focused field. The
    /// setup forms cancel in the core, which retracts the form; the
    /// front-end drafts simply close.
    pub fn cancel_settings_form(&self) {
        let ui = self.ui.peek().clone();
        match ui.text_field {
            Some(SettingsForm::EmbySetup) => {
                self.link.settings(GuiSettingsCommand::CancelSetup {
                    service: GuiSetupService::Emby,
                });
            }
            Some(SettingsForm::AudiobookshelfSetup) => {
                self.link.settings(GuiSettingsCommand::CancelSetup {
                    service: GuiSetupService::Audiobookshelf,
                });
            }
            Some(SettingsForm::Feed | SettingsForm::Rename) => {
                let mut ui = self.ui;
                ui.with_mut(|state| {
                    state.feed_form = None;
                    state.rename = None;
                    state.text_field = None;
                });
            }
            None => {}
        }
    }
}

/// Whether the named setup form is busy (`state.ts`'s `setupBusy`), so a
/// submit under a running validation sends nothing.
fn setup_busy(content: &GuiSettingsContent, service: GuiSetupService) -> bool {
    super::frame_setup_content(content).is_some_and(|setup| setup.service == service && setup.busy)
}
