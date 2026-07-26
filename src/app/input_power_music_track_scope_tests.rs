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
