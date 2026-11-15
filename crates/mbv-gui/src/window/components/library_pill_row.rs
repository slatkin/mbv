//! The Library pill row (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryPillRow`; add-gui-library-panel design D6, D12): the
//! canvas's single-select pill form, which every destination that has a
//! selector row paints — the Music collections, the Shows modes, the Emby
//! library's folders, the Podcasts views, and the Feeds views. The row is
//! one base component so a destination cannot invent a second pill look:
//! each destination passes its carried pills, the painted choice comes from
//! the core, and a press reports the pill's stable key to that destination's
//! command.
//!
//! `pill_attr` names the destination's own attribute (`'shows'` paints
//! `data-library-shows-pill`), so a destination's tests keep a per-tab hook
//! beside the shared `data-library-pill`.

use dioxus_native::prelude::*;

use crate::window::components::library_attr::library_attr;
use crate::window::components::library_pill_button::LibraryPillButton;

/// One painted pill: its stable key (the press names it), its name, and
/// whether the core carries it as the selected one (the former client
/// `LibraryPill` interface).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryPill {
    /// The pill's stable key: the press names it, and the focus marks it.
    pub key: String,
    /// The pill's label.
    pub name: String,
    /// Whether the core carries the pill as the selected one.
    pub selected: bool,
}

/// The props of the pill row (the `LibraryPillRow` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryPillRowProps {
    /// The row's accessible name, naming what the pills choose between.
    pub aria_label: &'static str,
    /// The pills in painted order.
    pub pills: Vec<LibraryPill>,
    /// The pill the keys' focus holds, which paints the accent outline
    /// (design D6), or `None` while no pill has it.
    #[props(default)]
    pub focused: Option<String>,
    /// The destination's own pill attribute suffix, or `None` where the
    /// shared `data-library-pill` is enough.
    #[props(default)]
    pub pill_attr: Option<&'static str>,
    /// Press a pill: the destination sends its command.
    pub onpress: Callback<String>,
}

impl std::fmt::Debug for LibraryPillRowProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryPillRowProps")
            .field("aria_label", &self.aria_label)
            .field("pills", &self.pills)
            .field("focused", &self.focused)
            .field("pill_attr", &self.pill_attr)
            .finish_non_exhaustive()
    }
}

/// The destination's own pill attributes for one pill: the per-tab hook beside the shared pill hook, and the focused
/// mark only while the keys' focus holds the pill.
fn pill_attrs(
    pill_attr: Option<&'static str>,
    pill: &LibraryPill,
    focused: Option<&str>,
) -> Vec<Attribute> {
    let is_focused = focused == Some(pill.key.as_str());
    let mut attrs = Vec::new();
    if let Some(attr) = pill_attr {
        attrs.push(Attribute::new(
            library_attr(attr, "pill"),
            pill.key.clone(),
            None,
            false,
        ));
    }
    attrs.push(Attribute::new(
        library_attr("", "pill"),
        pill.key.clone(),
        None,
        false,
    ));
    if is_focused {
        attrs.push(Attribute::new(
            library_attr("", "pill-focused"),
            "",
            None,
            false,
        ));
    }
    attrs
}

/// The Library pill row: the
/// pills region the arrangement paints, one button per carried pill.
#[component]
pub fn LibraryPillRow(props: LibraryPillRowProps) -> Element {
    let pill_attr = props.pill_attr;
    let focused = props.focused.clone();
    let on_press = props.onpress;

    rsx! {
        div {
            class: "flex flex-none items-center overflow-hidden",
            role: "group",
            "aria-label": "{props.aria_label}",
            "data-library-region": "pills",
            style: "height: 52px; gap: 6px; padding: 0 16px; border-bottom: 1px solid var(--canvas-border);",
            for pill in props.pills.iter() {
                {
                    let key = pill.key.clone();
                    let attrs = pill_attrs(pill_attr, pill, focused.as_deref());
                    let selected = pill.selected;
                    let name = pill.name.clone();
                    rsx! {
                        LibraryPillButton {
                            name: name,
                            selected: selected,
                            focused: focused.as_deref() == Some(key.as_str()),
                            attrs: attrs,
                            onpress: move |()| on_press.call(key.clone()),
                        }
                    }
                }
            }
        }
    }
}
