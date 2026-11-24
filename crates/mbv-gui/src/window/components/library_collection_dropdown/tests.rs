//! Headless tests of the collection dropdown (gui-collection-dropdown design
//! D6, D7, user override): the trigger names the selection (All, the one
//! checked collection, the count with the destination's noun) in the
//! accent-outlined box with the filter field's corner radius and a chevron
//! that follows the open fact, the open popover paints its checkbox rows with
//! the checked row's band and anchors to the trigger's right edge. The presses
//! ride the state methods the component's own handlers call one line each (a
//! headless document cannot fire a DOM click, the list's and the pill row's
//! precedent). The core channel is a mock pair; no test opens a window, sleeps,
//! or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryCollectionDropdown;
use crate::bridge::frame::library::{ALL_KEY, GuiCollectionEntry, GuiCollections};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The harness's props: the carried collections, the mock core sender, and the
/// handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    collections: GuiCollections,
    handle: Rc<RefCell<Option<LibraryState>>>,
}

/// The root: provides the Library store and paints the dropdown, as the panel
/// composes it.
fn dropdown_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        collections,
        handle,
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
    provide_context(state.clone());
    handle.borrow_mut().replace(state);
    rsx! {
        LibraryCollectionDropdown { collections }
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    receiver: mpsc::Receiver<CoreInput>,
}

impl Fixture {
    /// Poll the document until the effects have run and their re-renders landed
    /// (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Run `f` inside the virtual dom's runtime, so the signal writes mark their
    /// subscribers, and let the re-renders land.
    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

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

    /// The `style` attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
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

    /// The Library commands the core link received, in send order.
    fn library_commands(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the carried collections and let the first render and
/// its effects land.
fn built(collections: GuiCollections) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            dropdown_app,
            FixtureProps {
                sender,
                collections,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let state = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        receiver,
    };
    fixture.poll_until_idle();
    fixture
}

/// One collection entry of the fixtures.
fn entry(key: &str, name: &str, checked: bool) -> GuiCollectionEntry {
    GuiCollectionEntry {
        key: key.to_owned(),
        name: name.to_owned(),
        checked,
    }
}

/// The Music frame: All selected, three groups.
fn music_all() -> GuiCollections {
    GuiCollections {
        target: crate::gui_core::GuiCollectionTarget::Music(2),
        all: true,
        entries: vec![
            entry("g1", "Hip-Hop", false),
            entry("g2", "Jazz", false),
            entry("g3", "Soul", false),
        ],
        noun: crate::gui_core::GuiCollectionNoun::Groups,
    }
}

/// The Feeds frame: two subscriptions checked.
fn feeds_two_checked() -> GuiCollections {
    GuiCollections {
        target: crate::gui_core::GuiCollectionTarget::Feeds,
        all: false,
        entries: vec![
            entry("https://example.test/tifo", "Tifo Football", true),
            entry("https://example.test/off-menu", "Off Menu", true),
            entry("https://example.test/other", "Other Show", false),
        ],
        noun: crate::gui_core::GuiCollectionNoun::Subscriptions,
    }
}

/// Contract (design D6, spec "The dropdown trigger names the selection"): All
/// names itself.
#[test]
fn the_trigger_names_all() {
    let fixture = built(music_all());

    assert_eq!(fixture.text_of("[data-library-collections-trigger]"), "All");
}

/// Contract (design D6): one checked collection names itself.
#[test]
fn the_trigger_names_the_one_checked_collection() {
    let mut collections = music_all();
    collections.all = false;
    collections.entries[1].checked = true;
    let fixture = built(collections);

    assert_eq!(
        fixture.text_of("[data-library-collections-trigger]"),
        "Jazz"
    );
}

/// Contract (design D6): several checked collections name their count with the
/// destination's noun.
#[test]
fn the_trigger_names_the_count_with_the_noun() {
    let fixture = built(feeds_two_checked());

    assert_eq!(
        fixture.text_of("[data-library-collections-trigger]"),
        "2 subscriptions"
    );
}

/// Contract (the page-7 artboard, user override): the dropdown paints no Clear
/// control and no date-order toggle — the All row selects All, and the GUI
/// exposes no date order.
#[test]
fn the_dropdown_paints_no_clear_and_no_date_order_toggle() {
    let fixture = built(feeds_two_checked());

    assert!(!fixture.presents("[data-library-collections-clear]"));
    assert!(!fixture.presents("[data-library-date-order]"));
}

/// Contract (design D6a, user override): the trigger is an outlined box in the
/// accent, in the list filter field's corner radius — not a pill.
#[test]
fn the_trigger_paints_the_accent_box_with_the_filter_field_s_radius() {
    let fixture = built(music_all());
    let style = fixture.style_of("[data-library-collections-trigger]");

    assert!(style.contains("border-radius: 6px"), "{style}");
    assert!(!style.contains("999px"), "{style}");
    assert!(
        style.contains("border: 1px solid var(--canvas-accent)"),
        "{style}"
    );
}

/// Contract (design D6a, user override): the open popover anchors to the
/// trigger's right edge, under it.
#[test]
fn the_open_popover_anchors_to_the_trigger_s_right_edge() {
    let mut fixture = built(music_all());
    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_collections_dropdown());

    let style = fixture.style_of("[data-library-collections-menu]");
    assert!(style.contains("top: 34px"), "{style}");
    assert!(style.contains("right: 0"), "{style}");
    assert!(!style.contains("left: 0"), "{style}");
}

