//! The checkbox box the checkbox rows share: the square that leads a checkbox
//! row — an outline while unchecked, the fill with a check mark while checked.
//! The Library collection dropdown's rows and the Settings checkbox grid's
//! boxes paint the same shape in their own looks (the dropdown's 14 px square,
//! the grid's 16 px mark), so the element, the style, and the mark's paint
//! come in as props and each call site keeps its painted attributes byte for
//! byte.

use dioxus_native::prelude::*;

use crate::window::icons::Check;

/// The element the box paints: the dropdown's rows paint a `div`, the
/// Settings grid's boxes a `span` (`aria-hidden` decoration inside the
/// `button`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxBoxElement {
    /// A `div`, the dropdown row's leading checkbox.
    Div,
    /// A `span`, the Settings grid box's decorative mark holder.
    Span,
}

/// The props of the checkbox box.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct CheckboxBoxProps {
    /// Whether the box carries the check mark.
    pub checked: bool,
    /// The element the box paints.
    pub element: CheckboxBoxElement,
    /// The box's inline style: its size, border, radius, and fill — each
    /// site's own look.
    pub style: String,
    /// The check mark's size in CSS pixels.
    pub size: u16,
    /// The check mark's stroke width.
    pub stroke_width: f32,
    /// The check mark's colour.
    pub color: String,
    /// Whether the box is decoration: paints `aria-hidden="true"` (the
    /// Settings grid's mark inside its `button`).
    #[props(default)]
    pub aria_hidden: bool,
    /// Whether the box paints the Library collection dropdown's
    /// `data-library-collections-checkbox` name.
    #[props(default)]
    pub collections_marker: bool,
}

/// One checkbox box: the square that leads a checkbox row, with the check
/// mark while checked.
#[component]
pub fn CheckboxBox(props: CheckboxBoxProps) -> Element {
    let CheckboxBoxProps {
        checked,
        element,
        style,
        size,
        stroke_width,
        color,
        aria_hidden,
        collections_marker,
    } = props;
    match element {
        CheckboxBoxElement::Div => rsx! {
            div {
                class: "flex flex-none items-center justify-center",
                style: "{style}",
                "data-library-collections-checkbox": collections_marker.then_some(""),
                "aria-hidden": aria_hidden.then_some("true"),
                if checked {
                    Check { size: size, stroke_width: stroke_width, color: color }
                }
            }
        },
        CheckboxBoxElement::Span => rsx! {
            span {
                class: "flex flex-none items-center justify-center",
                style: "{style}",
                "data-library-collections-checkbox": collections_marker.then_some(""),
                "aria-hidden": aria_hidden.then_some("true"),
                if checked {
                    Check { size: size, stroke_width: stroke_width, color: color }
                }
            }
        },
    }
}
