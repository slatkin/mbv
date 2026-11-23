//! Headless tests of the Sessions section (D16): the contract of
//! `SettingsSessions.test.ts` — a press on a playback target sends
//! `selectTarget` with the target's stable key — the chooser's own path —
//! and moves the content cursor onto the pressed row. The press rides
//! `press_settings_entry`, the handler the section's rows call one line
//! each. The core channel is a mock pair; no test opens a window, sleeps, or
//! reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::SettingsSessions;
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::entries::settings_entries;
use crate::window::settings::state::SettingsState;

/// What the root hands the test: the section stores and the Settings store.
type FixtureHandles = (Sections, SettingsState);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides them, and paints the section.
fn sessions_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsSessions {}
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
    fn settings_commands(&self) -> Vec<mbv::GuiSettingsCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Settings(command) => Some(command),
                _ => None,
            })
            .collect()
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

/// A Sessions content: the local output and one Emby session.
fn sessions_content() -> GuiSettingsContent {
    GuiSettingsContent::Sessions {
        refreshing: false,
        targets: vec![
            crate::bridge::frame::playback::GuiTargetChoice {
                key: crate::bridge::frame::playback::GuiTargetKey::Local,
                label: "This machine".to_owned(),
                is_active: true,
                is_direct_peer: false,
            },
            crate::bridge::frame::playback::GuiTargetChoice {
                key: crate::bridge::frame::playback::GuiTargetKey::Emby("sess-1".to_owned()),
                label: "Bedroom".to_owned(),
                is_active: false,
                is_direct_peer: false,
            },
        ],
    }
}

/// The built section over the carried content.
fn built(content: GuiSettingsContent) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            sessions_app,
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
        section: GuiSettingsSection::Sessions,
        sections: vec![GuiSettingsSection::Sessions],
        content,
    });
    fixture
}

/// Contract (`SettingsSessions.test.ts`): a press on a playback target sends
/// `selectTarget` with the target's stable key and moves the content cursor
/// onto the pressed row.
#[rstest]
#[case("target:0", mbv_ui_model::panel_targets::ChooserTargetKey::Local)]
#[case(
    "target:1",
    mbv_ui_model::panel_targets::ChooserTargetKey::Target(
        mbv_ui_model::panel_targets::SessionTargetKey::Emby("sess-1".to_owned()),
    )
)]
fn a_press_on_a_target_sends_select_target_with_its_key(
    #[case] id: &str,
    #[case] key: mbv_ui_model::panel_targets::ChooserTargetKey,
) {
    let content = sessions_content();
    let mut fixture = built(content.clone());
    let entries = settings_entries(&content, &[]);

    let state = fixture.state.clone();
    let pressed = id.to_owned();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.press_settings_entry(&entries, &pressed));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![mbv::GuiSettingsCommand::SelectTarget { target: key }],
        "the press named the target's stable key"
    );
    assert_eq!(
        crate::window::settings::state::cursor_entry_id(
            &settings_entries(&content, &[]),
            &fixture.state.ui.peek().clone()
        ),
        Some(id.to_owned()),
        "the cursor moved onto the pressed row"
    );
}
