//! Typed text (add-gui-settings-screen design D14, the port of
//! `TypedText.svelte`): colour carries the data type, so a line that joins
//! values of several types is built as typed segments where it is built and
//! painted here, never split in the window. Each kind names one
//! `--canvas-type-*` token; the caller keeps the weight, the size, and the
//! role of the line, and a plain segment inherits them.
//!
//! A list repeats the same types down its rows, so the caller sets `repeat`
//! and a time or a date paints at the repeat opacity. A row that is selected
//! or current keeps its own ink at full strength, so it paints without
//! `repeat`.
//!
//! (Ported with task 5.5, which the Queue panel's right column needed; the
//! contracts of `TypedText.test.ts` travel with task 5.6.)

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};

/// The props of one typed line (the `TypedText.svelte` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct TypedTextProps {
    /// The line's typed runs, in display order.
    pub segments: Vec<GuiMetaSegmentFrame>,
    /// Whether the line repeats down a list: a time or a date then paints at
    /// the repeat opacity.
    #[props(default)]
    pub repeat: bool,
}

/// The token each kind paints; a plain run inherits the line's own colour.
fn token(kind: GuiMetaKindFrame) -> &'static str {
    match kind {
        GuiMetaKindFrame::Plain => "inherit",
        GuiMetaKindFrame::Time => "var(--canvas-type-time)",
        GuiMetaKindFrame::Date => "var(--canvas-type-date)",
        GuiMetaKindFrame::Count => "var(--canvas-type-count)",
        GuiMetaKindFrame::Label => "var(--canvas-type-label)",
        GuiMetaKindFrame::Secondary => "var(--canvas-type-secondary)",
        GuiMetaKindFrame::Role => "var(--canvas-type-role)",
    }
}

/// The wire name a kind paints in `data-meta-kind`, as the kind's camelCase
/// wire name.
fn kind_name(kind: GuiMetaKindFrame) -> &'static str {
    match kind {
        GuiMetaKindFrame::Plain => "plain",
        GuiMetaKindFrame::Time => "time",
        GuiMetaKindFrame::Date => "date",
        GuiMetaKindFrame::Count => "count",
        GuiMetaKindFrame::Label => "label",
        GuiMetaKindFrame::Secondary => "secondary",
        GuiMetaKindFrame::Role => "role",
    }
}

/// Whether one segment paints at the repeat opacity: only a repeated time or
/// date fades (design D14).
fn repeats(repeat: bool, kind: GuiMetaKindFrame) -> bool {
    repeat && matches!(kind, GuiMetaKindFrame::Time | GuiMetaKindFrame::Date)
}

/// One typed line: one `span` per segment, each painted in its kind's token
/// colour and, on a repeated line, a time or date at the repeat opacity.
#[component]
pub fn TypedText(props: TypedTextProps) -> Element {
    let repeat = props.repeat;
    rsx! {
        for (index, segment) in props.segments.iter().enumerate() {
            {
                let kind = segment.kind;
                let mut style = format!("color: {};", token(kind));
                if repeats(repeat, kind) {
                    style.push_str(" opacity: var(--canvas-type-repeat-opacity);");
                }
                rsx! {
                    span {
                        key: "{index}",
                        "data-meta-kind": "{kind_name(kind)}",
                        style: "{style}",
                        "{segment.text}"
                    }
                }
            }
        }
    }
}
