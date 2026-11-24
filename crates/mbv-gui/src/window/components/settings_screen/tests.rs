//! Headless tests of the Settings screen (D16): the contracts of
//! `SettingsScreen.test.ts` — the content frame titles the shown section
//! with its name, the Services section as Sources — and the screen root's
//! pointer press contracts the former client `window` listeners owned: the open
//! pick menu closes with no send, and the settings cursor hides. The press
//! contract rides the handler function the root's `onpointerdown` calls one
//! line each (a headless document cannot fire a DOM pointerdown). The core
//! channel is a mock pair; no test opens a window, sleeps, or reaches the
//! core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{SettingsScreen, screen_press};
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::state::{SettingsPane, SettingsState};

/// What the root hands the test: the section stores and the Settings store.
type FixtureHandles = (Sections, SettingsState);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    arrangement: GuiArrangement,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides them, and paints the screen.
fn settings_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsScreen { arrangement: props.arrangement }
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    state: SettingsState,
    receiver: Receiver<CoreInput>,
}

impl Fixture {
    /// Write the Settings section's frame, as the drain task would.
    fn set_frame(&mut self, frame: GuiSettingsFrame) {
        let mut settings = self.sections.settings;
        self.doc
            .vdom
            .in_runtime(move || settings.set(Some(SectionRef::new(frame))));
        self.poll_until_idle();
    }

    /// Fold one frame into the interaction state, as the root's sync does.
    fn sync_frame(&mut self, frame: &GuiSettingsFrame) {
        let state = self.state.clone();
        let frame = frame.clone();
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

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
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

/// A settings frame over `section`, with that section's name painted.
fn frame(section: GuiSettingsSection) -> GuiSettingsFrame {
    GuiSettingsFrame {
        section,
        sections: vec![
            GuiSettingsSection::Services,
            GuiSettingsSection::Playback,
            GuiSettingsSection::Mpv,
        ],
        content: GuiSettingsContent::Rows { rows: Vec::new() },
    }
}

/// The built screen over the carried frame and arrangement.
fn built(section: GuiSettingsSection, arrangement: GuiArrangement) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            settings_app,
            FixtureProps {
                sender,
                arrangement,
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
    let carried = frame(section);
    fixture.set_frame(carried.clone());
    fixture.sync_frame(&carried);
    fixture
}

/// Contract (`SettingsScreen.test.ts`): the content frame titles the shown
/// section with its name, the Services section as Sources.
#[rstest]
#[case(GuiSettingsSection::Services, "Sources")]
#[case(GuiSettingsSection::Playback, "Playback")]
#[case(GuiSettingsSection::Mpv, "mpv")]
fn the_content_frame_titles_the_section_with_its_name(
    #[case] section: GuiSettingsSection,
    #[case] title: &str,
) {
    let fixture = built(section, GuiArrangement::Narrow);

    assert_eq!(
        fixture.text_of("[data-settings-content-title]"),
        title,
        "the title names the section"
    );
}

/// Contract: the screen paints its arrangement, so the
/// window's narrow-window fact is visible in the tree.
#[test]
fn the_screen_paints_its_arrangement() {
    let fixture = built(GuiSettingsSection::Playback, GuiArrangement::Wide);

    assert!(fixture.presents("[data-settings-arrangement=\"wide\"]"));
}

/// Contract (the spec's *Narrow window* requirement, the former screen's Mini
/// gating): in Mini only the pane that holds the keyboard paints.
#[rstest]
#[case(SettingsPane::Sidebar)]
#[case(SettingsPane::Content)]
fn mini_paints_only_the_pane_that_holds_the_keyboard(#[case] pane: SettingsPane) {
    let mut fixture = built(GuiSettingsSection::Playback, GuiArrangement::Mini);
    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.pane = pane);
    });
    fixture.poll_until_idle();

    assert_eq!(
        fixture.presents("[data-settings-sidebar-pane]"),
        pane == SettingsPane::Sidebar,
        "the sidebar paints only while it holds the keyboard"
    );
    assert_eq!(
        fixture.presents("[data-settings-content]"),
        pane == SettingsPane::Content,
        "the content paints only while it holds the keyboard"
    );
}

/// Contract (`SettingsPickButton.test.ts`, task 8.3's wiring requirement): a
/// press outside the menu closes it and sends nothing. The press rides the
/// handler the screen root's `onpointerdown` calls.
#[test]
fn a_press_outside_the_menu_closes_it_and_sends_nothing() {
    let mut fixture = built(GuiSettingsSection::Playback, GuiArrangement::Narrow);
    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        state.open_pick_menu(
            "subtitleLanguage",
            "English",
            &[String::new(), "English".to_owned()],
        );
    });
    fixture.poll_until_idle();

    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || screen_press(&state));
    fixture.poll_until_idle();

    assert!(fixture.state.pick_menu.peek().is_none(), "the menu closed");
    assert!(
        fixture.settings_commands().is_empty(),
        "the close sent nothing"
    );
}

/// Contract: a
/// press on the screen hides the settings cursor, like `:focus-visible` in
/// reverse. The press rides the handler the screen root's `onpointerdown`
/// calls.
#[test]
fn a_press_on_the_screen_hides_the_settings_cursor() {
    let mut fixture = built(GuiSettingsSection::Playback, GuiArrangement::Narrow);
    let state = fixture.state.clone();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.show_settings_cursor());
    fixture.poll_until_idle();
    assert!(fixture.state.ui.peek().keyboard, "the cursor shows");

    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || screen_press(&state));
    fixture.poll_until_idle();

    assert!(!fixture.state.ui.peek().keyboard, "the cursor hid");
}
