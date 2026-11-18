//! Headless tests of the list header's title line (gui-collection-dropdown
//! design D6a, spec "The collection dropdown sits in the list header and no
//! pill row remains"): the title line paints the list title, then the collection
//! dropdown's trigger, then the date order toggle at the line's right end, and
//! the selection label paints only where no trigger names the selection. The
//! core channel is a mock pair; no test opens a window, sleeps, or reaches the
//! core.

use std::sync::{Arc, mpsc};

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryListHeader;
use crate::bridge::frame::library::{GuiCollectionEntry, GuiCollectionsFrame};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The layout rules the fixture resolves with, in place of the app stylesheet a
/// real window loads: the utilities the title line and the dropdown size their
/// boxes with, and the header's own width so the right-aligned toggle has a
/// line to sit at the end of.
const FIXTURE_CSS: &str = concat!(
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".justify-between { justify-content: space-between; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
    ".whitespace-nowrap { white-space: nowrap; }",
    "[data-library-list-header] { width: 600px; }",
);

/// The harness's props: the carried collections.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    collections: Option<GuiCollectionsFrame>,
}

/// The root: provides the Library store and paints the header, as a
/// destination's list composes it.
fn header_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        collections,
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
            title: "Podcasts".to_owned(),
            label: "All".to_owned(),
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

    /// The laid-out client rect of the first node the selector names.
    fn rect_of(&self, selector: &str) -> blitz_dom::BoundingRect {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_client_bounding_rect(id)
            .expect("the node is laid out")
    }
}

/// Build the document over the carried collections and resolve its layout.
fn built(collections: Option<GuiCollectionsFrame>) -> Fixture {
    let (sender, _receiver) = mpsc::channel::<CoreInput>();
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            header_app,
            FixtureProps {
                sender,
                collections,
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

/// The Podcasts frame: All selected, one show, newest first.
fn podcasts_frame() -> GuiCollectionsFrame {
    GuiCollectionsFrame {
        target: mbv::GuiCollectionTarget::Podcasts(0),
        all: true,
        entries: vec![GuiCollectionEntry {
            key: "s1".to_owned(),
            name: "Tifo Football".to_owned(),
            checked: false,
        }],
        noun: mbv::GuiCollectionNoun::Shows,
        order: Some(mbv::GuiDateOrder::Newest),
    }
}

/// Contract (design D6a, spec "Podcasts header"): the title line paints the
/// list title, then the trigger, then the date order toggle, and the toggle sits
/// at the line's right end.
#[test]
fn the_title_line_paints_the_title_the_trigger_then_the_date_order_toggle() {
    let fixture = built(Some(podcasts_frame()));

    assert_eq!(fixture.text_of("[data-library-list-title]"), "Podcasts");
    assert_eq!(fixture.text_of("[data-library-collections-trigger]"), "All");
    assert_eq!(fixture.text_of("[data-library-date-order]"), "Newest");

    let title = fixture.rect_of("[data-library-list-title]");
    let trigger = fixture.rect_of("[data-library-collections-trigger]");
    let toggle = fixture.rect_of("[data-library-date-order]");
    assert!(
        title.x + title.width <= trigger.x,
        "the trigger paints after the title"
    );
    assert!(
        trigger.x + trigger.width <= toggle.x,
        "the toggle paints after the trigger"
    );
    assert!(
        toggle.x + toggle.width > trigger.x + trigger.width,
        "the toggle sits at the line's right end"
    );
}

/// Contract (design D6a): the header paints no selection label while the
/// trigger names the selection itself.
#[test]
fn a_destination_with_collections_paints_no_selection_label() {
    let fixture = built(Some(podcasts_frame()));

    assert!(!fixture.presents("[data-library-selection-label]"));
}

/// Contract (design D6a): a destination without collections keeps the carried
/// selection's label on the title line.
#[test]
fn a_destination_without_collections_paints_its_selection_label() {
    let fixture = built(None);

    assert_eq!(fixture.text_of("[data-library-selection-label]"), "All");
    assert!(!fixture.presents("[data-library-collections-trigger]"));
}
