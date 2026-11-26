//! The Hero's labelled fact rows (gui-hero-design-polish design D2): one row
//! per `GuiHeroFactRowFrame`, in paint order. The label paints the word the
//! label enum names, and the value paints through `TypedText`, so the value's
//! type colour stays as today. The row is the artboard's two-column fact
//! line: the small label word in the label font at the left, the value beside
//! it, one rule under each row.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiHeroFactLabelFrame, GuiHeroFactRowFrame};
use crate::window::components::library_hero_frame::own;
use crate::window::components::typed_text::TypedText;

/// The label column's width, in CSS pixels: the widest label word
/// (`DIRECTOR`) plus the artboard's gap to the value column.
const LABEL_WIDTH_PX: f32 = 110.0;

/// The label word's look: the kicker's label font (design D2), in the
/// artboard's small capitals.
const LABEL_STYLE: &str = "font-family: var(--font-mono); font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--canvas-muted);";

/// The value line's look: today's meta row font (design D2) in the artboard's
/// text ink.
const VALUE_STYLE: &str = "font-size: 13px; color: var(--canvas-text);";

/// The word one row's label paints (design D2: the window paints the label
/// word from the label enum).
fn label_word(label: GuiHeroFactLabelFrame) -> &'static str {
    match label {
        GuiHeroFactLabelFrame::Director => "Director",
        GuiHeroFactLabelFrame::Released => "Released",
        GuiHeroFactLabelFrame::Studio => "Studio",
        GuiHeroFactLabelFrame::Network => "Network",
        GuiHeroFactLabelFrame::Author => "Author",
        GuiHeroFactLabelFrame::Published => "Published",
    }
}

/// The label's wire name, as the former client's data hooks spell it.
fn label_wire(label: GuiHeroFactLabelFrame) -> &'static str {
    match label {
        GuiHeroFactLabelFrame::Director => "director",
        GuiHeroFactLabelFrame::Released => "released",
        GuiHeroFactLabelFrame::Studio => "studio",
        GuiHeroFactLabelFrame::Network => "network",
        GuiHeroFactLabelFrame::Author => "author",
        GuiHeroFactLabelFrame::Published => "published",
    }
}

/// The props of the fact rows (the `LibraryHeroFactRows` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryHeroFactRowsProps {
    /// The labelled rows, in paint order.
    pub rows: Vec<GuiHeroFactRowFrame>,
    /// The destination's attribute prefix: empty for Music, as in `shows` or
    /// `emby` for the rest.
    pub attr: &'static str,
    /// The block's own attribute part, as in `rows`.
    pub hook: &'static str,
}

impl std::fmt::Debug for LibraryHeroFactRowsProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryHeroFactRowsProps")
            .field("rows", &self.rows)
            .field("attr", &self.attr)
            .field("hook", &self.hook)
            .finish()
    }
}

/// The Hero's labelled fact rows.
#[component]
pub fn LibraryHeroFactRows(props: LibraryHeroFactRowsProps) -> Element {
    rsx! {
        div {
            class: "flex min-w-0 flex-col",
            ..vec![Attribute::new(own(props.attr, props.hook), "", None, false)],
            for (index, row) in props.rows.iter().enumerate() {
                {
                    let row_attrs = vec![Attribute::new(
                        own(props.attr, "row"),
                        label_wire(row.label),
                        None,
                        false,
                    )];
                    rsx! {
                        div {
                            key: "{index}",
                            class: "grid items-baseline",
                            style: format!(
                                "grid-template-columns: {LABEL_WIDTH_PX}px minmax(0, 1fr); column-gap: 16px; padding: 7px 0; border-bottom: 1px solid var(--canvas-border);",
                            ),
                            ..row_attrs,
                            span {
                                class: "font-mono",
                                style: "{LABEL_STYLE}",
                                ..vec![Attribute::new(own(props.attr, "row-label"), "", None, false)],
                                "{label_word(row.label)}"
                            }
                            span {
                                class: "font-mono min-w-0",
                                style: "{VALUE_STYLE}",
                                ..vec![Attribute::new(own(props.attr, "row-value"), "", None, false)],
                                TypedText { segments: row.value.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}
