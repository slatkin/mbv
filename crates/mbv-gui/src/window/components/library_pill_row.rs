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
//!
//! [`library_pills_bound`] is the shared body of the five destination pill
//! components: it paints this row and turns the keys' pill request into the
//! destination's own command.

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
            .field("pill_attr", &self.pill_attr)
            .finish_non_exhaustive()
    }
}

/// The destination's own pill attributes for one pill: the per-tab hook beside
/// the shared pill hook.
fn pill_attrs(pill_attr: Option<&'static str>, pill: &LibraryPill) -> Vec<Attribute> {
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
    attrs
}

/// One destination's pill row, bound to that destination's commands (design D6, D12): the pills it
/// paints, the row's accessible name and its own attribute, the press its command answers, and the
/// pill request its apply step consumes. A destination's component narrows its own carried pills and
/// names its own command; this function owns the row and the request's effect, so the five pill rows
/// cannot drift.
///
/// The effect attaches to the calling destination's scope, as
/// [`library_virtual_list`](crate::window::components::library_virtual_list::library_virtual_list)'s
/// effects do, so a destination calls this exactly once per render.
pub fn library_pills_bound<T, A>(
    pills: Vec<LibraryPill>,
    aria_label: &'static str,
    pill_attr: Option<&'static str>,
    onpress: Callback<String>,
    intent: Memo<Option<T>>,
    apply: A,
) -> Element
where
    T: Clone + PartialEq + 'static,
    A: Fn() + 'static,
{
    // The keys' pill request becomes the command a press sends (design D6): the effect reads the
    // intent through the memo, so the consume-write of the state never re-runs an effect that
    // subscribes to it — the memo settles at `None`, and the re-run finds nothing to send.
    use_effect(move || {
        if intent.cloned().is_some() {
            apply();
        }
    });

    rsx! {
        LibraryPillRow {
            aria_label,
            pills,
            pill_attr,
            onpress,
        }
    }
}

/// The Library pill row: the
/// pills region the arrangement paints, one button per carried pill.
#[component]
pub fn LibraryPillRow(props: LibraryPillRowProps) -> Element {
    let pill_attr = props.pill_attr;
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
                    let attrs = pill_attrs(pill_attr, pill);
                    let selected = pill.selected;
                    let name = pill.name.clone();
                    rsx! {
                        LibraryPillButton {
                            name: name,
                            selected: selected,
                            attrs: attrs,
                            onpress: move |()| on_press.call(key.clone()),
                        }
                    }
                }
            }
        }
    }
}
