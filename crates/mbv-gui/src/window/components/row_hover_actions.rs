//! The rows' hover controls (gui-row-hover-actions design D1, D2): one part
//! paints Play, Enqueue, Shuffle, and Expand/collapse in that fixed order
//! at the trailing end of a Library row, so every row surface shows the same
//! controls in the same order.
//!
//! The part holds no state and owns no behaviour. A row painter passes one
//! callback per control it offers — `None` for a control its row does not
//! have, and only the controls with a callback paint — plus the node's open
//! state, which picks the toggle's glyph: [`ChevronDown`] while the node is
//! closed and a press expands it, [`ChevronUp`] while it is open and a press
//! collapses it. Each control runs the same path as the row's gesture or its
//! right-click menu entry (design D4), so the part adds no new action.
//!
//! The part is a sibling of the row's `<button>`, never a child: the row
//! button keeps its own semantics, as the queue's controls do. A press or a
//! double-click on a control stops at the control, so it never reaches the
//! row's pick or its double-click. The reveal is `assets/app.css`'s work on
//! the row host's `:hover` (no window hover state, no rest delay): the part
//! rests hidden and paints the row's own fill (`--row-fill`) behind it while
//! it covers the row's trailing text.

use dioxus_native::prelude::*;

use crate::window::icons::{ChevronDown, ChevronUp, Play, Plus, Shuffle};

/// The square one hover control paints, in CSS pixels: the Queue row's own Play
/// control reserves the same square (design D6).
pub(crate) const ROW_HOVER_ACTION_PX: u16 = 24;

/// The glyph size of one control, in CSS pixels.
pub(crate) const ROW_HOVER_ACTION_GLYPH_PX: u16 = 14;

/// The look of one hover control (design D6: the Queue row's Play control
/// paints the same one): a square ghost button in the row's heading tone, with
/// no fill of its own, so the shared pointer veil answers the pointer over it.
/// The square comes from [`ROW_HOVER_ACTION_PX`], so the Queue row's own sizing
/// of the control cannot drift from the look.
#[must_use]
pub(crate) fn action_style() -> String {
    format!(
        "width: {ROW_HOVER_ACTION_PX}px; height: {ROW_HOVER_ACTION_PX}px; padding: 0; background: none; border: 0; border-radius: 4px; color: var(--canvas-heading); cursor: pointer;"
    )
}

/// The props of the hover controls: one callback per control the row offers,
/// and the node's open state for the toggle's glyph.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct RowHoverActionsProps {
    /// Play the row; `None` on a row that cannot play.
    #[props(default)]
    pub on_play: Option<Callback<()>>,
    /// Add the row to the queue; `None` on a row that offers no Add.
    #[props(default)]
    pub on_queue: Option<Callback<()>>,
    /// Shuffle the row's items; `None` on a row that is no collection.
    #[props(default)]
    pub on_shuffle: Option<Callback<()>>,
    /// Open or close the row's node; `None` on a row that is no tree parent.
    #[props(default)]
    pub on_toggle: Option<Callback<()>>,
    /// Whether the row's node is open: it picks the toggle's glyph, so the
    /// control shows which of the two a press does now.
    #[props(default)]
    pub open: bool,
    /// The row's own name: when it is set, the Play control's accessible name
    /// composes as `Play {row_name}`, so a list of rows exposes a distinct Play
    /// control per row. `None` leaves the plain `Play`.
    #[props(default)]
    pub row_name: Option<String>,
}

/// One control's button: the shared look, the control's label and hook, and
/// the press that stops at the control before it runs `onpress`.
fn action_button(
    action: &'static str,
    label: &str,
    style: &str,
    onpress: Callback<()>,
    glyph: Element,
) -> Element {
    rsx! {
        button {
            "type": "button",
            class: "flex items-center justify-center",
            "aria-label": "{label}",
            title: "{label}",
            style: "{style}",
            "data-row-action": "{action}",
            onclick: move |event: MouseEvent| {
                event.stop_propagation();
                onpress.call(());
            },
            ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
            {glyph}
        }
    }
}

/// The rows' hover controls: Play, Enqueue, Shuffle, and Expand/collapse,
/// each painted only while its callback is set.
#[component]
pub fn RowHoverActions(props: RowHoverActionsProps) -> Element {
    let on_play = props.on_play;
    let on_queue = props.on_queue;
    let on_shuffle = props.on_shuffle;
    let on_toggle = props.on_toggle;
    let open = props.open;
    // The Play control's accessible name: the row's own name when the painter
    // passed one, so `N` rows never expose `N` identical Play buttons.
    let play_label = props
        .row_name
        .as_deref()
        .map_or_else(|| "Play".to_owned(), |name| format!("Play {name}"));
    // One look for every control the part paints, built once per paint.
    let style = action_style();

    rsx! {
        div {
            class: "row-hover-actions",
            role: "group",
            "aria-label": "Row actions",
            "data-row-hover-actions": "",
            if let Some(on_play) = on_play {
                {
                    action_button(
                        "play",
                        &play_label,
                        &style,
                        on_play,
                        rsx! {
                            Play { size: ROW_HOVER_ACTION_GLYPH_PX, fill: "currentColor".to_owned(), stroke_width: 0.0 }
                        },
                    )
                }
            }
            if let Some(on_queue) = on_queue {
                {
                    action_button(
                        "queue",
                        "Enqueue",
                        &style,
                        on_queue,
                        rsx! {
                            Plus { size: ROW_HOVER_ACTION_GLYPH_PX }
                        },
                    )
                }
            }
            if let Some(on_shuffle) = on_shuffle {
                {
                    action_button(
                        "shuffle",
                        "Shuffle",
                        &style,
                        on_shuffle,
                        rsx! {
                            Shuffle { size: ROW_HOVER_ACTION_GLYPH_PX }
                        },
                    )
                }
            }
            if let Some(on_toggle) = on_toggle {
                {
                    let label = if open { "Collapse" } else { "Expand" };
                    action_button(
                        "toggle",
                        label,
                        &style,
                        on_toggle,
                        rsx! {
                            if open {
                                ChevronUp { size: ROW_HOVER_ACTION_GLYPH_PX }
                            } else {
                                ChevronDown { size: ROW_HOVER_ACTION_GLYPH_PX }
                            }
                        },
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
