//! The Services section (`replace-tauri-with-blitz` task 8.4, the port of
//! `SettingsServices`; add-gui-settings-screen design D9, D13's canvas
//! Services grid): one card for each Service in the canvas's two-column
//! grid. The card's header carries the Service's name and, on the right, its
//! state pill — or, for Feeds, the subscription count the frame projects.
//! The actions the TUI offers for the Service's state paint as the canvas's
//! buttons: Remove destructive, setup primary, Cancel ghost, the rest
//! secondary. The open setup form is a recessed well inside its own
//! Service's card, with labelled mono fields — the Emby three-field form or
//! the Audiobookshelf two-field one — with its progress and its error. The
//! password and API-key fields are password inputs the draft clears on
//! submit, so a secret the user typed leaves the window in the command and
//! never in a frame or a draft (the spec's *Secrets never return*).
//!
//! The form host owns the keyboard hold's focus facts (task 8.4's wiring
//! requirement): its `focusin` re-arms the hold (`focusSettingsField`) and
//! its `focusout` drops it (`blurSettingsField`) — moving between the form's
//! own fields blurs one field and focuses the next, so the hold survives the
//! move and a focus-out of the form ends it. The form's first field mounts
//! with `focus_on_mount`, which arms the hold and focuses the field: the
//! port's stand-in for the former client `focusOnMount` action's first-input query,
//! plus the arm Blitz's focus event does not deliver.
//!
//! A form's submit reads the shown frame from the section store at press
//! time (`submit_settings_frame`), so no repaint clones the content for a
//! press.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::frame::settings::services::{
    GuiServiceActionIntent, GuiServiceEntry, GuiSettingsSetup, GuiSetupService,
};
use crate::bridge::settings::GuiSettingsCommand;
use crate::window::components::settings_button::{SettingsButton, SettingsButtonVariant};
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_field::SettingsField;
use crate::window::components::settings_screen::{
    SETTINGS_PANES_GRID_STYLE, SettingsPaneScaffold, settings_pane,
};
use crate::window::components::settings_state_pill::SettingsStatePill;
use crate::window::sections::Sections;
use crate::window::settings::entries::entry_id;
use crate::window::settings::focus::focus_on_mount;
use crate::window::settings::state::{SettingsForm, SettingsState, submit_settings_frame};

