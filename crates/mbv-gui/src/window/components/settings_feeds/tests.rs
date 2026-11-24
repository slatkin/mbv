//! Headless tests of the Feeds section (D16): the contracts of
//! `SettingsFeeds.test.ts` — a press on the add control or a subscription row
//! opens the feed form in place and moves the content cursor onto it; a
//! press on a remove row sends `feedRemove` with the subscription URL; the
//! add form's submit sends `feedAdd` with the typed fields; and Cancel drops
//! the text-field hold, so a movement key reaches the screen router (the U6
//! review fix). The edit-form submit contract (`feeds.test.ts`) the
//! Settings store tests own (`window/settings/tests/feeds.rs`). The press
//! contracts ride the handler functions the section's controls call one line
//! each. The core channel is a mock pair; no test opens a window, sleeps, or
//! reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use mbv_queue::FeedKind;
use rstest::rstest;

use super::SettingsFeeds;
use crate::bridge::frame::settings::feeds::GuiSettingsFeed;
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::entries::{entry_id, settings_entries};
use crate::window::settings::state::{SettingsForm, SettingsPane, SettingsState};

/// What the root hands the test: the section stores and the Settings store.
type FixtureHandles = (Sections, SettingsState);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides them, and paints the section.
fn feeds_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsFeeds {}
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

/// The Feeds content with two subscriptions.
fn feeds_content() -> GuiSettingsContent {
    GuiSettingsContent::Feeds {
        subscriptions: vec![
            GuiSettingsFeed {
                name: "A feed".to_owned(),
                url: "https://a".to_owned(),
                kind: FeedKind::Audio,
            },
            GuiSettingsFeed {
                name: "B feed".to_owned(),
                url: "https://b".to_owned(),
                kind: FeedKind::Video,
            },
        ],
        adding: false,
    }
}

/// The built section over the carried content.
fn built(content: GuiSettingsContent) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            feeds_app,
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
        section: GuiSettingsSection::Feeds,
        sections: vec![GuiSettingsSection::Feeds],
        content,
    });
    fixture
}

/// The painted text of the one kind field, after asserting that the control
/// is one field and that the field itself is the button (not a box around a
/// button per kind).
fn kind_field_text(doc: &DioxusDocument) -> String {
    let dom = doc.inner();
    let ids = dom
        .query_selector_all("[data-settings-field=\"kind\"]")
        .expect("the selector is valid");
    assert_eq!(ids.len(), 1, "the kind control is one field");
    let node = dom.get_node(ids[0]).expect("the node is in the document");
    assert!(
        node.data
            .is_element_with_tag_name(&LocalName::from("button")),
        "the field itself takes the press"
    );
    node.text_content().trim().to_owned()
}

/// Contract: the kind control paints
/// as one field showing the draft's kind, and the menu's other kind moves the
/// draft, so the submit carries the same `feedAdd` command as a typed form.
#[test]
fn the_kind_field_shows_the_chosen_kind_and_the_menu_sets_the_other() {
    let mut fixture = built(feeds_content());
    let state = fixture.state.clone();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.open_feed_form(None));
    fixture.poll_until_idle();

    assert_eq!(
        kind_field_text(&fixture.doc),
        "Audio",
        "the field shows the draft's kind"
    );

    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        state.toggle_feed_kind_menu();
        state.choose_feed_kind(1);
    });
    fixture.poll_until_idle();

    assert_eq!(
        kind_field_text(&fixture.doc),
        "Video",
        "the chosen kind replaced the draft's"
    );

    let content = feeds_content();
    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        state.focus_settings_field(SettingsForm::Feed);
        state.submit_settings_form(&content);
    });
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::FeedAdd {
            name: String::new(),
            url: String::new(),
            kind: FeedKind::Video,
        }],
        "the submit carried the chosen kind"
    );
}

