//! The Feeds section (`replace-tauri-with-blitz` task 8.4, the port of
//! `SettingsFeeds.svelte`; add-gui-settings-screen design D9, D13): the feed
//! subscriptions in a card of their own, the add control above them, and the
//! add/edit form as a card beside the list — a recessed well of labelled
//! mono fields with Cancel and the primary submit. A subscription's only
//! stable identity is its URL — the edit and the remove address it — so the
//! form's edit mode keeps the URL read-only and sends it back as the
//! identity. While a fetch runs the frame's `adding` flag shows the add in
//! progress.
//!
//! The kind control is the port's one markup deviation: the Svelte `<select>`
//! cannot work here — Blitz fires no `change` event (`DomEventData` has no
//! Change variant) and renders no popup — so the two kinds paint as Audio
//! and Video buttons, the chosen kind in the primary fill. The
//! `data-settings-field="kind"` name and the `Feed kind` accessible names
//! stay, and the draft's kind moves through the same `setFeedForm` update.
//!
//! The form host owns the keyboard hold's focus facts (task 8.4's wiring
//! requirement): its `focusin` re-arms the hold and its `focusout` drops it,
//! and the form's first field mounts with `focus_on_mount`, which arms the
//! hold and focuses the field — the port's stand-in for the Svelte
//! `focusOnMount` action's first-input query, plus the arm Blitz's focus
//! event does not deliver. The form's submit reads the shown frame from the
//! section store at press time (`submit_settings_frame`), so no repaint
//! clones the content for a press.

use dioxus_native::prelude::*;
use mbv_queue::FeedKind;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::window::components::settings_button::{SettingsButton, SettingsButtonVariant};
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_entry_row::SettingsEntryRow;
use crate::window::components::settings_field::SettingsField;
use crate::window::components::settings_screen::settings_section_view;
use crate::window::sections::Sections;
use crate::window::settings::entries::entry_id;
use crate::window::settings::focus::focus_on_mount;
use crate::window::settings::state::{SettingsForm, SettingsState, submit_settings_frame};
use crate::window::truncate::{TextStyle, Truncate};

