//! The Library dropdown's shared presentation (gui-collection-dropdown design
//! D6, D8): the accent-outlined trigger, the popover under it, and the rows
//! that name one choice. The collection dropdown and the Shows view dropdown
//! supply the label, the rows, the marker kind, and the handlers; this
//! component owns no destination vocabulary and no selection rule. The
//! collection dropdown keeps its filter field; the Shows view dropdown omits
//! it, so a view press replaces the active entry and closes the popover.

use dioxus_native::prelude::*;

use crate::window::components::checkbox_box::{CheckboxBox, CheckboxBoxElement};
use crate::window::components::clear_field_button::ClearFieldButton;
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::components::use_focus_while::use_focus_while;
use crate::window::icons::{ChevronDown, ChevronUp};
use crate::window::input::f32_from_f64;

/// The popover's width in logical pixels (`20rem` at the 16 px root).
const MENU_WIDTH_PX: f32 = 320.0;

/// The gap the popover keeps from the window's leading edge when it shifts
/// right to fit.
const MENU_EDGE_PX: f32 = 16.0;

/// How far right a trailing-anchored popover shifts so its leading edge stays
/// on screen: the trigger's right edge minus the popover's width is the
/// leading edge, and a leading edge left of the margin moves right to it.
fn trailing_shift(trigger_right: f32) -> f32 {
    (MENU_EDGE_PX - (trigger_right - MENU_WIDTH_PX)).max(0.0)
}

/// One row of an open Library dropdown: the key its press sends, its label,
/// and whether it is the active choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryDropdownRow {
    /// The stable key the press sends to the caller.
    pub key: String,
    /// The row's painted label.
    pub label: String,
    /// Whether this row is the active choice.
    pub selected: bool,
}

/// The leading mark a dropdown row paints: a collection checkbox or a
/// single-choice view radio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryDropdownMarker {
    /// A square checkbox, used by the multi-choice collection dropdown.
    Check,
    /// A round radio mark, used by the single-choice Shows view dropdown.
    Radio,
}

/// The props of the one dropdown presentation.
#[derive(Props, Clone, PartialEq)]
pub struct LibraryDropdownProps {
    /// The trigger's label, naming the active choice.
    pub label: String,
    /// Whether the popover paints.
    pub open: bool,
    /// The rows the open popover paints, in display order.
    pub rows: Vec<LibraryDropdownRow>,
    /// The leading mark's kind.
    pub marker: LibraryDropdownMarker,
    /// The dropdown's accessible name and the menu's accessible name.
    pub aria_label: &'static str,
    /// The trigger's test attribute.
    pub trigger_attr: &'static str,
    /// The popover's test attribute.
    pub menu_attr: &'static str,
    /// A row's test attribute; the row's key is its value.
    pub row_attr: &'static str,
    /// The leading mark's test attribute.
    pub mark_attr: &'static str,
    /// An optional attribute on the first row, used by the All row.
    #[props(default)]
    pub first_row_attr: Option<&'static str>,
    /// Whether the first row takes an extra gap below it.
    #[props(default)]
    pub first_row_gap: bool,
    /// The filter text when the dropdown paints a filter field.
    #[props(default)]
    pub filter: Option<String>,
    /// The filter field's placeholder, when one paints.
    #[props(default)]
    pub filter_placeholder: Option<&'static str>,
    /// The filter field's test attribute.
    #[props(default)]
    pub filter_attr: Option<&'static str>,
    /// Open or close the popover.
    pub on_toggle: Callback<()>,
    /// Select the row whose key the callback carries.
    pub on_select: Callback<String>,
    /// Type into the filter field. The Shows view passes a no-op callback.
    pub on_filter: Callback<String>,
    /// A press that landed on the dropdown's own surface.
    pub on_inside_press: Callback<()>,
    /// Whether the trigger sits at the wrapper's trailing edge and the popover
    /// anchors to the trigger's right. The collection and view dropdowns sit
    /// in the list header's trailing slot; the Shows Hero's season dropdown
    /// leads its workspace band instead.
    #[props(default = true)]
    pub align_end: bool,
}

