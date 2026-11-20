//! The flat lists' sort control (gui-flat-list-sort design D7): one
//! segmented control over the carried [`GuiListSortFrame`], so the five flat
//! lists cannot drift in how a sort is picked. The core owns the choices,
//! their paint order, and their labels; the window owns nothing but the press.
//!
//! The control is pointer-only and registers no key: a press calls
//! [`LibraryState::select_list_sort`], which sends the list-sort command and
//! resets that list's scroll. The list header paints it last on its heading
//! line, so it is the rightmost item there and the placement has one owner.

use dioxus_native::prelude::*;

use mbv::GuiListSortFrame;

use crate::window::library::state::LibraryState;

/// The props of the sort control: the list's carried sort frame alone, so a
/// panel repaint never clones the list's rows for the control.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibrarySortControlProps {
    /// The list's sort: the target a press names, its active sort, and the
    /// sorts the control offers in paint order.
    pub sort: GuiListSortFrame,
}

/// One segment's look: the active one takes the picked-row fill and the
/// heading tone, and every later segment draws the divider its neighbour
/// needs.
fn segment_style(active: bool, position: usize) -> String {
    let tone = if active {
        "background: var(--canvas-selected); color: var(--canvas-heading); font-weight: 600;"
    } else {
        "background: none; color: var(--canvas-muted); font-weight: 400;"
    };
    let divider = if position == 0 {
        ""
    } else {
        " border-left: 1px solid var(--canvas-border);"
    };
    format!(
        "height: 100%; padding: 0 10px; border: 0; cursor: pointer; font-size: 12px;{divider}{tone}"
    )
}

/// The sort control: one segment per choice of the carried frame.
#[component]
pub fn LibrarySortControl(props: LibrarySortControlProps) -> Element {
    let library = consume_context::<LibraryState>();
    let frame = props.sort;
    let target = frame.target;

    rsx! {
        div {
            class: "flex flex-none items-center",
            role: "radiogroup",
            "aria-label": "Sort",
            "data-library-sort-control": "",
            style: "height: 28px; border-radius: 6px; border: 1px solid var(--canvas-border); overflow: hidden;",
            for (position, choice) in frame.choices.iter().copied().enumerate() {
                {
                    let active = choice == frame.active;
                    let press_library = library.clone();
                    rsx! {
                        button {
                            "type": "button",
                            role: "radio",
                            "aria-checked": "{active}",
                            "aria-label": "Sort by {choice.label()}",
                            style: segment_style(active, position),
                            "data-library-sort-choice": choice.label(),
                            onclick: move |_| press_library.select_list_sort(target, choice),
                            "{choice.label()}"
                        }
                    }
                }
            }
        }
    }
}
