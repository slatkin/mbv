use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{Read, Write};

/// Power-view music library sitting on the album-folder-listing nav
/// level (`is_viewing_album_folders` holds): a grouped `["group",
/// "album"]` config, mirroring `render_power_library`'s inline-detail
/// tests, with two albums at the album level and `album-1` selected.
fn make_power_music_album_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let mut group = make_item("Alpha", "MusicArtist");
    group.id = "group-0".into();
    group.is_folder = true;

    let mut album1 = make_item("First Album", "MusicAlbum");
    album1.id = "album-1".into();
    album1.is_folder = true;
    let mut album2 = make_item("Second Album", "MusicAlbum");
    album2.id = "album-2".into();
    album2.is_folder = true;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![
            BrowseLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: vec![group],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            },
            BrowseLevel {
                parent_id: "group-0".into(),
                title: "Alpha".into(),
                items: vec![album1, album2],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            },
        ],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

fn add_beta_album(app: &mut App) {
    let mut album = make_item("Beta Album", "MusicAlbum");
    album.id = "album-beta".into();
    album.artist = "Beta".into();
    album.is_folder = true;
    app.libs[0].nav_stack.last_mut().unwrap().items.push(album);

    // A second Beta album gives the fixture a multi-album artist group for
    // header-navigation and bulk-action coverage.
    let mut album2 = make_item("Beta Album Two", "MusicAlbum");
    album2.id = "album-beta-2".into();
    album2.artist = "Beta".into();
    album2.is_folder = true;
    app.libs[0].nav_stack.last_mut().unwrap().items.push(album2);
}

fn push_tracks(app: &mut App, album_id: &str, count: usize) {
    let tracks: Vec<_> = (0..count)
        .map(|i| {
            let mut t = make_item(&format!("Track {i}"), "Audio");
            t.id = format!("{album_id}-track-{i}");
            t
        })
        .collect();
    app.album_tracks_cache.insert(album_id.to_string(), tracks);
}

fn make_power_music_album_list_app(album_count: usize, cursor: usize) -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let mut group = make_item("Alpha", "MusicArtist");
    group.id = "group-0".into();
    group.is_folder = true;

    let albums: Vec<_> = (0..album_count)
        .map(|i| {
            let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
            album.id = format!("album-{i}");
            album.artist = "Alpha".into();
            album.is_folder = true;
            album
        })
        .collect();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![
            BrowseLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: vec![group],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            },
            BrowseLevel {
                parent_id: "group-0".into(),
                title: "Alpha".into(),
                items: albums,
                total_count: album_count,
                cursor,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            },
        ],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

fn render_full_app(app: &mut App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();
}

struct RecursiveFetchServer {
    base_url: String,
    seen_parent_ids: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

type RecursiveFetchResponses = Vec<(
    &'static str,
    Result<Vec<(&'static str, &'static str, i64)>, &'static str>,
)>;

impl RecursiveFetchServer {
    fn start(responses: RecursiveFetchResponses) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let seen_parent_ids = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = seen_parent_ids.clone();
        let handle = std::thread::spawn(move || {
            let responses: std::collections::HashMap<_, _> = responses.into_iter().collect();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut last_request_at: Option<std::time::Instant> = None;
            while std::time::Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(accepted) => accepted,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if last_request_at.is_some_and(|last| {
                            last.elapsed() > std::time::Duration::from_millis(200)
                        }) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => panic!("recursive fetch test server accept failed: {e}"),
                };
                last_request_at = Some(std::time::Instant::now());
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap();
                let request = String::from_utf8_lossy(&buf[..n]);
                let first_line = request.lines().next().unwrap_or_default();
                let parent_id = first_line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|target| target.split('?').nth(1))
                    .and_then(|query| {
                        query
                            .split('&')
                            .find_map(|part| part.strip_prefix("ParentId=").map(str::to_string))
                    })
                    .unwrap_or_default();
                seen.lock().unwrap().push(parent_id.clone());

