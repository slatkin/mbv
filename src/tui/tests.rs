use super::*;
use crate::app::dispatch::action::{PlaybackIntent, ShellCommand};
use crate::app::tests::{make_app_stub, make_remote_app_stub_with_cmd_rx};
use crossterm::event::{KeyCode, KeyEvent};
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::media_list::SelectionOrigin;
use mbv_ui_msg::{MusicTreeAction, ShellRequest};

/// Record a terminal size on a test `Model` the way the shell records one,
/// so its single-panel fact matches the width.
fn set_test_terminal_size(model: &mut Model, width: u16, height: u16) {
    model.terminal_width = width;
    model.terminal_height = height;
    let single_panel = single_panel_for_width(width);
    model.app.set_single_panel(single_panel);
}

#[test]
fn ui_root_router_command_opens_help() {
    let mut model = Model::new(make_app_stub());
    let neutral_key = super::neutral_edge::translate_key(tuirealm::event::KeyEvent::from(
        KeyEvent::new(KeyCode::F(1), crossterm::event::KeyModifiers::NONE),
    ));
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(neutral_key))];
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Command(ShellCommand::OpenHelp.into())
    );
    assert_eq!(
        arbitrate_key(
            messages,
            Some(&ComponentId::UiRoot),
            &RouterOutcome::Command(ShellCommand::OpenHelp.into())
        )
        .0,
        [] as [mbv_ui_msg::Msg; 0]
    );
    assert!(!model.dispatch_router_command(&ShellCommand::OpenHelp.into()));
    assert!(
        model
            .application
            .mounted(&ComponentId::Overlay(OverlayId::Help))
    );
}

#[test]
fn consumed_space_cancels_the_playback_candidate_and_leaves_no_state() {
    let mut model = Model::new(make_app_stub());
    model.app.seed_playing_status_for_test(0, 0);
    // A focused media list holding an active Visual selection consumes Space
    // for its row-local toggle; the shell sees the leaf's consumption only.
    model.visual_selection = Some((PanelFocus::Library, 2));
    let neutral_key =
        mbv_ui_msg::KeyEvent::new(mbv_ui_msg::Key::Char(' '), mbv_ui_msg::KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(neutral_key))];

    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(PlaybackIntent::TogglePlayPause.into())
    );
    assert!(!model.apply_deferred_candidate(
        &RouterOutcome::Deferred(PlaybackIntent::TogglePlayPause.into()),
        true
    ));
    // A later unhandled press still behaves as a first press: no consumed
    // state is inherited.
    assert!(model.apply_deferred_candidate(
        &RouterOutcome::Deferred(PlaybackIntent::TogglePlayPause.into()),
        false
    ));
}

#[test]
fn terminal_resize_observer_applies_new_size_before_paint() {
    let mut model = Model::new(make_app_stub());
    set_test_terminal_size(&mut model, 60, 24);
    assert!(!crate::tui::tv_wide::is_right_panel_wide(
        &model.app,
        model.terminal_size()
    ));
    let mut music_resize = false;
    let mut tv_resize = false;
    apply_terminal_observer(
        &mut model,
        &TerminalObserverEvent::Resize {
            width: 150,
            height: 24,
        },
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.terminal_width, 150);
    assert_eq!(model.terminal_height, 24);
    assert!(crate::tui::tv_wide::is_right_panel_wide(
        &model.app,
        model.terminal_size()
    ));
}

/// Regression guard for an expanded pinned panel at 120 columns, the width
/// `TWO_COLUMN_THRESHOLD` is tuned for (the configured panel width is in
/// logical pixels, so its column count depends on the terminal cell size): in
/// Both mode, the queue column (40) and the `TAB_LEFT_PAD` insets leave the
/// library content exactly `TWO_COLUMN_THRESHOLD` columns wide, and wide mode
/// must hold. Restoring the old 80 threshold silently re-narrowed the pinned
/// panel by 4 columns.
#[test]
fn the_default_pinned_expanded_width_is_wide() {
    let mut model = Model::new(make_app_stub());
    set_test_terminal_size(&mut model, 120, 40);
    assert!(crate::tui::tv_wide::is_right_panel_wide(
        &model.app,
        model.terminal_size()
    ));
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
) -> (Model, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut app, command_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    let mut config = app.config_for_test(|config| config.clone());
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_client_for_test()
        .expect("test Emby client")
        .with_test_agent(http.agent());
    app.adopt_ready_emby_client_for_test(client);

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "music-library".into();
    library.collection_type = "music".into();
    library.is_folder = true;
    app.push_library_for_test(LibraryTab::new(library));
    // Keep Model::new on Home so mounting does not start an unrelated browse;
    // the shell action below still exercises the active Emby-library route.
    let mut model = Model::new(app);
    model
        .app
        .seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
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
    command_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) -> Vec<Vec<String>> {
    command_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueReplace { slots, .. } => Some(
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

// ── what the D4c set's tests read through `use super::*` ─────────────────
//
// A moved test file names the core's stub fixtures and a few leaf types
// through `use super::*`, as it did while it sat inside `crate::app::tests`.
// These re-exports keep that spelling path-only after the move.

pub(crate) use crate::app::state::playback::QueueTarget;
pub(crate) use crate::app::tests::*;
pub(crate) use crate::app::{App, AppCommand, Command, PanelIntent, QueueCardArtwork};
pub(crate) use crate::tui::Model;
pub(crate) use mbv_ctrl::player::PlayerEvent;
pub(crate) use mbv_ui_model::browse::BrowseLevel;
pub(crate) use mbv_ui_model::confirm::{ConfirmAction, ConfirmModal};
pub(crate) use mbv_ui_model::context_menu::ContextMenuAnchor;
pub(crate) use mbv_ui_model::feed::{FeedHomeVideoSelection, FeedHomeVideoState};
pub(crate) use mbv_ui_model::playback::QueueLink;
pub(crate) use mbv_ui_model::settings::{PanelFocus, SettingKey};
pub(crate) use mbv_ui_model::tab_selection::TabSelection;
pub(crate) use mbv_ui_model::toast::ToastSeverity;

// ── the TUI-staying tests of the D4c set ─────────────────────────────────
//
// These tests name the shell `Model` (or `TickHarness`, ratatui, crossterm,
// TuiRealm, `mbv_components`, or `mbv_render`), so they stay with the TUI
// Client. A file whose directory stayed in the core flattens to an
// `<owner>_<file>` name here; a family keeps its directory.

mod context_menu_entries;
mod context_menu_placement;
mod feeds_tab_guard;
mod gui_client_settings;
mod home_latest;
mod input_music_track_scope_tests;
mod library_viewport_reach_tests;
mod lifecycle;
mod music_track_test_support;
mod narrow_browse_migration;
mod panel_focus;
mod pinned_view_toggle;
mod queue;
mod routing_matrix;
mod tick_integration;
mod title_site;
mod title_site_overlay;
mod twitch_url;
