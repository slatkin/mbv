//! Headless tests of the view dropdown (design D8; emby-favourites design
//! D5): the trigger names
//! the one active view, the popover paints All, New, and Upcoming as radio
//! rows, and a press installs the chosen view and closes the popover. No test
//! opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use crate::gui_core::GuiLibraryView;
use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryViewDropdown;
use crate::bridge::frame::library::{GuiDestinationList, GuiLibraryList, GuiShowsList};
use crate::core_thread::CoreInput;
use crate::gui_core::GuiDestinationKind;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    view: GuiLibraryView,
    destination: GuiDestinationKind,
    handle: Rc<RefCell<Option<LibraryState>>>,
}

fn dropdown_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        view,
        destination,
        handle,
    } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let mut sections = Sections::new();
    sections.apply_batch(&crate::bridge::sections::GuiSectionBatch {
        library_list: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(GuiLibraryList {
                refreshing: false,
                tabs: Vec::new(),
                current_tab: None,
                destination: GuiDestinationList::Shows(GuiShowsList {
                    sort: None,
                    lib_idx: 3,
                    view,
                    loading: false,
                    series: Vec::new(),
                    upcoming_failed: false,
                    rows: Vec::new(),
                    favourites: None,
                }),
            }),
        }),
        ..Default::default()
    });
    let state = use_hook(move || {
        LibraryState::new(
            sections,
            CoreLink::new(sender),
            Memo::new(|| 0.0),
            Arc::clone(&clock),
        )
    });
    provide_context(state.clone());
    handle.borrow_mut().replace(state);
    rsx! { LibraryViewDropdown { view, destination } }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    receiver: mpsc::Receiver<CoreInput>,
}

impl Fixture {
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .unwrap()
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id).unwrap().text_content().trim().to_owned()
    }

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

fn built(view: GuiLibraryView) -> Fixture {
    built_at(view, GuiDestinationKind::Shows { lib_idx: 3 })
}

fn built_at(view: GuiLibraryView, destination: GuiDestinationKind) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            dropdown_app,
            FixtureProps {
                sender,
                view,
                destination,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let state = handle.borrow_mut().take().unwrap();
    let mut fixture = Fixture {
        doc,
        state,
        receiver,
    };
    fixture.poll_until_idle();
    fixture
}

/// Contract (design D8): the trigger names the one active view.
#[test]
fn the_trigger_names_the_active_view() {
    let fixture = built(GuiLibraryView::Upcoming);
    assert_eq!(fixture.text_of("[data-library-view-trigger]"), "Upcoming");
}

/// Contract (design D8): the popover paints the destination's views with radio
/// marks and no filter field.
#[test]
fn the_popover_paints_the_destination_radio_views_and_no_filter() {
    let mut fixture = built(GuiLibraryView::All);
    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_library_view_dropdown());

    assert_eq!(
        fixture
            .doc
            .inner()
            .query_selector_all("[data-library-view-row]")
            .unwrap()
            .len(),
        4
    );
    assert!(fixture.presents("[data-library-view-mark]"));
    assert!(!fixture.presents("input"));
}

/// Contract (emby-favourites design D5): a destination that is not Shows
/// paints All and Favourites alone, in that order.
#[test]
fn a_library_destination_paints_all_and_favourites_only() {
    let mut fixture = built_at(
        GuiLibraryView::All,
        GuiDestinationKind::Library { lib_idx: 3 },
    );
    let library = fixture.state.clone();
    fixture.in_runtime(move || library.toggle_library_view_dropdown());

    let dom = fixture.doc.inner();
    let labels: Vec<String> = dom
        .query_selector_all("[data-library-view-row]")
        .expect("the selector is valid")
        .into_iter()
        .map(|id| {
            dom.get_node(id)
                .expect("the node is in the document")
                .text_content()
                .trim()
                .to_owned()
        })
        .collect();
    assert_eq!(labels, ["All".to_owned(), "Favourites".to_owned()]);
}

/// Contract (design D8): a view press installs the choice and closes the
/// popover, with no collection command sent.
#[test]
fn a_view_press_replaces_the_choice_and_closes_the_popover() {
    let mut fixture = built(GuiLibraryView::All);
    let library = fixture.state.clone();
    fixture.in_runtime(move || {
        library.toggle_library_view_dropdown();
        library.select_library_view(3, GuiLibraryView::New);
        library.close_library_view_dropdown();
    });

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::LibrarySetView {
            lib_idx: 3,
            view: GuiLibraryView::New,
        }]
    );
    assert!(!fixture.state.library_view_open());
}