                let (status, body) = match responses.get(parent_id.as_str()) {
                    None => (
                        "404 Not Found",
                        serde_json::json!({ "error": format!("unexpected parent id {parent_id}") })
                            .to_string(),
                    ),
                    Some(Ok(items)) => {
                        let items: Vec<_> = items
                            .iter()
                            .map(|(id, name, index_number)| {
                                serde_json::json!({
                                    "Id": id,
                                    "Name": name,
                                    "Type": "Audio",
                                    "MediaType": "Audio",
                                    "IsFolder": false,
                                    "IndexNumber": index_number,
                                    "ParentIndexNumber": 1,
                                    "SortName": name,
                                    "UserData": {},
                                })
                            })
                            .collect();
                        (
                            "200 OK",
                            serde_json::json!({
                                "Items": items,
                                "TotalRecordCount": items.len(),
                            })
                            .to_string(),
                        )
                    }
                    Some(Err(message)) => (
                        "500 Internal Server Error",
                        serde_json::json!({ "error": message }).to_string(),
                    ),
                };
                let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                stream.write_all(response.as_bytes()).unwrap();
            }
            assert!(
                {
                    let seen = seen.lock().unwrap();
                    responses
                        .keys()
                        .all(|parent_id| seen.iter().any(|seen| seen.as_str() == *parent_id))
                },
                "unused recursive fetch stub responses"
            );
        });

        Self {
            base_url,
            seen_parent_ids,
            handle: Some(handle),
        }
    }

    fn seen_parent_ids(&self) -> Vec<String> {
        self.seen_parent_ids.lock().unwrap().clone()
    }

    fn first_seen_parent_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for parent_id in self.seen_parent_ids() {
            if !ids.contains(&parent_id) {
                ids.push(parent_id);
            }
        }
        ids
    }
}

impl Drop for RecursiveFetchServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

fn configure_recursive_fetch_server(app: &mut App, server: &RecursiveFetchServer) {
    let mut client = app.client.lock().unwrap();
    client.config.server_url = server.base_url.clone();
    client.user_id = "user-1".into();
    client.token = "token-1".into();
}

fn make_selectable_artist_header_bulk_app() -> App {
    let mut app = make_power_music_album_app();
    add_beta_album(&mut app);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 2;
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });
    app
}

#[test]
fn enter_at_album_folder_listing_enters_track_mode_without_nav_push() {
    let mut app = make_power_music_album_app();
    let nav_len_before = app.libs[0].nav_stack.len();
    assert!(app.is_viewing_album_folders(0));
    assert!(app.libs[0].album_track_focus.is_none());

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].album_track_focus, Some(0));
    assert_eq!(app.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn mouse_click_on_selected_album_folder_row_does_not_open_track_mode() {
    // Only Enter opens inline track-selection mode. A mouse click on the
    // already-selected album-folder row must not open it (and must not
    // fall back to the legacy nav_stack drilldown either).
    let mut app_key = make_power_music_album_app();
    let mut app_mouse = make_power_music_album_app();

    let nav_len_before = app_key.libs[0].nav_stack.len();
    assert_eq!(nav_len_before, app_mouse.libs[0].nav_stack.len());
    assert!(app_key.is_viewing_album_folders(0));
    assert!(app_key.libs[0].album_track_focus.is_none());
    assert!(app_mouse.libs[0].album_track_focus.is_none());

    let handled = app_key.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!handled);

    app_mouse.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app_mouse.layout.main.left_row_map = vec![Some(0)];
    app_mouse.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app_key.libs[0].album_track_focus, Some(0));
    assert_eq!(app_mouse.libs[0].album_track_focus, None);
    assert_eq!(app_key.libs[0].nav_stack.len(), nav_len_before);
    assert_eq!(app_mouse.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn refocus_click_after_focus_gained_is_suppressed() {
    let mut app = make_power_music_album_app();
    app.note_focus_gained();
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);
    assert!(app.refocus_at.is_none());
}

