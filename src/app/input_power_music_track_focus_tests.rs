#![allow(dead_code, unused_imports)]

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

fn add_following_artist_albums(app: &mut App, album_count: usize) {
    let albums = (0..album_count).map(|i| {
        let mut album = make_item(&format!("Beta Album {i:02}"), "MusicAlbum");
        album.id = format!("beta-album-{i}");
        album.artist = "Beta".into();
        album.is_folder = true;
        album
    });
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .extend(albums);
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
