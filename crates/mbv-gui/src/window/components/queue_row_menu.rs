//! The queue row's menu (`replace-tauri-with-blitz` task 5.5, the port of
//! `QueueRowMenu`; design D15): the frame's `rowMenu` overlay entries,
//! each with the key hint that selects it directly, and the core's cursor row
//! highlighted. The window holds no open, cursor, or hover state; a press on
//! an entry forwards like every other pointer event and the core resolves it
//! against the hit map, Esc and the hint chords resolve in the core's overlay
//! key rules. The Library Hero's item menu paints the same face
//! (add-gui-library-panel design D8) and will need the former component's
//! optional `onSelect` prop then; no `click` event reaches a Blitz handler
//! (`dioxus-native-dom` does not convert it), so this port paints the queue's
//! presentation-only menu and no press callback.
//!
//! The menu paints the Menus artboard's chrome (align-gui-with-design-
//! canvas B.2): a 230 px card over its anchor row's bottom edge, the
//! canvas's fill, border, radius, shadow, 6 px padding and 2 px gap, 34 px
//! entries at 13 px, and the muted mono hints. The destructive entry (the
//! projected flag, design DR6) paints in the canvas's tone under a divider.
//! The hint glyph is a paint-time map (design DR7): the core keeps the raw
//! `Enter` chord that `direct_select` matches.
//!
//! The component is presentation-only: `QueuePanel` measures the
//! `[data-menu-entry]` rows through their mounted handles and reports their
//! `rowMenuEntry` regions under its own second report key, inserted when the
//! menu opens — the same technique `TrackChips` uses for its popover rows —
//! so the entries land after the queue's regions in the merged report and
//! win the hit map where the menu paints over a queue row beneath it (the
//! hit map's last push wins). Reporting from the parent, not from this
//! child's mount effect, keeps that order even when the first painted frame
//! already carries the open menu: a child's mount effect runs before the
//! parent's, so a child-held key would land first and the entries would lose
//! to the rows beneath them.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiRowMenuEntry;
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::components::row_handle::record_row_handle;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the row menu (the `QueueRowMenu` props, in order):
/// the Queue panel passes the core's cursor and anchors the card at its
/// value column; the Library's right-click item menu passes no cursor, its
/// own pointer position, and the press callback that sends its command.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueRowMenuProps {
    /// The menu cursor the core holds: the entry at this index paints the
    /// selected fill. `None` for a menu the window owns (the Library's
    /// item menu), whose entries the core holds no cursor over.
    #[props(default)]
    pub cursor: Option<usize>,
    /// The overlay's entries, in the frame's order.
    pub entries: Vec<GuiRowMenuEntry>,
    /// The menu's top edge as a CSS length, measured from the containing
    /// block's top edge; the parent places it over the anchor row's bottom
    /// edge, as the canvas draws it.
    pub top: String,
    /// The menu's left edge from the containing block's left edge, for a
    /// menu the window anchors at a pointer position (the Library's
    /// right-click item menu). When it is set, `right` stays unset.
    #[props(default)]
    pub left: Option<String>,
    /// The menu's right edge from the containing block's right edge: the
    /// queue anchors it to its value column, the Library's Hero menu to its
    /// actions row's right edge.
    #[props(default)]
    pub right: Option<String>,
    /// The press on an entry, for a menu the window owns: the Library's
    /// item menu sends its own command. The queue's menu passes none, so its
    /// presses reach the core's hit map as every other pointer event does.
    #[props(default)]
    pub on_select: Option<Callback<usize>>,
    /// The menu's accessible name; a front-end menu names its own list (the
    /// Settings pick's values, the former client `label` prop that defaulted to
    /// `Row actions`).
    #[props(default = "Row actions".to_owned())]
    pub label: String,
    /// The painted rows' mounted events, by their entry index — the handles
    /// the parent's menu-entry regions read.
    pub row_slot: Signal<Vec<Option<Event<MountedData>>>>,
}

/// The painted hint: the canvas draws `⏎` where the core's hint chord is
/// the `Enter` key name. The map is paint-only, so the core's
/// `direct_select` keeps matching the raw `Enter` string and menu key
/// routing follows the configured bindings unchanged (design DR7).
fn paint_hint(hint: &str) -> &str {
    if hint == "Enter" { "⏎" } else { hint }
}

/// The row menu:
/// the 230 px card in the canvas chrome, one 34 px `menuitem` per entry with
/// its muted mono hint, the cursor row filled, and the destructive entry in
/// the canvas's destructive tone under a divider.
#[component]
pub fn QueueRowMenu(props: QueueRowMenuProps) -> Element {
    // The card's horizontal anchor:
    // a front-end menu sits at the pointer's left edge; the queue's sits at
    // its value column and the Hero's at its actions row's right edge. The
    // the former client paints `right` from its default whenever the parent passes no
    // `left`, so the port paints the same three styles.
    let mut position = format!("top: {};", props.top);
    if let Some(left) = props.left.as_ref() {
        let _ = write!(position, " left: {left};");
    }
    let _ = write!(
        position,
        " right: {};",
        props.right.as_deref().unwrap_or("var(--mbv-cell-col)")
    );
    rsx! {
        div {
            class: "absolute z-20 flex flex-col",
            role: "menu",
            "aria-label": "{props.label}",
            style: format!(
                "{position} width: 230px; padding: 6px; gap: 2px; {MENU_CARD_STYLE}"
            ),
            "data-row-menu": "",
            for (index, entry) in props.entries.iter().enumerate() {
                {
                    let cursor = props.cursor;
                    let on_select = props.on_select;
                    let row_style = format!(
                        "height: 34px; padding: 0 10px; border-radius: 4px; font-size: 13px; background: {}; color: {};",
                        if cursor == Some(index) {
                            "var(--canvas-selected)"
                        } else {
                            "none"
                        },
                        if entry.destructive {
                            "var(--canvas-destructive)"
                        } else if cursor == Some(index) {
                            "var(--canvas-heading)"
                        } else {
                            "var(--canvas-text)"
                        },
                    );
                    let label = entry.label.clone();
                    let hint = entry.key_hint.as_deref().map(paint_hint);
                    let entry_index = entry.index.unwrap_or(index);
                    let row_slot = props.row_slot;
                    rsx! {
                        if entry.destructive {
                            // The canvas's divider above the destructive entry.
                            div {
                                style: "height: 1px; margin: 4px 0; background: var(--canvas-menu-border);",
                                "data-menu-divider": "",
                            }
                        }
                        button {
                            "type": "button",
                            class: "flex items-center justify-between text-left",
                            role: "menuitem",
                            style: "{row_style}",
                            // The entry's own index in the shared builder's
                            // order, when the frame carries one (the
                            // Library's item menu), else the paint order: the
                            // hook and the press name the same entry.
                            "data-menu-entry": "{entry_index}",
                            onclick: move |_| {
                                // A front-end menu's press sends the entry's
                                // index through the callback; the queue's
                                // menu has none, so the press reaches the
                                // core's hit map as every other press does.
                                if let Some(on_select) = on_select {
                                    on_select.call(entry_index);
                                }
                            },
                            onmounted: move |event| {
                                record_row_handle(row_slot, index, event);
                            },
                            // The 13 px the row's style sets is the label's inherited
                            // size and weight: the box carries no style of its own.
                            Truncate {
                                text: label,
                                class: "flex-1".to_owned(),
                                style: TextStyle::sans(13.0, 400.0),
                            }
                            if let Some(hint) = hint {
                                span {
                                    class: "shrink-0 font-mono tabular-nums",
                                    style: "font-size: 11px; color: var(--canvas-muted);",
                                    "{hint}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