#[test]
fn click_without_focus_event_dispatches_normally() {
    let mut app = make_power_music_album_app();
    assert!(app.refocus_at.is_none());
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn click_outside_refocus_window_dispatches_normally() {
    let mut app = make_power_music_album_app();
    app.refocus_at = Some(Instant::now() - Duration::from_millis(500));
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn second_click_after_refocus_dispatches() {
    let mut app = make_power_music_album_app();
    app.note_focus_gained();
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);

    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn focus_lost_clears_pending_refocus() {
    let mut app = make_power_music_album_app();
    app.note_focus_gained();
    app.note_focus_lost();
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn selectable_artist_header_keyboard_up_down_selects_headers() {
    let mut app = make_power_music_album_app();
    add_beta_album(&mut app);
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 2;

    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

    assert_eq!(
        app.libs[0].artist_header_focus,
        Some(crate::app::ArtistHeaderSelection {
            first_album_id: "album-beta".into(),
            artist_label: "Beta".into(),
        })
    );
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        2,
        "selecting a header must not rewrite the album cursor"
    );

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert!(app.libs[0].artist_header_focus.is_none());
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 2);
}

#[test]
fn artist_header_selection_survives_group_size_change() {
    let mut app = make_power_music_album_app();
    let mut zeta_album = make_item("Zeta Album", "MusicAlbum");
    zeta_album.id = "album-zeta".into();
    zeta_album.artist = "Zeta".into();
    zeta_album.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(zeta_album);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-zeta".into(),
        artist_label: "Zeta".into(),
    });

    let mut zeta_album_two = make_item("Zeta Album Two", "MusicAlbum");
    zeta_album_two.id = "album-zeta-2".into();
    zeta_album_two.artist = "Zeta".into();
    zeta_album_two.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(zeta_album_two);

    render_full_app(&mut app, 100, 24);

    assert!(
        app.libs[0].artist_header_focus.is_some(),
        "revalidation should keep the same artist header focused when the \
             loaded sibling count changes"
    );
    assert_eq!(
        app.selected_artist_header_album_items(0)
            .expect("expected Zeta header selection to remain valid")
            .1
            .len(),
        2,
        "the same focused header should resolve the expanded group after another album loads"
    );
}

#[test]
fn selectable_artist_header_enter_is_consumed_noop() {
    let mut app = make_power_music_album_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });
    let nav_len = app.libs[0].nav_stack.len();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].nav_stack.len(), nav_len);
    assert!(app.libs[0].album_track_focus.is_none());
    assert!(app.libs[0].artist_header_focus.is_some());
}

#[test]
fn selectable_artist_header_mouse_click_selects_header() {
    let mut app = make_power_music_album_app();
    add_beta_album(&mut app);
    render_full_app(&mut app, 100, 24);
    let row = app
        .layout
        .main
        .left_row_targets
        .iter()
        .position(|target| {
            matches!(
                target,
                Some(LibraryRowTarget::ArtistHeader(selection))
                    if selection.artist_label == "Beta"
            )
        })
        .expect("expected Beta header row target");
    let x = app.layout.main.left_area.x;
    let y = app.layout.main.left_area.y + row as u16;

    let handled = app.click_set_cursor(x, y);

    assert!(handled);
    assert_eq!(
        app.libs[0].artist_header_focus,
        Some(crate::app::ArtistHeaderSelection {
            first_album_id: "album-beta".into(),
            artist_label: "Beta".into(),
        })
    );
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);
}

#[test]
fn selectable_artist_header_context_menu_uses_header_actions() {
    let mut app = make_power_music_album_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("expected header menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert_eq!(labels, vec!["Play All", "Shuffle", "Add to Queue"]);
    assert!(menu
        .entries
        .iter()
        .all(|entry| !matches!(entry.action, Some(ContextAction::PlayFolder(_)))));
}

#[test]
fn selectable_artist_header_members_use_current_display_plan_albums_only() {
    let mut app = make_power_music_album_app();
    add_beta_album(&mut app);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    let (_, albums) = app
        .selected_artist_header_album_items(0)
        .expect("expected selected header members");
    let ids: Vec<&str> = albums.iter().map(|album| album.id.as_str()).collect();

    assert_eq!(
        ids,
        vec!["album-1", "album-2"],
        "member resolution should preserve display album order and exclude Beta"
    );
}

#[test]
fn selectable_artist_header_stale_selection_is_cleared_on_revalidation() {
    let mut app = make_power_music_album_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "missing-album".into(),
        artist_label: "Unknown Artist".into(),
    });

    let albums = app.selected_artist_header_album_items(0);

    assert!(albums.is_none());
    assert!(app.libs[0].artist_header_focus.is_none());
}

#[test]
fn selectable_artist_header_direct_enqueue_fetches_header_albums_not_stale_cursor() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_selectable_artist_header_bulk_app();
    let server = RecursiveFetchServer::start(vec![
        (
            "album-1",
            Ok(vec![("a1-t2", "A1 Track 2", 2), ("a1-t1", "A1 Track 1", 1)]),
        ),
        ("album-2", Ok(vec![("a2-t1", "A2 Track 1", 1)])),
    ]);
    configure_recursive_fetch_server(&mut app, &server);

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    let queued_ids: Vec<&str> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        queued_ids,
        vec!["a1-t1", "a1-t2", "a2-t1"],
        "enqueue should preserve display album order and per-album track order"
    );
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()],
        "recursive fetches should target the selected header's albums, not stale album-beta"
    );
}