/// One row's look: the selected band and bold name the artboard carries, for
/// both the checkbox and the radio marker.
fn row_style(selected: bool, first_row_gap: bool) -> String {
    let selected_style = if selected {
        "font-weight: 600; color: var(--canvas-heading); background: var(--canvas-selected);"
    } else {
        "font-weight: 400; color: var(--canvas-text); background: none;"
    };
    format!(
        "height: 32px; padding: 0 10px; gap: 10px; border-radius: 4px; font-size: 14px;{}{}",
        if first_row_gap {
            " margin-bottom: 8px;"
        } else {
            ""
        },
        selected_style
    )
}

/// A collection row's square checkbox.
fn checkbox_mark(selected: bool) -> Element {
    rsx! {
        CheckboxBox {
            checked: selected,
            element: CheckboxBoxElement::Div,
            style: if selected {
                "width: 14px; height: 14px; flex: none; border-radius: 3px; border: 1px solid var(--canvas-accent); background: var(--canvas-accent);".to_owned()
            } else {
                "width: 14px; height: 14px; flex: none; border-radius: 3px; border: 1px solid var(--canvas-border); background: none;".to_owned()
            },
            size: 10,
            stroke_width: 2.0,
            color: "var(--canvas-menu)".to_owned(),
            collections_marker: true,
        }
    }
}

/// A single-choice view row's radio mark: a ring with a filled centre while
/// the entry is active.
fn radio_mark(selected: bool) -> Element {
    rsx! {
        span {
            class: "flex flex-none items-center justify-center",
            style: if selected {
                "width: 14px; height: 14px; border-radius: 999px; border: 1px solid var(--canvas-accent); background: var(--canvas-accent);"
            } else {
                "width: 14px; height: 14px; border-radius: 999px; border: 1px solid var(--canvas-border); background: none;"
            },
            if selected {
                span {
                    style: "width: 4px; height: 4px; border-radius: 999px; background: var(--canvas-menu);",
                }
            }
        }
    }
}

/// The callbacks carry no painted fact, so the debug form names only the
/// values the dropdown paints.
impl std::fmt::Debug for LibraryDropdownProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryDropdownProps")
            .field("label", &self.label)
            .field("open", &self.open)
            .field("rows", &self.rows)
            .field("marker", &self.marker)
            .field("align_end", &self.align_end)
            .finish_non_exhaustive()
    }
}

