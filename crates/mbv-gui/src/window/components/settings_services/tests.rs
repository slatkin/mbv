//! Headless tests of the Services section (D16): the contracts of
//! `SettingsServices.test.ts` — a submit sends `embySetup` with the three
//! fields and then the password field is empty (the spec's *Secrets never
//! return*); a press on a Service action sends its `serviceAction` and moves
//! the content cursor onto it; the setup form's Cancel sends `cancelSetup`
//! for the form's service; an open Audiobookshelf form renders inside the
//! Audiobookshelf card; and Remove paints as the destructive button. The
//! press contracts ride the handler functions the section's controls call
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

use super::{SettingsServices, cancel_setup};
use crate::bridge::frame::settings::services::{GuiServiceEntry, GuiSetupService};
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
fn services_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsServices {}
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

/// The Services content with the Emby setup form open.
fn services_content() -> GuiSettingsContent {
    GuiSettingsContent::Services {
        services: vec![
            crate::bridge::frame::settings::services::GuiSettingsService {
                entry: GuiServiceEntry::Emby,
                name: "Emby".to_owned(),
                state: "Not configured".to_owned(),
                context: String::new(),
                actions: vec![crate::bridge::frame::settings::services::GuiServiceAction {
                    intent: crate::bridge::frame::settings::services::GuiServiceActionIntent::SetupEmby,
                    label: "Set up Emby".to_owned(),
                }],
            },
            crate::bridge::frame::settings::services::GuiSettingsService {
                entry: GuiServiceEntry::Audiobookshelf,
                name: "Audiobookshelf".to_owned(),
                state: "Not configured".to_owned(),
                context: String::new(),
                actions: vec![crate::bridge::frame::settings::services::GuiServiceAction {
                    intent: crate::bridge::frame::settings::services::GuiServiceActionIntent::SetupAudiobookshelf,
                    label: "Set up Audiobookshelf".to_owned(),
                }],
            },
            crate::bridge::frame::settings::services::GuiSettingsService {
                entry: GuiServiceEntry::Feeds,
                name: "Feeds".to_owned(),
                state: "Ready".to_owned(),
                context: "1 subscription".to_owned(),
                actions: vec![crate::bridge::frame::settings::services::GuiServiceAction {
                    intent: crate::bridge::frame::settings::services::GuiServiceActionIntent::ManageFeeds,
                    label: "Manage feeds".to_owned(),
                }],
            },
        ],
        setup: Some(crate::bridge::frame::settings::services::GuiSettingsSetup {
            service: GuiSetupService::Emby,
            server_url: "http://emby".to_owned(),
            busy: false,
            error: String::new(),
        }),
    }
}

/// The Audiobookshelf card alone, with its form open and the four actions a
/// configured Service offers.
fn audiobookshelf_content() -> GuiSettingsContent {
    GuiSettingsContent::Services {
        services: vec![crate::bridge::frame::settings::services::GuiSettingsService {
            entry: GuiServiceEntry::Audiobookshelf,
            name: "Audiobookshelf".to_owned(),
            state: "Ready".to_owned(),
            context: String::new(),
            actions: vec![
                crate::bridge::frame::settings::services::GuiServiceAction {
                    intent: crate::bridge::frame::settings::services::GuiServiceActionIntent::TestAudiobookshelf,
                    label: "Test connection".to_owned(),
                },
                crate::bridge::frame::settings::services::GuiServiceAction {
                    intent: crate::bridge::frame::settings::services::GuiServiceActionIntent::SetupAudiobookshelf,
                    label: "Set up Audiobookshelf".to_owned(),
                },
                crate::bridge::frame::settings::services::GuiServiceAction {
                    intent: crate::bridge::frame::settings::services::GuiServiceActionIntent::ReplaceAudiobookshelf,
                    label: "Replace".to_owned(),
                },
                crate::bridge::frame::settings::services::GuiServiceAction {
                    intent: crate::bridge::frame::settings::services::GuiServiceActionIntent::RemoveAudiobookshelf,
                    label: "Remove".to_owned(),
                },
            ],
        }],
        setup: Some(crate::bridge::frame::settings::services::GuiSettingsSetup {
            service: GuiSetupService::Audiobookshelf,
            server_url: "http://abs".to_owned(),
            busy: false,
            error: String::new(),
        }),
    }
}

/// The built section over the carried content, with the frame's setup form
/// folded into the drafts.
fn built(content: GuiSettingsContent) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            services_app,
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
        section: GuiSettingsSection::Services,
        sections: vec![GuiSettingsSection::Services],
        content,
    });
    fixture
}

