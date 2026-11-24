//! Headless tests of the Settings sidebar (D16): the contracts of
//! `SettingsSidebar.test.ts` — a press on a section entry sends
//! `selectSection`, moves the sidebar cursor onto it, and hands the keyboard
//! to the content; each entry paints its icon and its name; the open
//! section's entry carries `aria-current="page"`; a press on Back closes the
//! screen; and the cursor row paints only while the keyboard shows it. The
//! press contracts ride the handler functions the sidebar's own handlers call
//! one line each (a headless document cannot fire a DOM click). The core
//! channel is a mock pair; no test opens a window, sleeps, or reaches the
//! core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{SettingsSidebar, back, press_section};
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::components::settings_screen::section_wire;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::state::{SettingsPane, SettingsState};

/// What the root hands the test: the section stores and the Settings store.
type FixtureHandles = (Sections, SettingsState);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides them, and paints the sidebar.
fn sidebar_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsSidebar {}
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

    /// Show or hide the settings cursor, as a screen key or a press does.
    fn set_keyboard(&mut self, keyboard: bool) {
        let state = self.state.clone();
        self.doc.vdom.in_runtime(move || {
            let mut ui = state.ui;
            ui.with_mut(|ui| ui.keyboard = keyboard);
        });
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

/// A settings frame over the Services section, with three sections listed.
fn frame() -> GuiSettingsFrame {
    GuiSettingsFrame {
        section: GuiSettingsSection::Services,
        sections: vec![
            GuiSettingsSection::Services,
            GuiSettingsSection::Feeds,
            GuiSettingsSection::Keys,
        ],
        content: GuiSettingsContent::Rows { rows: Vec::new() },
    }
}

/// The built sidebar over the open frame.
fn built() -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            sidebar_app,
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
    fixture.open(frame());
    fixture
}

/// Contract (`SettingsSidebar.test.ts`): a press on a section entry sends
/// `selectSection`, moves the sidebar cursor onto it, and hands the keyboard
/// to the content — the same effect Enter on the cursor row has. The press
/// rides the handler the entry's `onclick` calls.
#[rstest]
#[case(GuiSettingsSection::Services, 0)]
#[case(GuiSettingsSection::Feeds, 1)]
#[case(GuiSettingsSection::Keys, 2)]
fn a_press_on_a_section_entry_selects_it(
    #[case] section: GuiSettingsSection,
    #[case] index: usize,
) {
    let mut fixture = built();

    let state = fixture.state.clone();
    fixture
        .doc
        .vdom
        .in_runtime(move || press_section(&state, index, section));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::SelectSection {
            section: crate::gui_core::GuiSettingsSection::from(section),
        }],
        "the press sent the section's open command"
    );
    let ui = fixture.state.ui.peek();
    assert_eq!(ui.sidebar_cursor, index, "the cursor takes the row");
    assert_eq!(
        ui.pane,
        SettingsPane::Content,
        "the keyboard hands to the content"
    );
}

/// Contract (`SettingsSidebar.test.ts`): each entry paints its icon and its
/// name, the Services section as Sources.
#[rstest]
#[case(GuiSettingsSection::Services, "Sources")]
#[case(GuiSettingsSection::Feeds, "Feeds")]
#[case(GuiSettingsSection::Keys, "Keys")]
fn an_entry_paints_its_icon_and_its_name(#[case] section: GuiSettingsSection, #[case] name: &str) {
    let fixture = built();

    let selector = format!("[data-settings-section=\"{}\"]", section_wire(section));
    assert!(
        fixture.presents(&format!("{selector} svg")),
        "the entry paints its icon"
    );
    assert_eq!(
        fixture.text_of(&selector),
        name,
        "the entry paints its name"
    );
}

/// Contract (`SettingsSidebar.test.ts`): the open section's entry carries
/// `aria-current="page"`, and the other entries carry none.
#[rstest]
#[case(GuiSettingsSection::Services, Some("page"))]
#[case(GuiSettingsSection::Feeds, None)]
#[case(GuiSettingsSection::Keys, None)]
fn the_open_section_entry_carries_aria_current(
    #[case] section: GuiSettingsSection,
    #[case] current: Option<&str>,
) {
    let fixture = built();

    assert_eq!(
        fixture.attr_of(
            &format!("[data-settings-section=\"{}\"]", section_wire(section)),
            LocalName::from("aria-current")
        ),
        current.map(str::to_owned),
        "aria-current names the open section alone"
    );
}

/// Contract (`SettingsSidebar.test.ts`): a press on Back closes the screen.
/// The press rides the handler the Back control's `onclick` calls.
#[test]
fn a_press_on_back_closes_the_screen() {
    let mut fixture = built();

    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || back(&state));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::Close],
        "the back press closed the screen"
    );
}

/// Contract (`SettingsSidebar.test.ts`): the sidebar cursor row paints only
/// while the keyboard shows it — a pointer user sees no row that looks
/// picked. Each case builds one screen at the named cursor fact.
#[rstest]
#[case(false)]
#[case(true)]
fn the_cursor_row_paints_only_while_the_keyboard_shows_it(#[case] keyboard: bool) {
    let mut fixture = built();
    fixture.set_keyboard(keyboard);

    assert_eq!(
        fixture.presents("[data-settings-sidebar] [data-settings-cursor]"),
        keyboard,
        "the cursor row paints with the keyboard fact alone"
    );
}
