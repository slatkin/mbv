//! Headless tests of the Library section (D16): the contracts of
//! `SettingsLibrary.test.ts` — a press on a routable library sends
//! `routeDevices`, a press on the Automatic entry or a routable device sends
//! `assignRoute`, and a press on a device with no endpoint — a disabled row —
//! sends nothing. The press contracts ride `press_settings_entry`, the
//! handler the section's rows call one line each. The core channel is a mock
//! pair; no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::SettingsLibrary;
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::state::SettingsState;

/// What the root hands the test: the section stores and the Settings store.
type FixtureHandles = (Sections, SettingsState);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides them, and paints the section.
fn library_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsLibrary {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    state: SettingsState,
    receiver: Receiver<CoreInput>,
}

impl Fixture {
    /// Write the Settings section's frame and fold it into the interaction
    /// state, as the drain task and the root's sync do.
    fn open(&mut self, frame: GuiSettingsFrame) {
        let mut settings = self.sections.settings;
        let carried = frame.clone();
        self.doc
            .vdom
            .in_runtime(move || settings.set(Some(SectionRef::new(carried))));
        let state = self.state.clone();
        self.doc
            .vdom
            .in_runtime(move || state.sync_settings_frame(Some(&frame)));
        self.poll_until_idle();
    }

    /// The Settings commands the core link received, in send order.
    fn settings_commands(&self) -> Vec<crate::gui_core::GuiSettingsCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Settings(command) => Some(command),
                _ => None,
            })
            .collect()
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: LocalName) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(attribute)
            .map(str::to_owned)
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }
}

/// A Library content: two routable libraries, `music` selected with one
/// routable device and one device with no endpoint.
fn library_content() -> GuiSettingsContent {
    GuiSettingsContent::Library {
        rows: Vec::new(),
        routes: Some(vec![
            crate::bridge::frame::settings::library::GuiSettingsRoute {
                library: "music".to_owned(),
                name: "Music".to_owned(),
                endpoint: None,
            },
            crate::bridge::frame::settings::library::GuiSettingsRoute {
                library: "shows".to_owned(),
                name: "Shows".to_owned(),
                endpoint: Some("dlna:tv".to_owned()),
            },
        ]),
        selected: Some("music".to_owned()),
        devices: Some(vec![
            crate::bridge::frame::settings::library::GuiSettingsRouteDevice {
                name: "Phone".to_owned(),
                endpoint: Some("dlna:phone".to_owned()),
            },
            crate::bridge::frame::settings::library::GuiSettingsRouteDevice {
                name: "Ghost".to_owned(),
                endpoint: None,
            },
        ]),
    }
}

/// The built section over the carried content.
fn built(content: GuiSettingsContent) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            library_app,
            FixtureProps {
                sender,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let (sections, state) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the stores over");
    let mut fixture = Fixture {
        doc,
        sections,
        state,
        receiver,
    };
    fixture.open(GuiSettingsFrame {
        section: GuiSettingsSection::Library,
        sections: vec![GuiSettingsSection::Library],
        content,
    });
    fixture
}

/// Contract (`SettingsLibrary.test.ts`): a press on a routable library, the
/// Automatic entry, or a routable device sends its command and moves the
/// content cursor onto it.
#[rstest]
#[case("route:shows", crate::gui_core::GuiSettingsCommand::RouteDevices { library: "shows".to_owned() })]
#[case("routeClear:music", crate::gui_core::GuiSettingsCommand::AssignRoute { library: "music".to_owned(), endpoint: None })]
#[case("routeDevice:music:Phone", crate::gui_core::GuiSettingsCommand::AssignRoute { library: "music".to_owned(), endpoint: Some("dlna:phone".to_owned()) })]
fn a_press_on_a_route_entry_sends_its_command(
    #[case] id: &str,
    #[case] command: crate::gui_core::GuiSettingsCommand,
) {
    let content = library_content();
    let mut fixture = built(content.clone());
    let entries = crate::window::settings::entries::settings_entries(&content, &[]);

    let state = fixture.state.clone();
    let pressed = id.to_owned();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.press_settings_entry(&entries, &pressed));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![command],
        "the press sent the entry's command"
    );
    let ui = fixture.state.ui.peek().clone();
    assert_eq!(
        crate::window::settings::state::cursor_entry_id(
            &crate::window::settings::entries::settings_entries(&content, &[]),
            &ui
        ),
        Some(id.to_owned()),
        "the cursor moved onto the pressed entry"
    );
}

/// Contract (`SettingsLibrary.test.ts`): a press on a device with no endpoint
/// — a disabled row — sends nothing.
#[test]
fn a_press_on_a_device_with_no_endpoint_sends_nothing() {
    let content = library_content();
    let mut fixture = built(content.clone());
    let entries = crate::window::settings::entries::settings_entries(&content, &[]);

    assert_eq!(
        fixture.attr_of(
            "[data-settings-entry=\"routeDevice:music:Ghost\"]",
            LocalName::from("disabled")
        ),
        Some("true".to_owned()),
        "the unroutable device paints disabled"
    );

    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        state.press_settings_entry(&entries, "routeDevice:music:Ghost");
    });
    fixture.poll_until_idle();

    assert!(
        fixture.settings_commands().is_empty(),
        "the disabled row assigned nothing"
    );
}
