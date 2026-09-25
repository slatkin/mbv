use super::*;
use crate::app::components::media_list::SelectionOrigin;
use crate::app::components::msg::{MusicTreeAction, ShellRequest};
use crate::app::images::CachedImage;
use crate::app::tests::{
    install_test_emby, make_app_stub, make_item, make_remote_app_stub_with_cmd_rx,
};
use crate::app::{LibraryTab, PanelFocus, TabSelection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::EmbyItem;
use mbv_core::mock_http::MockHttp;
use std::sync::{Arc, Mutex};

#[test]
fn loaded_keys_override_reaches_the_model() {
    // The compiled `[keys]` configuration is read once, at Model
    // construction, from the config the App was built with (U2 row 2.3).
    let app = make_app_stub();
    let config = crate::config::Config {
        keybinds: mbv_core::keybinds::load(&mbv_core::keybinds::RawKeybinds {
            prefix: Some("Ctrl+b".into()),
            sections: vec![(
                "global".into(),
                mbv_core::keybinds::RawSection {
                    router: vec![("help_open".into(), "F9".into())],
                    prefix: vec![],
                },
            )],
        })
        .unwrap(),
        ..Default::default()
    };
    *app.config.lock().unwrap() = config;

    let model = Model::new(app);
    assert_eq!(
        model.keybinds.prefix,
        Some(mbv_core::keybinds::Chord::parse("Ctrl+b").unwrap())
    );
    assert_eq!(
        model.keybinds.router_override("help_open"),
        Some(mbv_core::keybinds::Chord::parse("F9").unwrap())
    );
    assert!(model.keybinds.router_override("settings_open").is_none());
}

#[test]
fn model_without_keys_configuration_holds_default_keybinds() {
    let model = Model::new(make_app_stub());
    assert_eq!(model.keybinds, mbv_core::keybinds::Keybinds::default());
}

#[test]
fn ui_root_router_command_opens_help() {
    let mut model = Model::new(make_app_stub());
    let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Command(Command::OpenHelp)
    );
    assert!(arbitrate_key(
        messages,
        Some(&ComponentId::UiRoot),
        &RouterOutcome::Command(Command::OpenHelp)
    )
    .0
    .is_empty());
    assert!(!model.dispatch_router_command(Command::OpenHelp));
    assert!(model
        .application
        .mounted(&ComponentId::Overlay(OverlayId::Help)));
}

#[test]
fn unhandled_space_fires_the_playback_candidate_once() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::TogglePlayPause)
    );
    // No candidate timing state exists: an unhandled press fires on that
    // press, and a later unhandled press fires again as a fresh press.
    assert!(
        model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), false)
    );
    assert!(
        model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), false)
    );
}

#[test]
fn consumed_space_cancels_the_playback_candidate_and_leaves_no_state() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    // A focused media list holding an active Visual selection consumes Space
    // for its row-local toggle; the shell sees the leaf's consumption only.
    model.visual_selection = Some((PanelFocus::Library, 2));
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE).into(),
    ))];

    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::TogglePlayPause)
    );
    assert!(
        !model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), true)
    );
    // A later unhandled press still behaves as a first press: no consumed
    // state is inherited.
    assert!(
        model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), false)
    );
}

/// The double-Esc stop (see `Model::router_outcome`): the first Esc falls
/// through to its claimants and dispatches nothing; a second Esc inside
/// [`DOUBLE_ESC_STOP_WINDOW`] resolves the stop candidate. A press on any
/// other key disarms, so Esc-then-Space stays a play/pause toggle.
#[test]
fn first_escape_falls_through_and_second_dispatches_the_stop_candidate() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    // First press: falls through, no candidate, nothing to dispatch.
    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
    // Second press inside the window: the stop candidate, which an unhandled
    // press dispatches.
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::Stop)
    );
    assert!(model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::Stop), false));

    // A non-Esc press between the two disarms: Space stays play/pause.
    let space = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE).into(),
    ))];
    model.router_outcome(&space);
    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
}

#[test]
fn consumed_escape_cancels_the_stop_candidate_and_leaves_no_state() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    // The first Esc arms the double-Esc window without a candidate.
    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::Stop)
    );
    // A leaf that consumed Esc (dismiss/back claims first) cancels the
    // candidate and records nothing.
    assert!(!model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::Stop), true));
    // A later unhandled press still behaves as a first press.
    assert!(model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::Stop), false));
}

#[test]
fn converted_surface_skips_observer_key_but_retains_redraw_signal() {
    let focused = ComponentId::LibraryPlaybackPanel;
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    // Leaf focused, empty policy: the fold drops the observer's Key trigger
    // (the leaf already got the event) but keeps non-key observer signals.
    let router = RouterOutcome::FallThrough;
    let routed = fold_keyboard_messages(
        vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))],
        Some(&focused),
        &router,
    );
    assert!(routed.is_empty());
    let routed = fold_keyboard_messages(
        vec![Msg::TerminalEvent(TerminalObserverEvent::NoOp)],
        Some(&focused),
        &router,
    );
    assert!(matches!(
        routed.as_slice(),
        [Msg::TerminalEvent(TerminalObserverEvent::NoOp)]
    ));
}

#[test]
fn terminal_resize_observer_preserves_layout_side_effects() {
    let mut model = Model::new(make_app_stub());
    model.app.force_clear = false;
    model
        .app
        .card_image_states
        .insert("stale".into(), CachedImage::empty());
    model.app.card_image_loading.insert("stale".into());
    let mut music_resize = false;
    let mut tv_resize = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::Resize {
            width: 80,
            height: 24,
        },
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(model.app.force_clear);
    assert!(model.app.card_image_states.is_empty());
    assert!(model.app.card_image_loading.is_empty());
    assert!(music_resize && tv_resize);
}

