#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, TabSelection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{Read, Write};

/// Music library sitting on the album-folder-listing nav
/// level (`is_viewing_album_folders` holds): a grouped `["group",
/// "album"]` config, mirroring `render_library`'s inline-detail
/// tests, with two albums at the album level and `album-1` selected.
pub(super) fn make_music_album_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::Library(0);
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
        search: None,
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
                music_grouping: None,
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
                music_grouping: None,
            },
        ],
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

pub(super) fn add_beta_album(app: &mut App) {
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

pub(super) fn push_tracks(app: &mut App, album_id: &str, count: usize) {
    let tracks: Vec<_> = (0..count)
        .map(|i| {
            let mut t = make_item(&format!("Track {i}"), "Audio");
            t.id = format!("{album_id}-track-{i}");
            t
        })
        .collect();
    app.album_tracks_cache.insert(album_id.to_string(), tracks);
}

pub(super) fn make_music_album_list_app(album_count: usize, cursor: usize) -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::Library(0);
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
        search: None,
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
                music_grouping: None,
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
                music_grouping: None,
            },
        ],
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

pub(super) fn add_following_artist_albums(app: &mut App, album_count: usize) {
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

pub(super) fn render_full_app(app: &mut App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();
}

pub(super) struct RecursiveFetchServer {
    base_url: String,
    seen_parent_ids: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

pub(super) type RecursiveFetchResponses = Vec<(
    &'static str,
    Result<Vec<(&'static str, &'static str, i64)>, &'static str>,
)>;

impl RecursiveFetchServer {
    pub(super) fn start(responses: RecursiveFetchResponses) -> Self {
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

    pub(super) fn seen_parent_ids(&self) -> Vec<String> {
        self.seen_parent_ids.lock().unwrap().clone()
    }

    pub(super) fn first_seen_parent_ids(&self) -> Vec<String> {
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

pub(super) fn configure_recursive_fetch_server(app: &mut App, server: &RecursiveFetchServer) {
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = server.base_url.clone();
    *app.config.lock().unwrap() = config.clone();
    let mut client = mbv_core::api::EmbyClient::new(config);
    client.user_id = "user-1".into();
    client.token = "token-1".into();
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
}