#[test]
fn selectable_artist_header_direct_play_fetches_header_albums_not_stale_cursor() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_selectable_artist_header_bulk_app();
    let server = RecursiveFetchServer::start(vec![
        (
            "album-1",
            Ok(vec![("a1-t2", "A1 Track 2", 2), ("a1-t1", "A1 Track 1", 1)]),
        ),
        ("album-2", Ok(vec![("a2-t1", "A2 Track 1", 1)])),
    ]);
    configure_recursive_fetch_server(&mut app, &server);

    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));

    let queued_ids: Vec<&str> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(queued_ids, vec!["a1-t1", "a1-t2", "a2-t1"]);
    assert_eq!(app.player_tab.queue_cursor, 0);
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()]
    );
}

#[test]
fn selectable_artist_header_context_shuffle_fetches_header_albums_not_stale_cursor() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_selectable_artist_header_bulk_app();
    let server = RecursiveFetchServer::start(vec![
        ("album-1", Ok(vec![("a1-t1", "A1 Track 1", 1)])),
        ("album-2", Ok(vec![("a2-t1", "A2 Track 1", 1)])),
    ]);
    configure_recursive_fetch_server(&mut app, &server);
    app.open_context_menu();
    let action = app
        .context_menu
        .as_ref()
        .and_then(|menu| {
            menu.entries
                .iter()
                .find(|entry| entry.label == "Shuffle")
                .and_then(|entry| entry.action.clone())
        })
        .expect("expected Shuffle header action");

    app.execute_context_action(Some(action));

    let mut queued_ids: Vec<&str> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    queued_ids.sort_unstable();
    assert_eq!(queued_ids, vec!["a1-t1", "a2-t1"]);
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()]
    );
}

#[test]
fn selectable_artist_header_fetch_error_leaves_queue_and_playback_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_selectable_artist_header_bulk_app();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing-track".into();
    existing.media_type = "Audio".into();
    app.player_tab.set_items(vec![existing], 0);
    let before_ids: Vec<String> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    let server = RecursiveFetchServer::start(vec![
        ("album-1", Ok(vec![("a1-t1", "A1 Track 1", 1)])),
        ("album-2", Err("album fetch failed")),
    ]);
    configure_recursive_fetch_server(&mut app, &server);

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    let after_ids: Vec<String> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(
        after_ids, before_ids,
        "enqueue must abort before mutation when any album fetch fails"
    );
    assert!(
        app.status.contains("status code 500"),
        "expected one surfaced fetch error, got {:?}; seen parent ids: {:?}",
        app.status,
        server.seen_parent_ids()
    );
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()]
    );
}

#[test]
fn up_down_in_track_mode_move_only_track_focus_and_clamp() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(2));
    // Clamp at the end -- no wrap.
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(2));

    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(0));
    // Clamp at the start -- no wrap.
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(0));

    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        album_cursor_before,
        "track-mode Up/Down must not move the album cursor"
    );
}

#[test]
fn track_mode_down_still_moves_track_focus_when_queue_panel_has_focus() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);
    app.panel_focus = PanelFocus::Queue;
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert_eq!(app.libs[0].album_track_focus, Some(2));
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        album_cursor_before
    );
}

#[test]
fn mouse_clicking_another_album_clears_track_focus() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    let handled = app.click_set_cursor(11, 5);

    assert!(handled);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
    assert!(app.libs[0].album_track_focus.is_none());
}

#[test]
fn selecting_music_group_clears_track_focus() {
    let mut app = make_power_music_album_app();
    let mut group2 = make_item("Beta", "MusicArtist");
    group2.id = "group-1".into();
    group2.is_folder = true;
    app.libs[0].nav_stack[0].items.push(group2);
    app.libs[0].album_track_focus = Some(1);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    app.select_music_group(0, 1);

    assert!(app.libs[0].album_track_focus.is_none());
    assert!(app.libs[0].artist_header_focus.is_none());
}

