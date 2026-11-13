//! Headless tests of the typed text painter (D16): the contracts of
//! `TypedText.test.ts` — the token each kind maps to, and the repeat
//! opacity only a repeated time or date paints. The test asserts the token
//! each kind maps to, never a rendered colour: the headless document holds
//! no palette.

use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{TypedText, kind_name};
use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};

/// The typed line's props: the segments and the repeat flag.
type TypedProps = (Vec<GuiMetaSegmentFrame>, bool);

/// The root: paints one typed line over the props.
fn typed_app(props: TypedProps) -> Element {
    let (segments, repeat) = props;
    rsx! {
        TypedText { segments, repeat }
    }
}

/// A document painting one typed line over `segments` and `repeat`.
fn built(segments: Vec<GuiMetaSegmentFrame>, repeat: bool) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(typed_app, (segments, repeat)),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// The `style` attribute of the first node the selector names.
fn style_of(doc: &DioxusDocument, selector: &str) -> String {
    use blitz_dom::{Document as _, local_name};
    let dom = doc.inner();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    dom.get_node(id)
        .expect("the node is in the document")
        .attr(local_name!("style"))
        .map(str::to_owned)
        .unwrap_or_default()
}

/// Contract (`TypedText.test.ts`): each kind paints its `--canvas-type-*`
/// token; a plain run inherits the line's own colour.
#[rstest]
#[case(GuiMetaKindFrame::Plain, "inherit")]
#[case(GuiMetaKindFrame::Time, "var(--canvas-type-time)")]
#[case(GuiMetaKindFrame::Date, "var(--canvas-type-date)")]
#[case(GuiMetaKindFrame::Count, "var(--canvas-type-count)")]
#[case(GuiMetaKindFrame::Label, "var(--canvas-type-label)")]
#[case(GuiMetaKindFrame::Secondary, "var(--canvas-type-secondary)")]
#[case(GuiMetaKindFrame::Role, "var(--canvas-type-role)")]
fn a_segment_paints_its_type_token(#[case] kind: GuiMetaKindFrame, #[case] token: &str) {
    let doc = built(
        vec![GuiMetaSegmentFrame {
            text: "x".to_owned(),
            kind,
        }],
        false,
    );

    let style = style_of(&doc, &format!("[data-meta-kind=\"{}\"]", kind_name(kind)));
    assert!(style.contains(&format!("color: {token}")), "{style}");
}

/// Contract (`TypedText.test.ts`): on a repeated line, only a time or a date
/// paints the repeat opacity; the other kinds keep full strength.
#[rstest]
#[case(GuiMetaKindFrame::Time, true)]
#[case(GuiMetaKindFrame::Date, true)]
#[case(GuiMetaKindFrame::Count, false)]
#[case(GuiMetaKindFrame::Plain, false)]
fn a_repeated_segment_paints_the_repeat_opacity(
    #[case] kind: GuiMetaKindFrame,
    #[case] faded: bool,
) {
    let doc = built(
        vec![GuiMetaSegmentFrame {
            text: "x".to_owned(),
            kind,
        }],
        true,
    );

    let style = style_of(&doc, &format!("[data-meta-kind=\"{}\"]", kind_name(kind)));
    assert_eq!(
        style.contains("opacity: var(--canvas-type-repeat-opacity)"),
        faded,
        "{style}"
    );
}
