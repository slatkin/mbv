//! The settings store's frame-fold methods (the port of `state.ts`'s
//! `syncSettingsFrame` and `resetSettingsUi`): one arriving frame folds
//! into the interaction state, and a UI reset drops it whole (design D1,
//! D10).

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsFrame;
use crate::bridge::frame::settings::services::GuiSetupService;

use super::{
    AudiobookshelfSetupDraft, EmbySetupDraft, SettingsForm, SettingsPane, SettingsState,
    SettingsUiState, frame_setup_content,
};

impl SettingsState {
    /// Fold one arriving frame's Settings section into the interaction state
    /// (`state.ts`'s `syncSettingsFrame`): the screen's open resets
    /// everything (a reopen starts on the sidebar at the frame's section), a
    /// section change re-anchors the sidebar cursor and moves the keyboard to
    /// the content — opening a section shows it (the spec's *Narrow window*
    /// requirement) — and the frame's answer ends a sent capture. The setup
    /// draft seeds from the form's carried server URL the first time the
    /// frame opens it, and leaves with the form. `None` closes the screen's
    /// state whole.
    pub fn sync_settings_frame(&self, settings: Option<&GuiSettingsFrame>) {
        let mut ui = self.ui;
        let Some(settings) = settings else {
            // A closed screen resets the state whole; an already-closed one
            // keeps its value, so the write marks no subscriber.
            ui.with_mut(|ui| {
                if *ui != SettingsUiState::initial() {
                    *ui = SettingsUiState::initial();
                }
            });
            return;
        };
        ui.with_mut(|ui| {
            let next = fold_frame(ui, settings);
            if *ui != next {
                *ui = next;
            }
        });
    }

    /// Reset the interaction state whole (`state.ts`'s `resetSettingsUi`,
    /// design D10): a new `uiResetEpoch` drops the cursors, the drafts, the
    /// marks, and the capture while the core keeps the open section.
    pub fn reset_settings_ui(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            *ui = SettingsUiState {
                section: ui.section,
                ..SettingsUiState::initial()
            };
        });
    }
}

/// One arriving frame folded into the interaction state (`state.ts`'s
/// `syncSettingsFrame` update body).
#[must_use]
fn fold_frame(ui: &SettingsUiState, settings: &GuiSettingsFrame) -> SettingsUiState {
    let mut next = ui.clone();
    if ui.section != Some(settings.section) {
        let sidebar_cursor = settings
            .sections
            .iter()
            .position(|section| *section == settings.section)
            .unwrap_or(0);
        let opened = ui.section.is_none();
        next = SettingsUiState {
            // A fresh open lands the keyboard on the sidebar; a section
            // change while the screen is open shows the new section.
            pane: if opened {
                SettingsPane::Sidebar
            } else {
                SettingsPane::Content
            },
            sidebar_cursor,
            section: Some(settings.section),
            ..SettingsUiState::initial()
        };
    }
    // The frame answered the sent capture: the new chord or the refusal
    // paints where the prompt was.
    if next.capture.as_ref().is_some_and(|capture| capture.sent) {
        next.capture = None;
    }
    let setup = frame_setup_content(&settings.content);
    match setup {
        None => {
            if next.text_field == Some(SettingsForm::EmbySetup)
                || next.text_field == Some(SettingsForm::AudiobookshelfSetup)
            {
                next.text_field = None;
            }
            next.emby_setup = None;
            next.audiobookshelf_setup = None;
        }
        Some(setup) if setup.service == GuiSetupService::Emby && next.emby_setup.is_none() => {
            next.emby_setup = Some(EmbySetupDraft {
                server_url: setup.server_url.clone(),
                username: String::new(),
                password: String::new(),
            });
        }
        Some(setup)
            if setup.service == GuiSetupService::Audiobookshelf
                && next.audiobookshelf_setup.is_none() =>
        {
            next.audiobookshelf_setup = Some(AudiobookshelfSetupDraft {
                server_url: setup.server_url.clone(),
                api_key: String::new(),
            });
        }
        Some(_) => {}
    }
    next
}