/// The one Feeds pane (the Svelte markup, character for character).
#[component]
pub fn SettingsFeeds() -> Element {
    let sections = consume_context::<Sections>();
    let settings = consume_context::<SettingsState>();
    let Some(view) = settings_section_view(&sections, &settings) else {
        return rsx! {};
    };
    let GuiSettingsContent::Feeds {
        subscriptions,
        adding,
    } = &view.frame.content
    else {
        return rsx! {};
    };
    let adding = *adding;
    let ui = settings.ui.cloned();

    // A press on the add control, a subscription row, or a remove button:
    // the entry's activation, the same one the keyboard's Enter runs.
    let press_settings = settings.clone();
    let press_entries = view.entries;
    let onpress = Callback::new(move |id: String| {
        press_settings.press_settings_entry(&press_entries, &id);
    });

    // The count line the card's header paints.
    let count = subscriptions.len();
    let count_label = format!("{count} subscription{}", if count == 1 { "" } else { "s" });
    let header = rsx! {
        span {
            style: "font-size: 12px; color: var(--canvas-type-count);",
            "{count_label}"
        }
    };

    let add_press = onpress;
    let feed_press = onpress;
    let remove_press = onpress;
    let cursor_id = view.cursor_id.clone();
    rsx! {
        div {
            class: "grid",
            style: "grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 20px; align-items: start;",
            "data-settings-feeds": "",
            SettingsCard {
                title: Some("Subscriptions".to_owned()),
                header: Some(header),
                div {
                    class: "flex",
                    {
                        let add_id = entry_id::feed_add();
                        rsx! {
                            SettingsButton {
                                entry: Some(add_id.clone()),
                                cursor: cursor_id.as_deref() == Some(add_id.as_str()),
                                onpress: Some(Callback::new(move |()| add_press.call(add_id.clone()))),
                                "Add feed"
                            }
                        }
                    }
                }
                if adding {
                    div {
                        style: "font-size: 13px; color: var(--canvas-muted);",
                        "data-settings-feed-adding": "",
                        "Adding…"
                    }
                }
                div {
                    class: "flex flex-col",
                    for feed in subscriptions {
                        {
                            let feed_row_id = entry_id::feed(&feed.url);
                            let remove_id = entry_id::feed_remove(&feed.url);
                            let row_press = feed_press;
                            let row_cursor = cursor_id.as_deref() == Some(feed_row_id.as_str());
                            rsx! {
                                div {
                                    key: "{feed.url}",
                                    class: "flex items-center",
                                    style: "gap: 8px;",
                                    SettingsEntryRow {
                                        id: feed_row_id.clone(),
                                        cursor: row_cursor,
                                        onpress: row_press,
                                        label: feed.name.clone(),
                                        trailing: Some(feed.kind.as_str().to_owned()),
                                    }
                                    SettingsButton {
                                        variant: SettingsButtonVariant::Destructive,
                                        entry: Some(remove_id.clone()),
                                        cursor: cursor_id.as_deref() == Some(remove_id.as_str()),
                                        onpress: Some(Callback::new(move |()| remove_press.call(remove_id.clone()))),
                                        "Remove"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(form) = ui.feed_form.as_ref() {
                {
                    let form_settings = settings.clone();
                    let form_sections = sections.clone();
                    rsx! {
                        SettingsCard {
                            title: Some(if form.identity.is_none() { "Add feed".to_owned() } else { "Edit feed".to_owned() }),
                            { feed_form(&form_settings, &form_sections, &view.frame.content, form) }
                        }
                    }
                }
            }
        }
    }
}

/// The add/edit form (the Svelte `data-settings-feed-form` well): the edit's
/// identity line, the name and URL fields, the kind control, and the form's
/// actions.
fn feed_form(
    settings: &SettingsState,
    sections: &Sections,
    content: &GuiSettingsContent,
    form: &crate::window::settings::state::FeedFormDraft,
) -> Element {
    let focus_settings = settings.clone();
    let blur_settings = settings.clone();
    let name_settings = settings.clone();
    let url_settings = settings.clone();
    let kind_settings = settings.clone();
    let arm_settings = settings.clone();
    let submit_settings = settings.clone();
    let cancel_settings = settings.clone();
    let submit_sections = sections.clone();
    let adding = matches!(content, GuiSettingsContent::Feeds { adding: true, .. });
    let editing = form.identity.is_some();
    let identity = form.identity.clone();
    let name = form.name.clone();
    rsx! {
        div {
            class: "flex flex-col",
            style: "padding: 16px; gap: 12px; background: var(--canvas-well); border-radius: 8px;",
            "data-settings-feed-form": "",
            onfocusin: move |_| focus_settings.focus_settings_field(SettingsForm::Feed),
            onfocusout: move |_| blur_settings.blur_settings_field(SettingsForm::Feed),
            if let Some(identity) = identity.as_ref() {
                Truncate {
                    text: identity.clone(),
                    class: "font-mono".to_owned(),
                    style: TextStyle::mono(12.0, 400.0),
                    css: "font-size: 12px; color: var(--canvas-muted);".to_owned(),
                }
            }
            SettingsField {
                label: "Name".to_owned(),
                field: "name".to_owned(),
                aria_label: "Feed name".to_owned(),
                value: name,
                onmounted: Some(Callback::new(move |event: Event<MountedData>| {
                    focus_on_mount(event, &arm_settings, SettingsForm::Feed);
                })),
                oninput: Callback::new(move |name: String| {
                    name_settings.update_feed_form(move |form| form.name = name);
                }),
            }
            if !editing {
                SettingsField {
                    label: "Feed URL".to_owned(),
                    field: "url".to_owned(),
                    aria_label: "Feed URL".to_owned(),
                    placeholder: Some("https://example.com/feed.xml".to_owned()),
                    value: form.url.clone(),
                    oninput: Callback::new(move |url: String| {
                        url_settings.update_feed_form(move |form| form.url = url);
                    }),
                }
            }
            { kind_control(&kind_settings, form.kind) }
            div {
                class: "flex justify-end",
                style: "gap: 8px;",
                SettingsButton {
                    variant: SettingsButtonVariant::Ghost,
                    onpress: Some(Callback::new(move |()| cancel_settings.cancel_feed_form())),
                    "Cancel"
                }
                SettingsButton {
                    variant: SettingsButtonVariant::Primary,
                    disabled: adding,
                    onpress: Some(Callback::new(move |()| {
                        submit_settings_frame(&submit_settings, &submit_sections);
                    })),
                    if adding {
                        "Adding…"
                    } else if editing {
                        "Save"
                    } else {
                        "Add"
                    }
                }
            }
        }
    }
}

/// The kind control (the port's stand-in for the Svelte `<select>`, see the
/// module doc): the two kinds as buttons, the chosen kind in the primary
/// fill, each moving the draft's kind through the same update.
fn kind_control(settings: &SettingsState, kind: FeedKind) -> Element {
    rsx! {
        label {
            class: "flex flex-col",
            style: "gap: 6px; font-size: 12px; color: var(--canvas-muted);",
            "Kind"
            div {
                class: "flex",
                style: "gap: 6px;",
                "data-settings-field": "kind",
                for (candidate, name) in [(FeedKind::Audio, "Audio"), (FeedKind::Video, "Video")] {
                    {
                        let kind_settings = settings.clone();
                        rsx! {
                            SettingsButton {
                                key: "{candidate.as_str()}",
                                variant: if kind == candidate {
                                    SettingsButtonVariant::Primary
                                } else {
                                    SettingsButtonVariant::Secondary
                                },
                                label: Some(format!("Feed kind: {}", candidate.as_str())),
                                onpress: Some(Callback::new(move |()| {
                                    kind_settings.update_feed_form(move |form| form.kind = candidate);
                                })),
                                "{name}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
