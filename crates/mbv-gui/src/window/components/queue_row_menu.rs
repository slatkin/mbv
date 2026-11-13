//! The queue row's menu (`replace-tauri-with-blitz` task 5.5, the port of
//! `QueueRowMenu.svelte`; design D15): the frame's `rowMenu` overlay entries,
//! each with the key hint that selects it directly, and the core's cursor row
//! highlighted. The window holds no open, cursor, or hover state; a press on
//! an entry forwards like every other pointer event and the core resolves it
//! against the hit map, Esc and the hint chords resolve in the core's overlay
//! key rules. The Library Hero's item menu paints the same face
//! (add-gui-library-panel design D8) and will need the Svelte component's
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

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiRowMenuEntry;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the row menu (the `QueueRowMenu.svelte` props the Queue panel
/// passes; the Library's `left`, `onSelect`, and `label` call-site values land
/// with the Library's item menu).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueRowMenuProps {
    /// The menu cursor the core holds: the entry at this index paints the
    /// selected fill.
    pub cursor: usize,
    /// The overlay's entries, in the frame's order.
    pub entries: Vec<GuiRowMenuEntry>,
    /// The menu's top edge as a CSS length, measured from the containing
    /// block's top edge; the parent places it over the anchor row's bottom
    /// edge, as the canvas draws it.
    pub top: String,
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

/// The row menu (the Svelte component's markup, character for character):
/// the 230 px card in the canvas chrome, one 34 px `menuitem` per entry with
/// its muted mono hint, the cursor row filled, and the destructive entry in
/// the canvas's destructive tone under a divider.
#[component]
pub fn QueueRowMenu(props: QueueRowMenuProps) -> Element {
    rsx! {
        div {
            class: "absolute z-20 flex flex-col",
            role: "menu",
            "aria-label": "Row actions",
            style: "top: {props.top}; right: var(--mbv-cell-col); width: 230px; padding: 6px; gap: 2px; background: var(--canvas-menu); border: 1px solid var(--canvas-menu-border); border-radius: 8px; box-shadow: var(--canvas-menu-shadow);",
            "data-row-menu": "",
            for (index, entry) in props.entries.iter().enumerate() {
                {
                    let cursor = props.cursor;
                    let row_style = format!(
                        "height: 34px; padding: 0 10px; border-radius: 4px; font-size: 13px; background: {}; color: {};",
                        if index == cursor {
                            "var(--canvas-selected)"
                        } else {
                            "none"
                        },
                        if entry.destructive {
                            "var(--canvas-destructive)"
                        } else if index == cursor {
                            "var(--canvas-heading)"
                        } else {
                            "var(--canvas-text)"
                        },
                    );
                    let label = entry.label.clone();
                    let hint = entry.key_hint.as_deref().map(paint_hint);
                    let mut row_slot = props.row_slot;
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
                            "data-menu-entry": "{index}",
                            onmounted: move |event| {
                                let mut rows = row_slot.write();
                                if rows.len() <= index {
                                    rows.resize(index + 1, None);
                                }
                                rows[index] = Some(event);
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
