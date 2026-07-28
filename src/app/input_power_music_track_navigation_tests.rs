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
fn track_mode_down_does_not_move_track_focus_when_queue_panel_has_focus() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);
    app.panel_focus = PanelFocus::Queue;
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert_eq!(app.libs[0].album_track_focus, Some(1));
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
    // The selected artist block starts with its border, padding, header, and
    // pinned hint, then renders every album. A 30-row page from album 0's
    // display row lands on album 30.
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        30,
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
    // The selected artist block contains the header, pinned hint, and every
    // album. A 30-row page up from album 35 lands on album 5.
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        5,
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
    // album row, 4 collapsed action hint, 5 colored bottom padding, 6
    // bottom border), then 7 = album 1. With a 1-row
    // page, PageDown targets the hint row, so paging resolves forward to
    // album 1.
    assert_eq!(down_app.libs[0].nav_stack.last().unwrap().cursor, 1);

    let mut up_app = make_power_music_album_list_app(10, 3);
    render_full_app(&mut up_app, 100, 40);
    assert!(up_app.libs[0].album_track_focus.is_none());
    up_app.layout.main.left_area.height = 4;

    let handled = up_app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    // The selected artist block contains the header, pinned hint, and every
    // album. With a 4-row page, PageUp from album 3 resolves to album 0
    // rather than leaving the cursor on the non-album header row.
    assert_eq!(up_app.libs[0].nav_stack.last().unwrap().cursor, 0);
}

#[test]
fn oversized_artist_block_scrolls_inline_without_moving_the_outer_block() {
    let mut app = make_power_music_album_list_app(60, 0);
    render_full_app(&mut app, 100, 40);
    let initial_offset = app.libs[0].nav_stack.last().unwrap().scroll;

    for _ in 0..35 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    render_full_app(&mut app, 100, 40);
    let down_offset = app.libs[0].nav_stack.last().unwrap().scroll;
    assert_eq!(down_offset, initial_offset);
    assert!(app
        .layout
        .main
        .left_row_targets
        .iter()
        .any(|target| matches!(target, Some(LibraryRowTarget::Album(35)))));
    let cursor_y = app
        .layout
        .main
        .cursor_screen_y
        .expect("expected the active album marker on screen");
    let area = app.layout.main.left_area;
    assert!(cursor_y >= area.y && cursor_y < area.y + area.height);

    for _ in 0..35 {
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    }
    render_full_app(&mut app, 100, 40);
    let up_offset = app.libs[0].nav_stack.last().unwrap().scroll;
    assert_eq!(up_offset, initial_offset);
    assert!(app
        .layout
        .main
        .left_row_targets
        .iter()
        .any(|target| matches!(target, Some(LibraryRowTarget::Album(0)))));
}

#[test]
fn oversized_artist_navigation_reaches_hidden_albums_before_following_artist() {
    let mut app = make_power_music_album_list_app(60, 0);
    add_following_artist_albums(&mut app, 2);
    render_full_app(&mut app, 100, 40);

    for expected_cursor in 1..60 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            app.libs[0].nav_stack.last().unwrap().cursor,
            expected_cursor
        );
        assert!(app.libs[0].artist_header_focus.is_none());
    }

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let beta_header = app.libs[0]
        .artist_header_focus
        .as_ref()
        .expect("expected navigation to reach the following artist header");
    assert_eq!(beta_header.artist_label, "Beta");

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert!(app.libs[0].artist_header_focus.is_none());
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 60);
}

#[test]
fn page_down_crosses_oversized_artist_window_to_following_artist() {
    let mut app = make_power_music_album_list_app(60, 59);
    add_following_artist_albums(&mut app, 2);
    render_full_app(&mut app, 100, 40);
    app.layout.main.left_area.height = 1;

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(app.libs[0].artist_header_focus.is_none());
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        60,
        "PageDown should leave the oversized artist at its boundary"
    );
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
