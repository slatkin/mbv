//! The two shapes every chooser in the window repeats (change
//! `gui-shared-chooser-menu`, design D1–D3): the card that hangs under an
//! anchor, and the row it paints.
//!
//! `ChooserMenu` owns the card's chrome and nothing else: the drop under the
//! anchor, the width bounds, the raised `Bg2` fill, the `Bg4` border, the
//! radius, the padding and row gap, the transparent hover veil (the card
//! answers the pointer with the row fills alone), the menu role, and the
//! kind's `data-chooser-menu` marker. `ChooserRow` owns one row's chrome and
//! its half of the interaction boundary: the button's menu-item role, the
//! queue rows' height and inset, the cursor's `Bg3` fill, the busy look, the
//! `data-chooser-entry` marker its owner names, and the mount-time call to the
//! mark the owner passes down. A row's content stays the caller's `children`,
//! so the Queue entries, the target choices, and the Refresh row each keep the
//! paint they own.
//!
//! The look moves here; the interaction does not. The core keeps the cursor,
//! and the chooser's owner keeps its region set, its key and press dispatch,
//! and the hit map order — it builds one mark per painted row and passes it as
//! a prop, exactly as it did before. Neither component reads the frame, a
//! cursor source, or a region set, so sharing the shape leaves interaction
//! authority where it was (invariant 38 rule 5).

use dioxus_native::prelude::*;

use crate::window::components::queue_panel::geometry::{MENU_DROP_PX, ROW_GAP_PX, ROW_HEIGHT_PX};
use crate::window::layout::region_set::MarkHandler;
use crate::window::theme::{EfToken, ef_var};

/// The card's padding: one inset on every side, so a row's fill stops short of
/// the border.
const CARD_PADDING_PX: u16 = 4;

/// The chooser a card belongs to. It picks the card's `data-chooser-menu`
/// marker only; the two cards paint the same chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChooserKind {
    /// The Queue switcher's dropdown.
    Queue,
    /// The playback-target control's popover.
    Target,
}

impl ChooserKind {
    /// The card's `data-chooser-menu` value. The stylesheet keys the Queue
    /// chooser's pointer fill on it, so the value is the owner's identity, not
    /// a look.
    const fn marker(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Target => "target",
        }
    }
}

/// The props of one chooser card: its kind, its accessible name when it has
/// one, and its rows.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct ChooserMenuProps {
    /// The chooser the card belongs to.
    pub kind: ChooserKind,
    /// The card's accessible name, when its owner gives it one. The target
    /// chooser passes none.
    #[props(default)]
    pub label: Option<&'static str>,
    /// The card's rows, in paint order.
    pub children: Element,
}

/// The card every chooser hangs under its anchor: the raised fill, the border,
/// the radius, the padding and gap, and the drop, with the menu role.
#[component]
pub fn ChooserMenu(props: ChooserMenuProps) -> Element {
    let style = format!(
        "top: calc(100% + {MENU_DROP_PX}px); left: 0; min-width: 260px; max-width: 28rem; padding: {CARD_PADDING_PX}px; gap: {ROW_GAP_PX}px; background: {}; border: 1px solid {}; border-radius: 8px; --hover-veil: transparent;",
        ef_var(EfToken::Bg2),
        ef_var(EfToken::Bg4)
    );
    rsx! {
        div {
            class: "absolute z-20 flex flex-col",
            role: "menu",
            style: "{style}",
            "aria-label": props.label,
            "data-chooser-menu": props.kind.marker(),
            {props.children}
        }
    }
}

/// The props of one chooser row: its marker value, its state, its class, its
/// accessible name when it has one, its owner's mark, and its content.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct ChooserRowProps {
    /// The row's `data-chooser-entry` value: the Queue entry's queue id or
    /// `peer`, a target choice's index, or `refresh`.
    pub entry: String,
    /// Whether the core's cursor rests on this row.
    pub cursor: bool,
    /// Whether the row is busy (the Refresh row while a discovery runs): it
    /// paints grey at reduced opacity.
    pub busy: bool,
    /// The row's accessible name, when its owner gives it one.
    #[props(default)]
    pub label: Option<String>,
    /// The row's own flex classes beyond the shared ones — `justify-between`
    /// for a Queue entry, empty for a target row.
    pub class: String,
    /// The mark the row records from `onmounted`, built by the owner that owns
    /// the chooser's region set.
    #[props(default)]
    pub mark: Option<MarkHandler>,
    /// The row's content.
    pub children: Element,
}

/// One row of a chooser: the queue rows' height and inset, the cursor fill,
/// the busy look, and the menu-item role. It hands its mount event to the mark
/// its owner passes and does nothing else with it.
#[component]
pub fn ChooserRow(props: ChooserRowProps) -> Element {
    let color = if props.busy {
        ef_var(EfToken::Grey1)
    } else {
        ef_var(EfToken::Fg)
    };
    let opacity = if props.busy { " opacity: 0.6;" } else { "" };
    let background = if props.cursor {
        format!(" background: {};", ef_var(EfToken::Bg3))
    } else {
        String::new()
    };
    let style = format!(
        "height: {ROW_HEIGHT_PX}px; padding: 0 8px; gap: 8px; border-radius: 5px; font-size: 13px; font-weight: 500; color: {color};{opacity}{background}"
    );
    let mark = props.mark;
    rsx! {
        button {
            "type": "button",
            class: "flex w-full items-center text-left {props.class}",
            role: "menuitem",
            style: "{style}",
            "data-chooser-entry": "{props.entry}",
            "aria-label": props.label,
            onmounted: move |event| {
                // The owner marks this row: the closure records the handle its
                // region reads.
                if let Some(mark) = mark {
                    mark.call(event);
                }
            },
            {props.children}
        }
    }
}