/// Contract (`SettingsFeeds.test.ts`): a press on the add control or a
/// subscription row opens the feed form in place — an add with no identity,
/// an edit seeded with the subscription's URL — and moves the content cursor
/// onto it, with no send.
#[rstest]
#[case("feedAdd", None)]
#[case("feed:https://a", Some("https://a"))]
fn a_press_opens_the_feed_form_in_place(#[case] id: &str, #[case] identity: Option<&str>) {
    let content = feeds_content();
    let mut fixture = built(content.clone());
    let entries = settings_entries(&content, &[]);

    let state = fixture.state.clone();
    let pressed = id.to_owned();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.press_settings_entry(&entries, &pressed));
    fixture.poll_until_idle();

    assert!(
        fixture.settings_commands().is_empty(),
        "opening the form sends nothing"
    );
    let ui = fixture.state.ui.peek().clone();
    assert_eq!(
        ui.feed_form
            .as_ref()
            .and_then(|form| form.identity.as_deref()),
        identity,
        "the form opened in the pressed row's mode"
    );
    assert_eq!(
        ui.pane,
        SettingsPane::Content,
        "the keyboard took the content"
    );
    assert_eq!(
        crate::window::settings::state::cursor_entry_id(&settings_entries(&content, &[]), &ui),
        Some(id.to_owned()),
        "the cursor moved onto the pressed row"
    );
}

/// Contract (`SettingsFeeds.test.ts`): a press on a remove row sends
/// `feedRemove` with the subscription URL.
#[test]
fn a_press_on_a_remove_row_sends_feed_remove_with_the_url() {
    let content = feeds_content();
    let mut fixture = built(content.clone());
    let entries = settings_entries(&content, &[]);

    let state = fixture.state.clone();
    let id = entry_id::feed_remove("https://a");
    fixture
        .doc
        .vdom
        .in_runtime(move || state.press_settings_entry(&entries, &id));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::FeedRemove {
            url: "https://a".to_owned(),
        }],
        "the remove named the subscription's URL"
    );
}

/// Contract (`SettingsFeeds.test.ts`): the add form's submit sends `feedAdd`
/// with the typed fields, and the form closes. The submit rides
/// `submit_settings_form`, the handler the form's Add button calls.
#[test]
fn the_add_form_submit_sends_feed_add_with_the_typed_fields() {
    let content = feeds_content();
    let mut fixture = built(content.clone());
    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        state.open_feed_form(None);
        state.focus_settings_field(SettingsForm::Feed);
        state.update_feed_form(|form| {
            form.name = "C feed".to_owned();
            form.url = "https://c".to_owned();
        });
    });
    fixture.poll_until_idle();

    let state = fixture.state.clone();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.submit_settings_form(&content));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::FeedAdd {
            name: "C feed".to_owned(),
            url: "https://c".to_owned(),
            kind: FeedKind::Audio,
        }],
        "the submit carried the typed fields"
    );
    assert!(
        fixture.state.ui.peek().feed_form.is_none(),
        "the form closed"
    );
}

/// Contract (`SettingsFeeds.test.ts`, the U6 review fix): Cancel drops the
/// text-field hold with the form, so a movement key reaches the screen
/// router again. The press rides `cancel_feed_form`, the handler the Cancel
/// button calls.
#[test]
fn cancel_drops_the_text_field_hold() {
    let content = feeds_content();
    let mut fixture = built(content);
    let state = fixture.state.clone();
    fixture.doc.vdom.in_runtime(move || {
        state.open_feed_form(None);
        state.focus_settings_field(SettingsForm::Feed);
    });
    fixture.poll_until_idle();
    assert_eq!(
        fixture.state.ui.peek().text_field,
        Some(SettingsForm::Feed),
        "the form's field holds the keyboard"
    );

    let state = fixture.state.clone();
    fixture
        .doc
        .vdom
        .in_runtime(move || state.cancel_feed_form());
    fixture.poll_until_idle();

    let ui = fixture.state.ui.peek().clone();
    assert!(ui.feed_form.is_none(), "the form closed");
    assert_eq!(ui.text_field, None, "the hold dropped with the form");
    assert!(
        fixture.settings_commands().is_empty(),
        "the cancel sent nothing"
    );
}