/// The one Services pane.
#[component]
pub fn SettingsServices() -> Element {
    let Some(SettingsPaneScaffold {
        sections,
        settings,
        frame,
        onpress,
        cursor_id,
        ..
    }) = settings_pane()
    else {
        return rsx! {};
    };
    let GuiSettingsContent::Services { services, setup } = &frame.content else {
        return rsx! {};
    };
    let ui = settings.ui.cloned();

    rsx! {
        div {
            class: "grid",
            style: SETTINGS_PANES_GRID_STYLE,
            "data-settings-services": "",
            for service in services {
                {
                    let header = if service.entry == GuiServiceEntry::Feeds && !service.context.is_empty() {
                        rsx! {
                            span {
                                style: "font-size: 12px; color: var(--canvas-type-count);",
                                "data-settings-service-context": "",
                                "{service.context}"
                            }
                        }
                    } else {
                        rsx! {
                            SettingsStatePill { state: service.state.clone() }
                        }
                    };
                    rsx! {
                        SettingsCard {
                            key: "{service.entry:?}",
                            title: Some(service.name.clone()),
                            label: Some(service.name.clone()),
                            header: Some(header),
                            div {
                                class: "flex flex-wrap",
                                style: "gap: 8px;",
                                for action in &service.actions {
                                    {
                                        let id = entry_id::service_action(service.entry, action.intent);
                                        let action_press = onpress;
                                        rsx! {
                                            SettingsButton {
                                                key: "{id}",
                                                variant: action_variant(action.intent),
                                                entry: Some(id.clone()),
                                                cursor: cursor_id.as_deref() == Some(id.as_str()),
                                                onpress: Some(Callback::new(move |()| action_press.call(id.clone()))),
                                                "{action.label}"
                                            }
                                        }
                                    }
                                }
                            }
                            if service.entry == GuiServiceEntry::Emby {
                                if let (Some(setup), Some(draft)) = (
                                    setup.as_ref().filter(|setup| setup.service == GuiSetupService::Emby),
                                    ui.emby_setup.as_ref(),
                                ) {
                                    { emby_form(&settings, &sections, setup, draft) }
                                }
                            } else if service.entry == GuiServiceEntry::Audiobookshelf {
                                if let (Some(setup), Some(draft)) = (
                                    setup.as_ref().filter(|setup| setup.service == GuiSetupService::Audiobookshelf),
                                    ui.audiobookshelf_setup.as_ref(),
                                ) {
                                    { audiobookshelf_form(&settings, &sections, setup, draft) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Each Service action's button kind (design D13): Remove is destructive,
/// setup is primary, and every other action is secondary.
fn action_variant(intent: GuiServiceActionIntent) -> SettingsButtonVariant {
    match intent {
        GuiServiceActionIntent::SetupEmby | GuiServiceActionIntent::SetupAudiobookshelf => {
            SettingsButtonVariant::Primary
        }
        GuiServiceActionIntent::RemoveEmby | GuiServiceActionIntent::RemoveAudiobookshelf => {
            SettingsButtonVariant::Destructive
        }
        _ => SettingsButtonVariant::Secondary,
    }
}

/// The Emby setup form: the
/// three fields over the draft, the error, and the form's actions.
fn emby_form(
    settings: &SettingsState,
    sections: &Sections,
    setup: &GuiSettingsSetup,
    draft: &crate::window::settings::state::EmbySetupDraft,
) -> Element {
    let focus_settings = settings.clone();
    let blur_settings = settings.clone();
    let url_settings = settings.clone();
    let username_settings = settings.clone();
    let password_settings = settings.clone();
    let arm_settings = settings.clone();
    let submit_settings = settings.clone();
    let cancel_settings = settings.clone();
    let submit_sections = sections.clone();
    let busy = setup.busy;
    let error = setup.error.clone();
    let server_url = draft.server_url.clone();
    let username = draft.username.clone();
    let password = draft.password.clone();
    rsx! {
        div {
            class: "flex flex-col",
            style: "padding: 16px; gap: 12px; background: var(--canvas-well); border-radius: 8px;",
            "data-settings-setup": "emby",
            onfocusin: move |_| focus_settings.focus_settings_field(SettingsForm::EmbySetup),
            onfocusout: move |_| blur_settings.blur_settings_field(SettingsForm::EmbySetup),
            SettingsField {
                label: "Server URL".to_owned(),
                field: "serverUrl".to_owned(),
                aria_label: "Emby server URL".to_owned(),
                placeholder: Some("http://emby:8096".to_owned()),
                value: server_url,
                onmounted: Some(Callback::new(move |event: Event<MountedData>| {
                    focus_on_mount(event, &arm_settings, SettingsForm::EmbySetup);
                })),
                oninput: Callback::new(move |server_url: String| {
                    url_settings.update_emby_setup(move |draft| draft.server_url = server_url);
                }),
            }
            SettingsField {
                label: "Username".to_owned(),
                field: "username".to_owned(),
                aria_label: "Emby username".to_owned(),
                value: username,
                oninput: Callback::new(move |username: String| {
                    username_settings.update_emby_setup(move |draft| draft.username = username);
                }),
            }
            SettingsField {
                label: "Password".to_owned(),
                field: "password".to_owned(),
                aria_label: "Emby password".to_owned(),
                secret: true,
                value: password,
                oninput: Callback::new(move |password: String| {
                    password_settings.update_emby_setup(move |draft| draft.password = password);
                }),
            }
            if !error.is_empty() {
                div {
                    style: "font-size: 13px; color: var(--canvas-destructive);",
                    "data-settings-setup-error": "",
                    "{error}"
                }
            }
            { form_actions(&submit_settings, &cancel_settings, &submit_sections, GuiSetupService::Emby, busy) }
        }
    }
}

/// The Audiobookshelf setup form: the two fields over the draft, the error, and
/// the form's actions.
fn audiobookshelf_form(
    settings: &SettingsState,
    sections: &Sections,
    setup: &GuiSettingsSetup,
    draft: &crate::window::settings::state::AudiobookshelfSetupDraft,
) -> Element {
    let focus_settings = settings.clone();
    let blur_settings = settings.clone();
    let url_settings = settings.clone();
    let key_settings = settings.clone();
    let arm_settings = settings.clone();
    let submit_settings = settings.clone();
    let cancel_settings = settings.clone();
    let submit_sections = sections.clone();
    let busy = setup.busy;
    let error = setup.error.clone();
    let server_url = draft.server_url.clone();
    let api_key = draft.api_key.clone();
    rsx! {
        div {
            class: "flex flex-col",
            style: "padding: 16px; gap: 12px; background: var(--canvas-well); border-radius: 8px;",
            "data-settings-setup": "audiobookshelf",
            onfocusin: move |_| focus_settings.focus_settings_field(SettingsForm::AudiobookshelfSetup),
            onfocusout: move |_| blur_settings.blur_settings_field(SettingsForm::AudiobookshelfSetup),
            SettingsField {
                label: "Server URL".to_owned(),
                field: "serverUrl".to_owned(),
                aria_label: "Audiobookshelf server URL".to_owned(),
                placeholder: Some("http://audiobookshelf:13378".to_owned()),
                value: server_url,
                onmounted: Some(Callback::new(move |event: Event<MountedData>| {
                    focus_on_mount(event, &arm_settings, SettingsForm::AudiobookshelfSetup);
                })),
                oninput: Callback::new(move |server_url: String| {
                    url_settings.update_audiobookshelf_setup(move |draft| draft.server_url = server_url);
                }),
            }
            SettingsField {
                label: "API key".to_owned(),
                field: "apiKey".to_owned(),
                aria_label: "Audiobookshelf API key".to_owned(),
                secret: true,
                value: api_key,
                oninput: Callback::new(move |api_key: String| {
                    key_settings.update_audiobookshelf_setup(move |draft| draft.api_key = api_key);
                }),
            }
            if !error.is_empty() {
                div {
                    style: "font-size: 13px; color: var(--canvas-destructive);",
                    "data-settings-setup-error": "",
                    "{error}"
                }
            }
            { form_actions(&submit_settings, &cancel_settings, &submit_sections, GuiSetupService::Audiobookshelf, busy) }
        }
    }
}

/// The form's actions: a ghost Cancel that
/// cancels the form's setup, and the primary submit — its label the
/// validation's progress while the form is busy, and disabled with it.
fn form_actions(
    settings: &SettingsState,
    cancel: &SettingsState,
    sections: &Sections,
    service: GuiSetupService,
    busy: bool,
) -> Element {
    let submit_settings = settings.clone();
    let cancel_settings = cancel.clone();
    let submit_sections = sections.clone();
    rsx! {
        div {
            class: "flex justify-end",
            style: "gap: 8px;",
            SettingsButton {
                variant: SettingsButtonVariant::Ghost,
                onpress: Some(Callback::new(move |()| cancel_setup(&cancel_settings, service))),
                "Cancel"
            }
            SettingsButton {
                variant: SettingsButtonVariant::Primary,
                disabled: busy,
                onpress: Some(Callback::new(move |()| {
                    submit_settings_frame(&submit_settings, &submit_sections);
                })),
                if busy { "Checking…" } else { "Set up" }
            }
        }
    }
}

/// The form's Cancel (`state.ts`'s `sendSettingsCommand`): cancel the named
/// setup form.
fn cancel_setup(settings: &SettingsState, service: GuiSetupService) {
    settings
        .link
        .settings(GuiSettingsCommand::CancelSetup { service });
}

#[cfg(test)]
mod tests;