#[test]
fn switching_music_group_clears_track_focus() {
    let mut app = make_power_music_album_app();
    let mut group2 = make_item("Beta", "MusicArtist");
    group2.id = "group-1".into();
    group2.is_folder = true;
    app.libs[0].nav_stack[0].items.push(group2);
    app.libs[0].album_track_focus = Some(1);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    app.switch_music_group(0, 1);

    assert!(app.libs[0].album_track_focus.is_none());
    assert!(app.libs[0].artist_header_focus.is_none());
}

#[test]
fn up_down_in_track_mode_with_no_cached_tracks_is_noop() {
    let mut app = make_power_music_album_app();
    // No `push_tracks` call -- album_tracks_cache has no entry for
    // "album-1", mirroring "not yet loaded".
    app.libs[0].album_track_focus = Some(0);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].album_track_focus, Some(0));
}

#[test]
fn escape_in_track_mode_clears_focus_without_go_back() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(2);
    let nav_len_before = app.libs[0].nav_stack.len();
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(!handled);
    assert!(app.libs[0].album_track_focus.is_none());
    assert_eq!(
        app.libs[0].nav_stack.len(),
        nav_len_before,
        "Escape in track mode must not pop nav_stack (not a go_back)"
    );
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        album_cursor_before
    );
}

#[test]
fn up_down_outside_track_mode_still_move_album_cursor() {
    let mut app = make_power_music_album_app();
    assert!(app.libs[0].album_track_focus.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert!(app.libs[0].album_track_focus.is_none());
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn escape_outside_track_mode_still_calls_go_back_unchanged() {
    // `make_power_music_album_app`'s grouped `["group","album"]` fixture
    // sits at the *root* of the synthetic music-group view (nav_stack
    // len == 2), which `go_back`'s own pre-existing guard already
    // no-ops on ("don't pop when already at the root of a synthetic
    // group view" -- see `go_back`'s doc comment in actions.rs). The
    // regression this proves is narrower than "pops": Task 3 must route
    // Escape to the exact same `go_back()` call as before when
    // `album_track_focus` is `None`, whatever `go_back()` itself does --
    // demonstrated by comparing `handle_key(Esc)` against calling
    // `go_back()` directly on an identical, freshly-built app.
    let mut via_go_back = make_power_music_album_app();
    via_go_back.go_back();

    let mut via_escape_key = make_power_music_album_app();
    assert!(via_escape_key.libs[0].album_track_focus.is_none());
    let handled = via_escape_key.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        via_escape_key.libs[0].nav_stack.len(),
        via_go_back.libs[0].nav_stack.len()
    );
    assert_eq!(
        via_escape_key.libs[0].nav_stack.last().unwrap().cursor,
        via_go_back.libs[0].nav_stack.last().unwrap().cursor
    );
}

#[test]
fn page_down_in_album_list_mode_pages_by_rendered_rows_with_inline_detail() {
    let mut app = make_power_music_album_list_app(60, 0);
    push_tracks(&mut app, "album-0", 4);
    render_full_app(&mut app, 100, 40);
    let viewport_rows = app.layout.main.left_area.height as usize;
    assert_eq!(
        viewport_rows, 30,
        "fixture sanity: expected 30 rendered list rows"
    );
    assert!(app.power_right_panel_image_renders_allowed());

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(!handled);
    assert!(!app.power_right_panel_image_renders_allowed());
    // Display rows: 0 = artist header; the selected album 0 is wrapped
    // in the colored-block frame (1 = top border, 2 = colored top
    // padding, 3 = album row, 4 = collapsed action-hint row -- tracks
    // stay hidden until Enter is pressed, 5 = colored bottom padding,
    // 6 = bottom border), then 7.. = the remaining albums one row each.
    // A 30-row page from display row 3 lands on display row 33 = album 27.
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        27,
        "PageDown should move by rendered display rows, not raw album count"
    );
    assert!(app.libs[0].album_track_focus.is_none());
}

#[test]
fn page_up_in_album_list_mode_pages_by_rendered_rows_with_inline_detail() {
    let mut app = make_power_music_album_list_app(60, 35);
    push_tracks(&mut app, "album-35", 4);
    render_full_app(&mut app, 100, 40);
    let viewport_rows = app.layout.main.left_area.height as usize;
    assert_eq!(
        viewport_rows, 30,
        "fixture sanity: expected 30 rendered list rows"
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    // Display row 0 is the artist header, then one row per album up to
    // album 35's colored-block frame. The selected block adds its border,
    // padding, album-artist row, title row, and trailing block rows, so
    // album 35 is at display row 39. A 30-row page up lands on display
    // row 9 = album 8.
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        8,
        "PageUp should move by rendered display rows, not raw album count"
    );
    assert!(app.libs[0].album_track_focus.is_none());
}

