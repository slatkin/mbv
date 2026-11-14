//! Headless tests of the Playlists section (D16): the contracts of
//! `SettingsPlaylists.test.ts` — a press on the list's refresh, a playlist
//! row, an action, or delete sends the same command the keyboard's
//! activation sends and moves the content cursor onto it; a press on the
//! open playlist's Back, refresh, or an item action sends its command; and a
//! press on Rename opens the draft in place (the draft's Enter submit the
//! Settings store tests own, `window/settings/tests/playlists.rs`). The
//! press contracts ride `press_settings_entry`, the handler the section's
//! controls call one line each. The core channel is a mock pair; no test
//! opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::SettingsPlaylists;
use crate::bridge::frame::settings::playlists::{GuiOpenPlaylist, GuiPlaylistItem, GuiPlaylistRow};
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::sections::Sections;
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
fn playlists_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsPlaylists {}
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
            .in_runtime(move || settings.set(Some(carried)));
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

/// The playlist list's content.
fn list_content() -> GuiSettingsContent {
    GuiSettingsContent::Playlists {
        playlists: vec![GuiPlaylistRow {
            id: "p1".to_owned(),
            name: "Mix".to_owned(),
            item_count: 2,
        }],
        open: None,
        loading: false,
        open_loading: false,
    }
}

/// The open playlist's content.
fn open_content() -> GuiSettingsContent {
    GuiSettingsContent::Playlists {
        playlists: Vec::new(),
        open: Some(GuiOpenPlaylist {
            id: "p1".to_owned(),
            name: "Mix".to_owned(),
            items: vec![GuiPlaylistItem {
                id: "i1".to_owned(),
                name: "Song".to_owned(),
            }],
        }),
        loading: false,
        open_loading: false,
    }
}

/// The built section over the carried content.
fn built(content: GuiSettingsContent) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            playlists_app,
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
        section: GuiSettingsSection::Playlists,
        sections: vec![GuiSettingsSection::Playlists],
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

/// The entry id the content cursor rests on, after the press.
fn cursor_id(fixture: &Fixture, content: &GuiSettingsContent) -> Option<String> {
    crate::window::settings::state::cursor_entry_id(
        &settings_entries(content, &[]),
        &fixture.state.ui.peek().clone(),
    )
}

/// Contract (`SettingsPlaylists.test.ts`): a press on the list's refresh, a
/// playlist row, an action, or delete sends its command and moves the
/// content cursor onto it.
#[rstest]
#[case("playlistRefresh:list", mbv::GuiSettingsCommand::PlaylistRefresh { id: None })]
#[case("playlist:p1", mbv::GuiSettingsCommand::PlaylistOpen { id: Some("p1".to_owned()) })]
#[case("playlistAction:p1:play", mbv::GuiSettingsCommand::PlaylistAction { id: "p1".to_owned(), action: mbv_ui_msg::MusicTreeAction::Play })]
#[case("playlistAction:p1:shuffle", mbv::GuiSettingsCommand::PlaylistAction { id: "p1".to_owned(), action: mbv_ui_msg::MusicTreeAction::Shuffle })]
#[case("playlistAction:p1:enqueue", mbv::GuiSettingsCommand::PlaylistAction { id: "p1".to_owned(), action: mbv_ui_msg::MusicTreeAction::Enqueue })]
#[case("playlistDelete:p1", mbv::GuiSettingsCommand::PlaylistDelete { id: "p1".to_owned() })]
fn a_press_on_the_list_sends_its_command(
    #[case] id: &str,
    #[case] command: mbv::GuiSettingsCommand,
) {
    let content = list_content();
    let mut fixture = built(content.clone());

    press(&mut fixture, &content, id);

    assert_eq!(
        fixture.settings_commands(),
        vec![command],
        "the press sent the entry's command"
    );
    assert_eq!(
        cursor_id(&fixture, &content),
        Some(id.to_owned()),
        "the cursor moved onto the pressed control"
    );
}

/// Contract (`SettingsPlaylists.test.ts`): a press on the open playlist's
/// Back, refresh, or an item action sends its command and moves the content
/// cursor onto it.
#[rstest]
#[case("playlistBack", mbv::GuiSettingsCommand::PlaylistBack)]
#[case("playlistRefresh:p1", mbv::GuiSettingsCommand::PlaylistRefresh { id: Some("p1".to_owned()) })]
#[case("item:p1:i1:enqueue", mbv::GuiSettingsCommand::PlaylistItemAction { id: "p1".to_owned(), item_id: "i1".to_owned(), action: mbv_ui_msg::MusicTreeAction::Enqueue })]
fn a_press_on_the_open_playlist_sends_its_command(
    #[case] id: &str,
    #[case] command: mbv::GuiSettingsCommand,
) {
    let content = open_content();
    let mut fixture = built(content.clone());

    press(&mut fixture, &content, id);

    assert_eq!(
        fixture.settings_commands(),
        vec![command],
        "the press sent the entry's command"
    );
    assert_eq!(
        cursor_id(&fixture, &content),
        Some(id.to_owned()),
        "the cursor moved onto the pressed control"
    );
}

/// Contract (`SettingsPlaylists.test.ts`): a press on Rename opens the draft
/// in place, with no send. The field's Enter submit the store tests own.
#[test]
fn a_press_on_rename_opens_the_draft() {
    let content = list_content();
    let mut fixture = built(content.clone());

    press(&mut fixture, &content, "playlistRename:p1");

    assert!(
        fixture.settings_commands().is_empty(),
        "opening the draft sends nothing"
    );
    assert_eq!(
        fixture.state.ui.peek().rename,
        Some(crate::window::settings::state::RenameDraft {
            id: "p1".to_owned(),
            name: "Mix".to_owned(),
        }),
        "the draft opened seeded with the shown name"
    );
    assert_eq!(
        cursor_id(&fixture, &content),
        Some("playlistRename:p1".to_owned()),
        "the cursor moved onto the pressed control"
    );
}