/// Contract (design D6a): the trigger's chevron points up while the popover is
/// open and down while it is closed.
#[test]
fn the_chevron_follows_the_open_fact() {
    let closed = built(music_all());
    assert!(
        closed.presents("[data-library-collections-trigger] svg.lucide-chevron-down"),
        "the closed trigger's chevron points down"
    );
    assert!(
        !closed.presents("[data-library-collections-trigger] svg.lucide-chevron-up"),
        "the closed trigger paints no up chevron"
    );

    let mut open = built(music_all());
    let library = open.state.clone();
    open.in_runtime(move || library.toggle_collections_dropdown());
    assert!(
        open.presents("[data-library-collections-trigger] svg.lucide-chevron-up"),
        "the open trigger's chevron points up"
    );
    assert!(
        !open.presents("[data-library-collections-trigger] svg.lucide-chevron-down"),
        "the open trigger paints no down chevron"
    );
}

/// Contract (design D7): the trigger opens the dropdown, and a second press
/// closes it.
#[test]
fn the_trigger_opens_and_closes_the_dropdown() {
    let mut fixture = built(music_all());
    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_collections_dropdown());

    assert!(fixture.presents("[data-library-collections-menu]"));

    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_collections_dropdown());

    assert!(!fixture.presents("[data-library-collections-menu]"));
}

/// Contract (design D6, spec "Clear resets the selection"): the All row's
/// press sends the All command and does not open the dropdown.
#[test]
fn the_all_row_press_sends_all_without_opening_the_dropdown() {
    let mut fixture = built(feeds_two_checked());
    let library = fixture.state.clone();
    let collections = feeds_two_checked();
    fixture.in_runtime(move || {
        library.press_collection(&collections, ALL_KEY);
    });

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::CollectionAll {
            target: crate::gui_core::GuiCollectionTarget::Feeds,
        }]
    );
    assert!(fixture.state.ui.peek().collections.is_none());
}

/// Contract (design D6, spec "A check leaves the dropdown open"): a row's press
/// sends its toggle and the dropdown stays open.
#[test]
fn a_row_press_toggles_and_leaves_the_dropdown_open() {
    let mut fixture = built(music_all());
    let library = fixture.state.clone();
    let collections = music_all();
    fixture.in_runtime(move || {
        library.toggle_collections_dropdown();
        library.press_collection(&collections, "g3");
    });

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::CollectionToggle {
            target: crate::gui_core::GuiCollectionTarget::Music(2),
            key: "g3".to_owned(),
        }]
    );
    assert!(fixture.state.ui.peek().collections.is_some());
}

/// Contract (design D6): the rows paint a leading square checkbox, All first.
#[test]
fn the_rows_paint_a_leading_checkbox() {
    let mut fixture = built(feeds_two_checked());
    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_collections_dropdown());

    assert!(
        fixture.presents("[data-library-collections-all] [data-library-collections-checkbox]"),
        "the All row's checkbox paints"
    );
    assert!(
        fixture
            .presents("[data-library-collections-row=\"https://example.test/tifo\"] [data-library-collections-checkbox]"),
        "a collection row's checkbox paints"
    );
}

/// Contract (design D6): a checked row's checkbox carries the check mark, and
/// an unchecked row's checkbox is empty.
#[test]
fn the_check_mark_paints_in_the_checked_row_s_checkbox() {
    let mut fixture = built(feeds_two_checked());
    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_collections_dropdown());

    assert!(
        fixture.presents(
            "[data-library-collections-row=\"https://example.test/tifo\"] [data-library-collections-checkbox] svg.lucide-check"
        ),
        "the checked row's checkbox carries the check"
    );
    assert!(
        !fixture.presents(
            "[data-library-collections-row=\"https://example.test/other\"] [data-library-collections-checkbox] svg.lucide-check"
        ),
        "the unchecked row's checkbox is empty"
    );
}

/// Contract (design D6): the checked row paints the highlighted band with its
/// bold name, the unchecked row paints neither.
#[test]
fn the_checked_row_paints_the_highlight_band() {
    let mut fixture = built(feeds_two_checked());
    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_collections_dropdown());

    let checked = fixture.style_of("[data-library-collections-row=\"https://example.test/tifo\"]");
    assert!(
        checked.contains("background: var(--canvas-selected)"),
        "{checked}"
    );
    assert!(checked.contains("font-weight: 600"), "{checked}");

    let unchecked =
        fixture.style_of("[data-library-collections-row=\"https://example.test/other\"]");
    assert!(unchecked.contains("background: none"), "{unchecked}");
    assert!(unchecked.contains("font-weight: 400"), "{unchecked}");
}

/// Contract (design D7, spec "The filter narrows the entries"): the open
/// dropdown paints the matching collections and All, and no other collection.
#[test]
fn the_filter_narrows_the_entries() {
    let mut fixture = built(feeds_two_checked());
    let library = fixture.state.clone();
    fixture.in_runtime(move || {
        library.toggle_collections_dropdown();
        library.set_collections_filter("tifo".to_owned());
    });

    assert!(
        fixture.presents("[data-library-collections-all]"),
        "All stays shown"
    );
    assert!(
        fixture.presents("[data-library-collections-row=\"https://example.test/tifo\"]"),
        "the matching collection paints"
    );
    assert!(
        !fixture.presents("[data-library-collections-row=\"https://example.test/off-menu\"]"),
        "a collection the filter does not match paints nothing"
    );
}
