//! Tests of the Library rows' hover Shuffle command (gui-row-hover-actions
//! design D4, task 3.2): the command runs the row menu's own shuffle entry, so
//! a folder row shuffles the folder exactly as its right-click Shuffle does,
//! and a row whose menu carries no shuffle entry changes nothing. The Emby
//! boundary is the in-memory mock transport, so no request leaves the process
//! and no worker outlives the assertion.

use super::*;

const LIB_ID: &str = "lib-movies";
const FOLDER_ID: &str = "fold-1";

/// The folder's listing, as the mock transport returns it: two playable tracks,
/// so the shuffle replaces the empty Owner queue with them.
const FOLDER_TRACKS: &str = r#"{"Items":[{"Id":"t-1","Name":"First","Type":"Audio","MediaType":"Audio"},{"Id":"t-2","Name":"Second","Type":"Audio","MediaType":"Audio"}],"TotalRecordCount":2}"#;

/// A browse level over `items`, hanging off `parent_id`.
fn level(parent_id: &str, title: &str, items: Vec<EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        parent_id: parent_id.to_string(),
        title: title.to_string(),
        items,
        rows: ServerRows::new(0),
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".to_string(),
        sort_order: "Ascending".to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// An Emby folder row.
fn folder_item() -> EmbyItem {
    let mut item = make_item("A Folder", "Folder");
    item.id = FOLDER_ID.to_string();
    item.is_folder = true;
    item
}

/// An Emby leaf row (a movie), whose menu carries no shuffle entry.
fn movie_item() -> EmbyItem {
    let mut item = make_item("A Movie", "Movie");
    item.id = "movie-1".to_string();
    item.media_type = "Video".to_string();
    item
}

/// A stub `App` attached to a local daemon Owner, whose Emby runtime holds a
/// client over `http` and whose one active library level holds `row`: the
/// Library panel has focus, so the row's menu builds and its entries run.
fn shuffle_app(
    http: &MockHttp,
    row: EmbyItem,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(Vec::new());
    let mut config = app.config_for_test(|config| config.clone());
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .cached_emby_client_for_test()
        .expect("the installed setup holds a client")
        .with_test_agent(http.agent());
    app.adopt_ready_emby_client_for_test(client);
    app.take_libraries_for_test();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = LIB_ID.to_string();
    library.collection_type = "movies".to_string();
    let mut tab = LibraryTab::new(library);
    tab.nav_stack.push(level(LIB_ID, "Movies", vec![row]));
    app.push_library_for_test(tab);
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app.set_panel_focus(PanelFocus::Library);
    (app, cmd_rx)
}

/// The last queue replacement the Owner received: its item ids and its source.
/// Drains the channel, so call once per assertion point.
fn sent_replacement(
    cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) -> Option<(Vec<String>, mbv_queue::QueueSource)> {
    let mut sent = None;
    for command in cmd_rx.try_iter() {
        if let mbv_ctrl::CtrlCmd::UnifiedQueueReplace { items, source, .. } = command {
            let ids = items.iter().map(|item| item.id().to_string()).collect();
            sent = Some((ids, source));
        }
    }
    sent
}

/// Contract (design D4, spec "Shuffle from a folder row"): a folder row's
/// Shuffle runs the row menu's `ShuffleFolder` entry, which shuffles the
/// folder's playable items into the queue as a shuffle-sourced replacement.
#[test]
fn a_folder_rows_shuffle_runs_its_shuffle_folder_entry() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_when(&format!("ParentId={FOLDER_ID}"), 200, FOLDER_TRACKS);
    let (mut app, cmd_rx) = shuffle_app(&http, folder_item());

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::Shuffle {
            row: GuiLibraryMenuRow::Item(FOLDER_ID.to_string()),
        },
    );

    let (mut ids, source) = sent_replacement(&cmd_rx).expect("the shuffle replaced the queue");
    ids.sort();
    assert_eq!(ids, ["t-1", "t-2"]);
    assert!(
        matches!(source, mbv_queue::QueueSource::Shuffle),
        "the replacement is the shuffle's, not the folder play's: {source:?}"
    );
}

/// Contract (design D4): a row whose menu carries no shuffle entry changes
/// nothing — no fetch starts and no queue replacement reaches the Owner.
#[test]
fn a_rows_menu_without_a_shuffle_entry_changes_nothing() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let (mut app, cmd_rx) = shuffle_app(&http, movie_item());
    let requests_before = http.request_count();

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::Shuffle {
            row: GuiLibraryMenuRow::Item("movie-1".to_string()),
        },
    );

    assert_eq!(http.request_count(), requests_before, "no fetch starts");
    assert!(
        sent_replacement(&cmd_rx).is_none(),
        "no replacement reaches the Owner"
    );
}