#[test]
fn paging_past_display_edges_clamps_in_display_order_not_api_order() {
    let mut app = make_power_music_album_list_app(3, 0);
    app.libs[0].nav_stack.last_mut().unwrap().items[0].artist = "Zulu".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[1].artist = "Alpha".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[2].artist = "Bravo".into();
    push_tracks(&mut app, "album-0", 4);
    render_full_app(&mut app, 100, 40);
    app.layout.main.left_area.height = 100;

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        0,
        "PageDown past the last display row should clamp to the last display-order album"
    );

    let mut app = make_power_music_album_list_app(3, 1);
    app.libs[0].nav_stack.last_mut().unwrap().items[0].artist = "Zulu".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[1].artist = "Alpha".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[2].artist = "Bravo".into();
    push_tracks(&mut app, "album-1", 4);
    render_full_app(&mut app, 100, 40);
    app.layout.main.left_area.height = 100;

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        1,
        "PageUp past the first display row should clamp to the first display-order album"
    );
}

#[test]
fn paging_from_non_selectable_hint_and_header_rows_chooses_nearest_album_by_direction() {
    // Inline tracks (and the rule/loading rows around them) no longer
    // render in the music-group view until track-selection mode is
    // entered (Enter pressed), so paging can no longer land on those --
    // browsing-mode paging is disabled entirely once track-selection
    // mode is active (see `page_power_grouped_album_cursor`'s
    // `album_track_focus.is_some()` guard). The two non-selectable rows
    // paging can still land on while merely *browsing* the album list
    // are: the artist header, and the collapsed action-hint row that
    // sits directly under the selected album.
    let mut down_app = make_power_music_album_list_app(10, 0);
    render_full_app(&mut down_app, 100, 40);
    assert!(down_app.libs[0].album_track_focus.is_none());
    down_app.layout.main.left_area.height = 1;

    let handled = down_app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(!handled);
    // Display rows: 0 artist header; selected album 0 is wrapped in the
    // colored-block frame (1 top border, 2 colored top padding, 3
    // album-artist row, 4 album row, 5 collapsed action hint, 6 colored
    // bottom padding, 7 bottom border), then 8 = album 1. With a 1-row
    // page, PageDown targets the hint row, so paging resolves forward to
    // album 1.
    assert_eq!(down_app.libs[0].nav_stack.last().unwrap().cursor, 1);

    let mut up_app = make_power_music_album_list_app(10, 3);
    render_full_app(&mut up_app, 100, 40);
    assert!(up_app.libs[0].album_track_focus.is_none());
    up_app.layout.main.left_area.height = 4;

    let handled = up_app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    // Display rows: 0 artist header, 1-3 albums 0-2, then selected album
    // 3 is wrapped in the colored-block frame (4 top border, 5 colored
    // top padding, 6 album-artist row, 7 album row, ...). With a 4-row
    // page, PageUp targets row 3, the nearest album in the upward
    // direction: album 2.
    assert_eq!(up_app.libs[0].nav_stack.last().unwrap().cursor, 2);
}

fn buffer_to_string(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn render_inline_album_detail_uses_track_focus_as_cursor() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(2);

    let backend = ratatui::backend::TestBackend::new(100, 40);
    let mut term = ratatui::Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    let out = buffer_to_string(&term);

    // Track-focus mode reuses the inline album-detail cursor, so the
    // focused track keeps the selected-row gutter.
    let track_line = out
        .lines()
        .find(|l| l.contains("Track 2"))
        .unwrap_or_else(|| panic!("no 'Track 2' row found in rendered output:\n{out}"));
    assert!(
            track_line.contains('\u{258c}'),
            "expected focused track row to keep the selected-row marker, got: {track_line:?}\nfull output:\n{out}"
        );
}

// ── Task 4: scope-correct actions (#145) ─────────────────────────────

#[test]
fn current_lib_item_in_list_mode_returns_album_folder_not_a_track() {
    // Regression: album-list mode (`album_track_focus == None`) must
    // keep resolving to the selected album folder itself, exactly as
    // before Task 4.
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    assert!(app.libs[0].album_track_focus.is_none());

    let item = app.current_lib_item();

    let item = item.expect("current_lib_item should resolve the selected album");
    assert_eq!(item.id, "album-1");
    assert!(item.is_folder, "list mode must resolve to the album folder");
}