/// The one trigger, popover, and row presentation.
#[component]
pub fn LibraryDropdown(props: LibraryDropdownProps) -> Element {
    let LibraryDropdownProps {
        label,
        open,
        rows,
        marker,
        aria_label,
        trigger_attr,
        menu_attr,
        row_attr,
        mark_attr,
        first_row_attr,
        first_row_gap,
        filter,
        filter_placeholder,
        filter_attr,
        on_toggle,
        on_select,
        on_filter,
        on_inside_press,
        align_end,
    } = props;

    let has_filter = filter.is_some();
    let mut filter_mounted = use_focus_while(move || open && has_filter);
    // A trailing-anchored popover hangs left of the trigger's right edge; in a
    // narrow Library column that edge is left of the window's, so the popover
    // measures the trigger as it opens and shifts right, over the other
    // columns.
    let mut anchor: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let mut shift = use_signal(|| 0.0_f32);
    use_effect(use_reactive(
        &(open, align_end),
        move |(open, align_end)| {
            let mounted = anchor.cloned();
            spawn(async move {
                if !open || !align_end {
                    shift.set(0.0);
                    return;
                }
                let Some(event) = mounted else {
                    return;
                };
                let Ok(rect) = event.get_client_rect().await else {
                    return;
                };
                // A trigger with no laid-out width is unmeasured: leave the
                // popover on its anchor.
                if rect.width() <= 0.0 {
                    return;
                }
                shift.set(trailing_shift(f32_from_f64(rect.max_x())));
            });
        },
    ));
    // `0.0 - shift`, not `-shift`: a rest shift of zero paints `0`, not `-0`.
    let right = 0.0 - shift.cloned();

    // One flag paints both alignments: the wrapper justifies the trigger to
    // the flag's edge, and the popover anchors there — hung off the trigger's
    // right edge by `{right}px`, or led from the wrapper's edge at `0`.
    let (side, justify) = if align_end {
        ("right", "justify-end")
    } else {
        ("left", "justify-start")
    };

    let toggle_attrs = vec![Attribute::new(trigger_attr, "", None, false)];
    let menu_attrs = vec![Attribute::new(menu_attr, "", None, false)];
    let filter_attrs = filter_attr.map_or(Vec::new(), |name| {
        vec![Attribute::new(name, "", None, false)]
    });

    rsx! {
        div {
            class: "relative flex min-w-0 flex-1 items-center {justify}",
            role: "group",
            "aria-label": "{aria_label}",
            style: "gap: 12px;",
            onpointerdown: move |event: PointerEvent| {
                event.stop_propagation();
                on_inside_press.call(());
            },
            div {
                class: "relative flex min-w-0 items-center",
                style: "gap: 6px;",
                onmounted: move |event: Event<MountedData>| anchor.set(Some(event)),
                button {
                    "type": "button",
                    class: "flex flex-none items-center",
                    style: "height: 28px; max-width: 18rem; padding: 0 12px; gap: 6px; border-radius: 6px; font-size: 14px; border: 1px solid var(--canvas-accent); background: none; color: var(--canvas-heading);",
                    "aria-label": "Select {aria_label}",
                    "aria-haspopup": "menu",
                    "aria-expanded": "{open}",
                    onclick: move |_| on_toggle.call(()),
                    ..toggle_attrs,
                    span {
                        style: "min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        "{label}"
                    }
                    if open {
                        ChevronUp {
                            size: 14,
                            color: "var(--canvas-accent)".to_owned(),
                        }
                    } else {
                        ChevronDown {
                            size: 14,
                            color: "var(--canvas-accent)".to_owned(),
                        }
                    }
                }
                if open {
                    div {
                        class: "absolute z-20 flex flex-col",
                        role: "menu",
                        "aria-label": "{aria_label}",
                        style: format!(
                            "top: 34px; {side}: {right}px; width: 20rem; max-width: calc(100vw - 48px); {MENU_CARD_STYLE}"
                        ),
                        ..menu_attrs,
                        if let Some(filter) = filter {
                            label {
                                class: "flex flex-none items-center",
                                style: "height: 40px; padding: 0 12px; gap: 10px; border-bottom: 1px solid var(--canvas-menu-border); color: var(--canvas-muted);",
                                span {
                                    class: "font-mono",
                                    style: "font-size: 14px; color: var(--canvas-accent);",
                                    "/"
                                }
                                input {
                                    "type": "text",
                                    class: "min-w-0 flex-1",
                                    style: "background: none; border: 0; outline: none; color: var(--canvas-heading); font-size: 14px;",
                                    placeholder: filter_placeholder.unwrap_or_default(),
                                    "aria-label": "Filter the choices",
                                    value: "{filter}",
                                    onmounted: move |event: Event<MountedData>| filter_mounted.set(Some(event)),
                                    oninput: move |event: FormEvent| on_filter.call(event.value()),
                                    ..filter_attrs,
                                }
                                if !filter.is_empty() {
                                    ClearFieldButton {
                                        label: "Clear filter",
                                        on_clear: move |()| on_filter.call(String::new()),
                                    }
                                }
                            }
                        }
                        div {
                            class: "flex flex-col flex-none",
                            style: "padding: 6px;",
                            for (position, row) in rows.into_iter().enumerate() {
                                {
                                    let first = position == 0;
                                    let selected = row.selected;
                                    let key = row.key.clone();
                                    let attrs = vec![Attribute::new(row_attr, key.clone(), None, false)];
                                    let mark_attrs = vec![Attribute::new(mark_attr, "", None, false)];
                                    let first_attrs = if first {
                                        first_row_attr.map_or(Vec::new(), |name| {
                                            vec![Attribute::new(name, "", None, false)]
                                        })
                                    } else {
                                        Vec::new()
                                    };
                                    let select = on_select;
                                    rsx! {
                                        button {
                                            "type": "button",
                                            class: "flex flex-none items-center text-left",
                                            role: if marker == LibraryDropdownMarker::Radio { "menuitemradio" } else { "menuitemcheckbox" },
                                            style: row_style(selected, first && first_row_gap),
                                            "aria-checked": "{selected}",
                                            onclick: move |_| select.call(key.clone()),
                                            ..attrs,
                                            ..first_attrs,
                                            div {
                                                class: "flex flex-none items-center justify-center",
                                                ..mark_attrs,
                                                if marker == LibraryDropdownMarker::Check {
                                                    { checkbox_mark(selected) }
                                                } else {
                                                    { radio_mark(selected) }
                                                }
                                            }
                                            span { "{row.label}" }
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
}
