//! A Settings pick row's control (`replace-tauri-with-blitz` task 8.2, the
//! port of `SettingsPickButton`; add-gui-settings-screen design D13,
//! D15, the canvas pick button): 34 px high, at least 140 px wide, the value
//! and a `ChevronDown`, opening the row's values in the row menu's chrome.
//! The press runs the row's activation (`pressSettingsEntry`), the same one
//! Enter runs, so the keyboard and the pointer open one menu. The menu is
//! window content: the open state lives in the Settings pick store
//! (`window/settings/pick_menu.rs`), so the Settings key router takes its
//! Up, Down, Enter, and Esc while it is open, and one press outside closes
//! it with no send. A press on an entry sends `setValue` for the row's
//! setting and that value, the same command the keyboard's Enter sends.
//!
//! the former client component closed the menu from a `window` `pointerdown`
//! listener that skipped the button and the menu (`buttonEl` and `menuEl`
//! containment); the same window also ran the Settings screen's
//! `hideSettingsCursor` on every pointerdown, presses on the pick controls
//! included. Blitz exposes no window listener, so the press-outside close
//! runs where the presses meet — the Settings screen's root pointerdown
//! (task 8.3), which owns the press-outside-close contract — and the button
//! and the menu stop the press's propagation, as the former skip list did.
//! Stopping alone would also swallow the root's cursor hide, so each
//! handler hides the cursor first (`press_inside`): the former behavior is
//! "not an outside press", never "not a press at all".

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiRowMenuEntry;
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::icons::ChevronDown;
use crate::window::settings::pick_menu::pick_value_label;
use crate::window::settings::state::SettingsState;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the pick button (the `SettingsPickButton` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsPickButtonProps {
    /// The row's setting name, which `setValue` carries back.
    pub setting: String,
    /// The row's accessible name.
    pub label: String,
    /// The value the row shows now; the empty value paints as Any.
    pub value: String,
    /// Whether the pick cannot be operated.
    #[props(default)]
    pub disabled: bool,
    /// A press on the button: the row's activation.
    pub onpress: Callback<()>,
}

/// The one pick button.
#[component]
pub fn SettingsPickButton(props: SettingsPickButtonProps) -> Element {
    let settings = consume_context::<SettingsState>();
    // The open menu, while it is this row's. A read (not a peek), so the
    // button repaints when the router opens or closes the menu; the menu
    // state is one row's values, never a frame.
    let menu = settings
        .pick_menu
        .cloned()
        .filter(|menu| menu.setting == props.setting);
    let row_slot: Signal<Vec<Option<Event<MountedData>>>> = use_signal(Vec::new);
    let aria = format!("{}: {}", props.label, pick_value_label(&props.value));
    let onpress = props.onpress;
    let entries: Vec<GuiRowMenuEntry> = menu
        .as_ref()
        .map(|menu| {
            menu.values
                .iter()
                .map(|candidate| GuiRowMenuEntry {
                    label: pick_value_label(candidate).to_owned(),
                    key_hint: None,
                    destructive: false,
                    checked: None,
                    index: None,
                })
                .collect()
        })
        .unwrap_or_default();
    let select = settings.clone();
    let button_press = settings.clone();
    let menu_press = settings.clone();
    rsx! {
        div {
            class: "relative flex-none",
            "data-settings-pick": "{props.setting}",
            button {
                "type": "button",
                class: "flex items-center justify-between",
                style: "height: 34px; min-width: 140px; gap: 10px; padding: 0 10px 0 12px; background: var(--canvas-library-raise); border: 1px solid var(--canvas-library-control); border-radius: 6px; color: var(--canvas-text); font-size: 13px;",
                "aria-label": "{aria}",
                "aria-haspopup": "menu",
                "aria-expanded": "{menu.is_some()}",
                disabled: props.disabled,
                // A press on the button is the row's activation, not an
                // outside press: the screen root's close (task 8.3) skips it
                // the way the former listener skipped `buttonEl`. The cursor
                // hide is not an outside-press fact, so the press hides it
                // itself, as the former window listener did.
                onpointerdown: move |event: PointerEvent| {
                    press_inside(&button_press);
                    event.stop_propagation();
                },
                onclick: move |_| onpress.call(()),
                Truncate {
                    text: pick_value_label(&props.value).to_owned(),
                    class: "min-w-0".to_owned(),
                    style: TextStyle::sans(13.0, 400.0),
                }
                ChevronDown { size: 12, stroke_width: 2.2, aria_hidden: true }
            }
            if let Some(menu) = menu.as_ref() {
                div {
                    // A press inside the menu selects its entry; it is not an
                    // outside press, so it does
                    // not reach the screen root's close. The cursor hides, as
                    // every the former client pointerdown did.
                    onpointerdown: move |event: PointerEvent| {
                        press_inside(&menu_press);
                        event.stop_propagation();
                    },
                    QueueRowMenu {
                        label: format!("{} values", props.label),
                        cursor: Some(menu.cursor),
                        entries: entries.clone(),
                        top: "calc(100% + 4px)".to_owned(),
                        right: Some("0".to_owned()),
                        on_select: Some(Callback::new(move |index: usize| {
                            select.select_pick_value(index);
                        })),
                        row_slot,
                    }
                }
            }
        }
    }
}

/// A pointer press inside this row's pick controls (`hideSettingsCursor`):
/// the cursor hide the former window listener ran on every pointerdown,
/// presses on the pick controls included. The caller stops the press from
/// reaching the screen root's close afterwards.
fn press_inside(settings: &SettingsState) {
    settings.hide_settings_cursor();
}
