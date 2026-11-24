//! Headless tests of the list header's heading line (gui-collection-dropdown
//! design D6a, user override, spec "The collection dropdown sits in the list
//! header and no pill row remains"; gui-library-header-service-marks design
//! D5, D6): the heading line paints the destination's Service mark or its text
//! title and then the collection dropdown's trigger at the line's right end,
//! and the selection label paints only where it is non-empty and no trigger
//! names the selection. The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::sync::{Arc, mpsc};

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryListHeader, LibraryListHeading};
use crate::bridge::frame::library::{GuiCollectionEntry, GuiCollections};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;
use crate::window::service_mark::ServiceMark;

/// The layout rules the fixture resolves with, in place of the app stylesheet a
/// real window loads: the utilities the heading line and the dropdown size
/// their boxes with, and the header's own width so the right-aligned toggle has
/// a line to sit at the end of.
const FIXTURE_CSS: &str = concat!(
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".justify-between { justify-content: space-between; }",
    ".justify-end { justify-content: flex-end; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
    ".whitespace-nowrap { white-space: nowrap; }",
    "[data-library-list-header] { width: 600px; }",
);

/// The harness's props: the heading, the carried collections, and the
/// selection label.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    heading: LibraryListHeading,
    collections: Option<GuiCollections>,
    label: String,
}

/// The root: provides the Library store and paints the header, as a
/// destination's list composes it.
fn header_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        heading,
        collections,
        label,
    } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let state = use_hook(move || {
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(sender),
            playhead_position,
            Arc::clone(&clock),
        )
    });
    provide_context(state);
    rsx! {
        LibraryListHeader {
            heading,
            label,
            collections,
            filter: String::new(),
            placeholder: "Filter All".to_owned(),
            aria_label: "Filter episodes".to_owned(),
            on_input: move |_| {},
            on_accept: move |()| {},
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
}

impl Fixture {
    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: &str) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(blitz_dom::LocalName::from(attribute))
            .map(str::to_owned)
    }

    /// The trimmed text of the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
            .trim()
            .to_owned()
    }
}

/// Build the document over the heading, the carried collections, and the
/// selection label and resolve its layout.
fn built_with(
    heading: LibraryListHeading,
    collections: Option<GuiCollections>,
    label: String,
) -> Fixture {
    let (sender, _receiver) = mpsc::channel::<CoreInput>();
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            header_app,
            FixtureProps {
                sender,
                heading,
                collections,
                label,
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    doc.inner.borrow_mut().resolve(0.0);
    Fixture { doc }
}

/// Build the document over the carried collections and the selection label,
/// under a Service mark heading.
fn built_with_label(collections: Option<GuiCollections>, label: String) -> Fixture {
    built_with(
        LibraryListHeading::Mark(ServiceMark::Rss),
        collections,
        label,
    )
}

/// Build the document over the carried collections and the default `All`
/// selection label, under a Service mark heading.
fn built(collections: Option<GuiCollections>) -> Fixture {
    built_with_label(collections, "All".to_owned())
}

/// The Podcasts frame: All selected, one show, newest first.
fn podcasts_frame() -> GuiCollections {
    GuiCollections {
        target: crate::gui_core::GuiCollectionTarget::Podcasts(0),
        all: true,
        entries: vec![GuiCollectionEntry {
            key: "s1".to_owned(),
            name: "Tifo Football".to_owned(),
            checked: false,
        }],
        noun: crate::gui_core::GuiCollectionNoun::Shows,
    }
}

/// Contract (design D6a, user override; gui-library-header-service-marks
/// design D5): the heading line paints the destination's Service mark and the
/// collections trigger, and no text title. Where the trigger sits on the line
/// is design presentation, not a contract: no test asserts geometry (user
/// decision 2026-11-19).
#[test]
fn the_heading_line_paints_the_mark_and_the_trigger() {
    let fixture = built(Some(podcasts_frame()));

    assert_eq!(
        fixture.attr_of("[data-library-list-mark]", "data-library-list-mark"),
        Some("rss".to_owned())
    );
    assert!(!fixture.presents("[data-library-list-title]"));
    assert_eq!(fixture.text_of("[data-library-collections-trigger]"), "All");
}

/// Contract (gui-library-header-service-marks design D5, D6): a `Title`
/// heading paints its text and no Service mark, as Home does.
#[test]
fn a_title_heading_paints_the_title_and_no_mark() {
    let fixture = built_with(
        LibraryListHeading::Title("Home".to_owned()),
        None,
        "All".to_owned(),
    );

    assert_eq!(fixture.text_of("[data-library-list-title]"), "Home");
    assert!(!fixture.presents("[data-library-list-mark]"));
}

/// Contract (design D6a): the header paints no selection label while the
/// trigger names the selection itself.
#[test]
fn a_destination_with_collections_paints_no_selection_label() {
    let fixture = built(Some(podcasts_frame()));

    assert!(!fixture.presents("[data-library-selection-label]"));
}

/// Contract (design D6a): a destination without collections keeps the carried
/// selection's label on the title line while the label is non-empty.
#[test]
fn a_destination_without_collections_paints_its_selection_label() {
    let fixture = built(None);

    assert_eq!(fixture.text_of("[data-library-selection-label]"), "All");
    assert!(!fixture.presents("[data-library-collections-trigger]"));
}

/// Contract (design D6a, user override): a header whose label is empty paints
/// no selection label span.
#[test]
fn an_empty_label_paints_no_selection_label() {
    let fixture = built_with_label(None, String::new());

    assert!(!fixture.presents("[data-library-selection-label]"));
    assert!(!fixture.presents("[data-library-collections-trigger]"));
}