#[test]
fn terminal_resize_observer_applies_new_size_before_paint() {
    let mut model = Model::new(make_app_stub());
    model.app.terminal_width = 60;
    model.app.terminal_height = 24;
    assert!(!model.app.is_right_panel_wide());
    let mut music_resize = false;
    let mut tv_resize = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::Resize {
            width: 150,
            height: 24,
        },
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.terminal_width, 150);
    assert_eq!(model.app.terminal_height, 24);
    assert!(model.app.is_right_panel_wide());
}

#[test]
fn terminal_focus_observer_preserves_refocus_side_effects() {
    let mut model = Model::new(make_app_stub());
    let mut music_resize = false;
    let mut tv_resize = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusGained,
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(model.app.refocus_at.is_some());
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusLost,
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(model.app.refocus_at.is_none());
}

fn music_album(id: &str) -> EmbyItem {
    let mut item = make_item(id, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item.media_type = "Audio".into();
    item
}

fn respond_with_tracks(http: &MockHttp, tracks: &[(&str, &str)]) {
    let items: Vec<_> = tracks
        .iter()
        .map(|(id, sort_name)| {
            serde_json::json!({
                "Id": id,
                "Name": id,
                "Type": "Audio",
                "MediaType": "Audio",
                "SortName": sort_name,
            })
        })
        .collect();
    http.respond(200, &serde_json::json!({ "Items": items }).to_string());
}

fn mocked_music_action_model(
    http: &MockHttp,
) -> (Model, std::sync::mpsc::Receiver<mbv_core::ctrl::CtrlCmd>) {
    let (mut app, command_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .expect("test Emby client")
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(Arc::new(Mutex::new(client)));

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "music-library".into();
    library.collection_type = "music".into();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    // Keep Model::new on Home so mounting does not start an unrelated browse;
    // the shell action below still exercises the active Emby-library route.
    let mut model = Model::new(app);
    model.app.tab = TabSelection::EmbyLibrary(0);
    while command_rx.try_recv().is_ok() {}
    (model, command_rx)
}

fn dispatch_music_artist_action(model: &mut Model, action: MusicTreeAction, items: Vec<EmbyItem>) {
    let mut music_resize = false;
    let mut tv_resize = false;
    model.handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::MusicArtistAction {
            action,
            items,
            origin: SelectionOrigin::Queue,
            unresolved_targets: Vec::new(),
        })),
        &mut music_resize,
        &mut tv_resize,
    );
}

fn replacement_command_ids(
    command_rx: &std::sync::mpsc::Receiver<mbv_core::ctrl::CtrlCmd>,
) -> Vec<Vec<String>> {
    command_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_core::ctrl::CtrlCmd::UnifiedQueueReplace { slots, .. } => Some(
                slots
                    .into_iter()
                    .map(|slot| slot.item.id().to_string())
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn music_artist_play_replaces_once_with_ordered_album_tracks() {
    let http = MockHttp::new();
    respond_with_tracks(&http, &[("a-track-2", "02"), ("a-track-1", "01")]);
    respond_with_tracks(&http, &[("b-track-1", "01")]);
    let (mut model, command_rx) = mocked_music_action_model(&http);

    dispatch_music_artist_action(
        &mut model,
        MusicTreeAction::Play,
        vec![music_album("album-a"), music_album("album-b")],
    );

    assert_eq!(
        replacement_command_ids(&command_rx),
        vec![vec![
            "a-track-1".to_string(),
            "a-track-2".to_string(),
            "b-track-1".to_string(),
        ]]
    );
    assert_eq!(model.app.effective_panel_focus(), PanelFocus::Library);
    assert_eq!(http.request_count(), 2);
}

#[test]
fn music_artist_shuffle_replaces_once_with_all_album_tracks() {
    let http = MockHttp::new();
    respond_with_tracks(&http, &[("a-track-1", "01")]);
    respond_with_tracks(&http, &[("b-track-1", "01"), ("b-track-2", "02")]);
    let (mut model, command_rx) = mocked_music_action_model(&http);

    dispatch_music_artist_action(
        &mut model,
        MusicTreeAction::Shuffle,
        vec![music_album("album-a"), music_album("album-b")],
    );

    let mut ids = replacement_command_ids(&command_rx);
    assert_eq!(ids.len(), 1);
    ids[0].sort();
    assert_eq!(
        ids[0],
        vec![
            "a-track-1".to_string(),
            "b-track-1".to_string(),
            "b-track-2".to_string(),
        ]
    );
    assert_eq!(model.app.queue_source, crate::config::QueueSource::Shuffle);
    assert_eq!(model.app.effective_panel_focus(), PanelFocus::Library);
    assert_eq!(http.request_count(), 2);
}

#[test]
fn music_artist_enqueue_keeps_ordered_per_album_appends() {
    let http = MockHttp::new();
    respond_with_tracks(&http, &[("a-track-1", "01")]);
    respond_with_tracks(&http, &[("b-track-2", "02"), ("b-track-1", "01")]);
    let (mut model, command_rx) = mocked_music_action_model(&http);

    dispatch_music_artist_action(
        &mut model,
        MusicTreeAction::Enqueue,
        vec![music_album("album-a"), music_album("album-b")],
    );

    let ids: Vec<_> = model
        .app
        .displayed_queue()
        .all_queue_items()
        .into_iter()
        .map(|item| item.id().to_string())
        .collect();
    assert_eq!(ids, vec!["a-track-1", "b-track-1", "b-track-2"]);
    assert!(replacement_command_ids(&command_rx).is_empty());
    assert_eq!(http.request_count(), 2);
}
