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
use crate::window::settings::state::{
    AudiobookshelfSetupDraft, EmbySetupDraft, SettingsForm, SettingsState, submit_settings_frame,
};

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
                                    {
                                        let fields = emby_setup_fields(&settings, draft);
                                        setup_form(&settings, &sections, setup, fields)
                                    }
                                }
                            } else if service.entry == GuiServiceEntry::Audiobookshelf {
                                if let (Some(setup), Some(draft)) = (
                                    setup.as_ref().filter(|setup| setup.service == GuiSetupService::Audiobookshelf),
                                    ui.audiobookshelf_setup.as_ref(),
                                ) {
                                    {
                                        let fields = audiobookshelf_setup_fields(&settings, draft);
                                        setup_form(&settings, &sections, setup, fields)
                                    }
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

/// One field of a Service setup form (design D13): the label above the
/// input, the `data-settings-field` name, the input's accessible name, its
/// optional placeholder, the secret flag, the draft's current value, and the
/// update that writes a typed value back into the draft.
struct SetupField {
    label: &'static str,
    field: &'static str,
    aria_label: &'static str,
    placeholder: Option<&'static str>,
    secret: bool,
    value: String,
    update: Callback<String>,
}

/// One Service's setup form (design D13): the field list over the draft, the
/// error, and the form's actions. The first field mounts focused and arms the
/// keyboard hold.
fn setup_form(
    settings: &SettingsState,
    sections: &Sections,
    setup: &GuiSettingsSetup,
    fields: Vec<SetupField>,
) -> Element {
    let focus_settings = settings.clone();
    let blur_settings = settings.clone();
    let arm_settings = settings.clone();
    let submit_settings = settings.clone();
    let cancel_settings = settings.clone();
    let submit_sections = sections.clone();
    let form = setup_form_kind(setup.service);
    let busy = setup.busy;
    let error = setup.error.clone();
    let first_mounted = Callback::new(move |event: Event<MountedData>| {
        focus_on_mount(event, &arm_settings, form);
    });
    rsx! {
        div {
            class: "flex flex-col",
            style: "padding: 16px; gap: 12px; background: var(--canvas-well); border-radius: 8px;",
            "data-settings-setup": setup_wire(setup.service),
            onfocusin: move |_| focus_settings.focus_settings_field(form),
            onfocusout: move |_| blur_settings.blur_settings_field(form),
            for (index, field) in fields.into_iter().enumerate() {
                SettingsField {
                    label: field.label.to_owned(),
                    field: field.field.to_owned(),
                    aria_label: field.aria_label.to_owned(),
                    placeholder: field.placeholder.map(str::to_owned),
                    secret: field.secret,
                    value: field.value,
                    onmounted: (index == 0).then_some(first_mounted),
                    oninput: field.update,
                }
            }
            if !error.is_empty() {
                div {
                    style: "font-size: 13px; color: var(--canvas-destructive);",
                    "data-settings-setup-error": "",
                    "{error}"
                }
            }
            { form_actions(&submit_settings, &cancel_settings, &submit_sections, setup.service, busy) }
        }
    }
}

/// The Emby setup form's fields, over its draft.
fn emby_setup_fields(settings: &SettingsState, draft: &EmbySetupDraft) -> Vec<SetupField> {
    let url_settings = settings.clone();
    let username_settings = settings.clone();
    let password_settings = settings.clone();
    vec![
        SetupField {
            label: "Server URL",
            field: "serverUrl",
            aria_label: "Emby server URL",
            placeholder: Some("http://emby:8096"),
            secret: false,
            value: draft.server_url.clone(),
            update: Callback::new(move |server_url: String| {
                url_settings.update_emby_setup(move |draft| draft.server_url = server_url);
            }),
        },
        SetupField {
            label: "Username",
            field: "username",
            aria_label: "Emby username",
            placeholder: None,
            secret: false,
            value: draft.username.clone(),
            update: Callback::new(move |username: String| {
                username_settings.update_emby_setup(move |draft| draft.username = username);
            }),
        },
        SetupField {
            label: "Password",
            field: "password",
            aria_label: "Emby password",
            placeholder: None,
            secret: true,
            value: draft.password.clone(),
            update: Callback::new(move |password: String| {
                password_settings.update_emby_setup(move |draft| draft.password = password);
            }),
        },
    ]
}

/// The Audiobookshelf setup form's fields, over its draft.
fn audiobookshelf_setup_fields(
    settings: &SettingsState,
    draft: &AudiobookshelfSetupDraft,
) -> Vec<SetupField> {
    let url_settings = settings.clone();
    let key_settings = settings.clone();
    vec![
        SetupField {
            label: "Server URL",
            field: "serverUrl",
            aria_label: "Audiobookshelf server URL",
            placeholder: Some("http://audiobookshelf:13378"),
            secret: false,
            value: draft.server_url.clone(),
            update: Callback::new(move |server_url: String| {
                url_settings
                    .update_audiobookshelf_setup(move |draft| draft.server_url = server_url);
            }),
        },
        SetupField {
            label: "API key",
            field: "apiKey",
            aria_label: "Audiobookshelf API key",
            placeholder: None,
            secret: true,
            value: draft.api_key.clone(),
            update: Callback::new(move |api_key: String| {
                key_settings.update_audiobookshelf_setup(move |draft| draft.api_key = api_key);
            }),
        },
    ]
}

/// The setup form's painted `data-settings-setup` name: the Service's wire
/// spelling.
fn setup_wire(service: GuiSetupService) -> &'static str {
    match service {
        GuiSetupService::Emby => "emby",
        GuiSetupService::Audiobookshelf => "audiobookshelf",
    }
}

/// The keyboard hold's form fact for a Service's setup form.
fn setup_form_kind(service: GuiSetupService) -> SettingsForm {
    match service {
        GuiSetupService::Emby => SettingsForm::EmbySetup,
        GuiSetupService::Audiobookshelf => SettingsForm::AudiobookshelfSetup,
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