/// Contract (`SettingsServices.test.ts`): a submit sends `embySetup` with the
/// three fields, then the password field is empty — the typed secret rides
/// the command and stays in no draft or painted value. The submit rides
/// `submit_settings_form`, the handler the form's Set up button calls.
#[test]
fn a_submit_sends_emby_setup_then_the_password_field_is_empty() {
    let content = services_content();
    let mut fixture = built(content.clone());
    // The form paints on the draft the frame's open seeded; type into it as
    // the fields' inputs would.
    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        state.focus_settings_field(crate::window::settings::state::SettingsForm::EmbySetup);
        state.update_emby_setup(|draft| {
            draft.username = "user@x".to_owned();
            draft.password = "s3cret".to_owned();
        });
    });
    fixture.poll_until_idle();

    assert_eq!(
        fixture.attr_of(
            "[data-settings-field=\"password\"]",
            LocalName::from("type")
        ),
        Some("password".to_owned()),
        "the password field is a password input"
    );

    let state = fixture.state.clone();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.submit_settings_form(&content));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::EmbySetup {
            server_url: "http://emby".to_owned(),
            username: "user@x".to_owned(),
            password: crate::gui_core::SetupSecret::new("s3cret".to_owned()),
        }],
        "the submit carried the three fields"
    );
    assert_eq!(
        fixture
            .state
            .ui
            .peek()
            .emby_setup
            .as_ref()
            .map(|draft| draft.password.as_str()),
        Some(""),
        "the submit emptied the password draft"
    );
}

/// Contract (`SettingsServices.test.ts`): a press on a Service action sends
/// its `serviceAction` and moves the content cursor onto it.
#[rstest]
#[case(
    "service:emby:setupEmby",
    mbv_ui_model::settings::ServiceActionIntent::SetupEmby
)]
#[case(
    "service:audiobookshelf:setupAudiobookshelf",
    mbv_ui_model::settings::ServiceActionIntent::SetupAudiobookshelf
)]
#[case(
    "service:feeds:manageFeeds",
    mbv_ui_model::settings::ServiceActionIntent::ManageFeeds
)]
fn a_press_on_a_service_action_sends_its_service_action(
    #[case] id: &str,
    #[case] intent: mbv_ui_model::settings::ServiceActionIntent,
) {
    let content = services_content();
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
        vec![crate::gui_core::GuiSettingsCommand::ServiceAction { action: intent }],
        "the press sent the action"
    );
    let ui = fixture.state.ui.peek().clone();
    assert_eq!(
        ui.pane,
        SettingsPane::Content,
        "the keyboard took the content"
    );
    assert_eq!(
        crate::window::settings::state::cursor_entry_id(&settings_entries(&content, &[]), &ui),
        Some(id.to_owned()),
        "the cursor moved onto the pressed action"
    );
}

/// Contract (`SettingsServices.test.ts`): the setup form's Cancel sends
/// `cancelSetup` for the form's service. The press rides the handler the
/// Cancel button calls.
#[test]
fn the_setup_form_cancel_sends_cancel_setup_for_the_form_service() {
    let mut fixture = built(services_content());

    let state = fixture.state.clone();
    fixture
        .doc
        .vdom
        .in_runtime(move || cancel_setup(&state, GuiSetupService::Emby));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::CancelSetup {
            service: crate::gui_core::GuiSetupService::Emby,
        }],
        "the cancel named the form's service"
    );
}

/// Contract (`SettingsServices.test.ts`): an open Audiobookshelf form renders
/// inside the Audiobookshelf card.
#[test]
fn an_open_audiobookshelf_form_renders_inside_the_audiobookshelf_card() {
    let fixture = built(audiobookshelf_content());

    assert!(
        fixture.presents("[data-settings-card][aria-label=\"Audiobookshelf\"] [data-settings-setup=\"audiobookshelf\"]"),
        "the form paints inside its Service's card"
    );
}

/// Contract (`SettingsServices.test.ts`): Remove paints as the destructive
/// button.
#[test]
fn remove_paints_as_the_destructive_button() {
    let fixture = built(audiobookshelf_content());

    assert_eq!(
        fixture.attr_of(
            &format!("[data-settings-entry=\"{}\"]", entry_id::service_action(
                GuiServiceEntry::Audiobookshelf,
                crate::bridge::frame::settings::services::GuiServiceActionIntent::RemoveAudiobookshelf,
            )),
            LocalName::from("data-settings-button")
        ),
        Some("destructive".to_owned()),
        "Remove paints destructive"
    );
}