#[test]
fn current_lib_item_in_track_mode_returns_focused_track() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);

    let item = app.current_lib_item();

    let item = item.expect("current_lib_item should resolve the focused track");
    assert_eq!(item.id, "album-1-track-1");
    assert!(
        !item.is_folder,
        "track mode must resolve to the track, not the album folder"
    );
}

#[test]
fn current_lib_item_in_track_mode_falls_back_safely_when_cache_missing() {
    // Async fetch still in flight: `album_tracks_cache` has no entry for
    // "album-1" yet. Must not panic and must not index out of bounds.
    let mut app = make_power_music_album_app();
    app.libs[0].album_track_focus = Some(0);
    assert!(!app.album_tracks_cache.contains_key("album-1"));

    let item = app.current_lib_item();

    let item = item.expect("must fall back to the album folder item, not None");
    assert_eq!(item.id, "album-1");
    assert!(item.is_folder);
}

#[test]
fn enter_again_in_track_mode_plays_focused_track_from_cached_queue() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    // Queue built from the cached album tracks, starting at the focused
    // track (index 1 -> "album-1-track-1").
    let ids: Vec<_> = app.player_tab.items.iter().map(|i| i.id.clone()).collect();
    assert_eq!(
        ids,
        vec!["album-1-track-0", "album-1-track-1", "album-1-track-2"]
    );
    assert_eq!(app.player_tab.queue_cursor, 1);
    assert_eq!(app.libs[0].album_track_focus, Some(1));
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert_eq!(
        app.libs[0].album_track_focus,
        Some(2),
        "after Enter plays the focused track, the next Down key must remain in \
             track-selection mode"
    );
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        album_cursor_before,
        "after Enter plays the focused track, Down must not fall back to album-list navigation"
    );
    // Note: `app.queue_source` is not asserted here -- `play_items_routed`
    // (pre-existing, out of Task 4's scope) calls
    // `on_queue_replace_silent` as its first statement, which
    // unconditionally resets `queue_source` to `Unknown` immediately
    // after `select()` sets it to `Album`, so it is not a Task-4
    // regression -- the queue *contents* (ids + cursor, asserted
    // above) are the correct observable here.
}

#[test]
fn enter_again_in_track_mode_with_missing_cache_does_not_panic() {
    let mut app = make_power_music_album_app();
    // No `push_tracks` -- cache miss, async fetch still in flight.
    app.libs[0].album_track_focus = Some(0);
    let nav_len_before = app.libs[0].nav_stack.len();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].album_track_focus, Some(0));
    assert_eq!(app.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn context_menu_in_list_mode_offers_folder_scoped_actions_for_selected_album() {
    // Regression: album-list mode's context menu must still target the
    // selected ALBUM's id via the folder-scoped actions.
    let mut app = make_power_music_album_app();
    assert!(app.libs[0].album_track_focus.is_none());

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu should open");
    let actions: Vec<_> = menu
        .entries
        .iter()
        .filter_map(|e| e.action.clone())
        .collect();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::PlayFolder(id) if id == "album-1")),
        "expected PlayFolder(\"album-1\"), got: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::ShuffleFolder(id) if id == "album-1")),
        "expected ShuffleFolder(\"album-1\"), got: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::EnqueueFolder(item) if item.id == "album-1")),
        "expected EnqueueFolder(album-1), got: {actions:?}"
    );
}

#[test]
fn context_menu_in_track_mode_offers_track_scoped_actions_not_folder_actions() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu should open");
    let actions: Vec<_> = menu
        .entries
        .iter()
        .filter_map(|e| e.action.clone())
        .collect();
    assert!(
        actions.iter().any(|a| matches!(a, ContextAction::Play)),
        "track mode must offer the generic per-item Play action, got: {actions:?}"
    );
    assert!(
        actions.iter().any(|a| matches!(a, ContextAction::Enqueue)),
        "track mode must offer the generic per-item Enqueue action, got: {actions:?}"
    );
    assert!(
        !actions.iter().any(|a| matches!(
            a,
            ContextAction::PlayFolder(_)
                | ContextAction::ShuffleFolder(_)
                | ContextAction::EnqueueFolder(_)
        )),
        "track mode must not offer album-folder-scoped actions, got: {actions:?}"
    );
}
