//! Headless tests of the Keys section (D16): the contracts of
//! `SettingsKeys.test.ts` — the frame's refusal paints on the edited row
//! alone; a reset sends `keysReset` with the action's id; a press on a clear
//! sends `keysClear` with the target; and a press on a chord arms its
//! capture and sends no command. The press contracts ride
//! `press_settings_entry`, the handler the section's buttons call one line
//! each. The core channel is a mock pair; no test opens a window, sleeps, or
//! reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::SettingsKeys;
use crate::bridge::frame::settings::keys::{GuiSettingsKeyRefusal, GuiSettingsKeyRow};
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::entries::{entry_id, settings_entries};
use crate::window::settings::state::{SettingsPane, SettingsState};

/// What the root hands the test: the section stores and the Settings store.
type FixtureHandles = (Sections, SettingsState);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides them, and paints the section.
fn keys_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsKeys {}
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

/// The Keys content: one changed action carrying the frame's refusal, and
/// one unchanged action.
fn keys_content() -> GuiSettingsContent {
    GuiSettingsContent::Keys {
        prefix: Some("ctrl+p".to_owned()),
        rows: vec![
            GuiSettingsKeyRow {
                id: "stop".to_owned(),
                label: "Stop".to_owned(),
                section: "Playback".to_owned(),
                chords: vec!["x".to_owned()],
                prefix_chord: Some("p".to_owned()),
                changed: true,
                prefix_addressable: true,
            },
            GuiSettingsKeyRow {
                id: "toggle_play_pause".to_owned(),
                label: "Play or pause".to_owned(),
                section: "Playback".to_owned(),
                chords: vec![" ".to_owned()],
                prefix_chord: None,
                changed: false,
                prefix_addressable: false,
            },
        ],
        refusal: Some(GuiSettingsKeyRefusal {
            action: "stop".to_owned(),
            text: "the chord collides with `stop`".to_owned(),
        }),
    }
}

/// The built section over the carried content.
fn built(content: GuiSettingsContent) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            keys_app,
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
        section: GuiSettingsSection::Keys,
        sections: vec![GuiSettingsSection::Keys],
        content,
    });
    fixture
}

/// A press on the entry `id` names, over the section's entries.
fn press(fixture: &mut Fixture, content: &GuiSettingsContent, id: &str) {
    let entries = settings_entries(content, &[]);
    let state = fixture.state.clone();
    let pressed = id.to_owned();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.press_settings_entry(&entries, &pressed));
    fixture.poll_until_idle();
}

/// Contract (`SettingsKeys.test.ts`): the refusal text from the frame shows
/// on the edited row, and the unchanged row paints no refusal — the section
/// paints one refusal line.
#[test]
fn the_refusal_text_shows_on_the_edited_row() {
    let content = keys_content();
    let fixture = built(content);

    assert!(
        fixture.presents("[data-settings-key-row=\"stop\"] + [data-settings-key-refusal]"),
        "the refusal paints under the edited row"
    );
    assert!(
        !fixture.presents(
            "[data-settings-key-row=\"toggle_play_pause\"] + [data-settings-key-refusal]"
        ),
        "the unchanged row paints no refusal"
    );
    assert_eq!(
        fixture.text_of("[data-settings-key-refusal]"),
        "the chord collides with `stop`",
        "the refusal text is the frame's"
    );
}

/// Contract (`SettingsKeys.test.ts`): a reset sends `keysReset` with the
/// action's id.
#[test]
fn a_reset_sends_keys_reset_with_the_action_id() {
    let content = keys_content();
    let mut fixture = built(content.clone());

    press(&mut fixture, &content, &entry_id::key_reset("stop"));

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::KeysReset {
            action: "stop".to_owned(),
        }],
        "the reset named the action"
    );
}

/// Contract (`SettingsKeys.test.ts`): a press on a clear sends `keysClear`
/// with the target — the prefix chord's or the action's prefix-namespace
/// one.
#[rstest]
#[case("keyClear:prefix", "prefix")]
#[case("keyClear:prefix:stop", "prefix:stop")]
fn a_press_on_a_clear_sends_keys_clear_with_the_target(#[case] id: &str, #[case] target: &str) {
    let content = keys_content();
    let mut fixture = built(content.clone());

    press(&mut fixture, &content, id);

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::KeysClear {
            target: target.to_owned(),
        }],
        "the clear named the target"
    );
}

/// Contract (`SettingsKeys.test.ts`): a press on a chord arms its capture
/// and sends no command.
#[test]
fn a_press_on_a_chord_arms_its_capture_and_sends_nothing() {
    let content = keys_content();
    let mut fixture = built(content.clone());

    press(&mut fixture, &content, "keyTarget:stop");

    assert_eq!(
        fixture.state.ui.peek().capture,
        Some(crate::window::settings::state::Capture {
            target: "stop".to_owned(),
            sent: false,
        }),
        "the chord's capture armed"
    );
    assert!(
        fixture.settings_commands().is_empty(),
        "arming a capture sends nothing"
    );
    assert_eq!(
        fixture.state.ui.peek().pane,
        SettingsPane::Content,
        "the keyboard took the content"
    );
}
