//! Regression coverage for the narrow browse surfaces, seeded by
//! `migrate-narrow-browse-to-components` (archived) and since grown to cover
//! adjacent feeds/hero work. All tests are live; none are `#[ignore]`d.
//!
//! Groups:
//! - Saved-position restore seam (`LibEvent::RestoreLibraryPosition`): restore
//!   still writes the resting `BrowseLevel` cursor a later content projection
//!   hands the owning component.
//! - Painted-selection movement under `j`/`k` for TV.
//! - `*_paints_each_browse_row_once`: the double-paint guard — assert on the
//!   `TestBackend` buffer through the full `Model::draw_frame` path, red only
//!   if both the legacy painter and the component `view` run for one surface.
//! - `feed_home_video_group_*`: shared inline/wide hero placement, scroll, and
//!   frame completeness for the Home feed video group.
//!
//! The `wide_podcast_*` group (wide Audiobookshelf podcast show-row
//! snapshot/paint) was deleted with the show browser
//! (reorganize-podcast-pill-navigation 4.3): the surname-bucket pills and
//! show rows it pinned no longer exist, and the tab's paint ownership lives
//! on in `src/app/render/tests_podcast_panel.rs` (one pill bar, grouped
//! split episode rows, Workspace-free Wide hero, one painter per surface).

use super::*;
use crate::app::components::emby_library_content::EmbyLibraryContent as BrowserOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

fn folder_items(prefix: &str, item_type: &str, n: usize) -> Vec<mbv_core::api::EmbyItem> {
    (0..n)
        .map(|i| {
            let mut item = make_item(&format!("{prefix} {i}"), item_type);
            item.id = format!("{prefix}-{i}");
            item.is_folder = true;
            item
        })
        .collect()
}

// ── Characterization: saved-position restore (green now, green after) ─────────

fn narrow_backend() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(60, 20)).unwrap()
}

fn buffer_text(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw(model: &mut Model, term: &mut Terminal<TestBackend>) -> String {
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    buffer_text(term)
}

/// The embedded Movies/HomeVideos/Generic owner's cursor, read through the
/// mounted `LibraryPanel` (task 6.1: the panel is the library surface's one
/// event boundary; the owner is never a component).
fn owner_cursor(model: &mut Model) -> usize {
    let (_, key, _) = model
        .active_emby_library_owner()
        .expect("the active library's owner has migrated");
    model
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any_mut()
        .downcast_mut::<LibraryPanel>()
        .expect("Library panel type")
        .owner_mut(&key)
        .and_then(|owner| owner.as_any_mut().downcast_mut::<BrowserOwner>())
        .map(|owner| owner.cursor())
        .expect("browser owner installed")
}

/// Feed one key into whatever component currently holds focus and route any
/// emitted `Msg` the way the run loop does. Pre-migration the narrow browse
/// surfaces have no owning component, so focus rests on `UiRoot` and the key
/// is dead — which is exactly what the ignored tests document.
fn press(model: &mut Model, code: Key) {
    let focused = model.application.focus().cloned();
    if let Some(id) = &focused {
        let msg = model
            .application
            .get_component_mut(id)
            .expect("focused component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }));
        if let Some(msg) = msg {
            let mut music_resize = false;
            let mut tv_resize = false;
            model.handle_terminal_message(msg, &mut music_resize, &mut tv_resize);
        }
    }
    model.sync_mounted_surfaces();
}

fn tv_shows_app() -> App {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.collection_type = "tvshows".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items: folder_items("Series", "Series", 5),
            total_count: 5,
            resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
            item_types: Some("Series".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    app
}

/// Regression 3: narrow TV `j` moves the painted selection. The panel-hosted
/// `TvContent` owner owns the surface at every breakpoint (task 8.4), so the
/// painted selection lives in the mounted panel's retained geometry, keyed
/// off the owner's local cursor.
#[test]
fn narrow_tv_browse_j_moves_painted_selection() {
    let mut model = Model::new(tv_shows_app());
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    // Seed past the selected-Series inline hero (which swallows its own row)
    // so both samples are plain rows.
    model.test_tv_owner_mut().set_cursor_for_test(1);
    draw(&mut model, &mut term);
    let before = model.test_painted_library_layout().selected_item_rect;
    assert!(
        before.is_some(),
        "narrow TV browse must paint a selected row"
    );

    press(&mut model, Key::Char('j'));
    draw(&mut model, &mut term);
    let after = model.test_painted_library_layout().selected_item_rect;

    assert_ne!(
        before, after,
        "j must move the painted selection down the narrow TV series list"
    );
}

fn feed_home_video_group_app() -> App {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut first = make_item("Video One", "Movie");
    first.id = "video-one".into();
    first.runtime_ticks = 3_600 * 10_000_000;
    first.genres = vec!["Family".into()];
    first.overview = "Distinctive wrapping overview fragment for inline expansion.".into();
    let mut second = make_item("Video Two", "Movie");
    second.id = "video-two".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
            total_count: 1,
            resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![first.clone(), second.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![first, second],
            }],
            loading: false,
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });
    app
}

#[test]
fn feed_home_video_group_browser_wheel_keeps_control_cursor_authoritative() {
    let mut app = feed_home_video_group_app();
    app.terminal_width = 140;
    app.terminal_height = 40;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let state = app.libs[0].feed_home_video.as_mut().unwrap();
    for i in 0..30 {
        let mut item = make_item(&format!("Video extra {i}"), "Movie");
        item.id = format!("video-extra-{i}");
        state.groups[0].items.push(item.clone());
        state.all_items.push(item);
    }
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    // The wide fixed-row control gives a durable one-row-per-item geometry.
    let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
    draw(&mut model, &mut term);
    let list_area = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(LibraryPanel::test_list_rect)
        .expect("the Library panel painted a list slot");
    let total_rows = model.app.libs[0]
        .feed_home_video
        .as_ref()
        .unwrap()
        .selected_len();
    assert!(total_rows > 2, "the feed fixture must hold several rows");
    assert!(
        list_area.width > 0 && list_area.height > 0,
        "the panel's list slot must have painted"
    );
    let mut music_resize = false;
    let mut tv_resize = false;

    // Seed the control selection at the last row through the panel's
    // keyboard delivery (task 6.1: the panel forwards the chord to the
    // embedded owner, which echoes its resolved index).
    let end = model
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("Library panel mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::End,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("End emits the typed cursor echo");
    model.handle_terminal_message(end, &mut music_resize, &mut tv_resize);
    draw(&mut model, &mut term);
    assert_eq!(
        owner_cursor(&mut model),
        total_rows - 1,
        "End selects the last row"
    );
    assert_eq!(
        model.app.libs[0]
            .feed_home_video
            .as_ref()
            .unwrap()
            .video_cursor,
        total_rows - 1,
        "the shell resting cursor follows the control selection"
    );

    // One wheel notch through the mounted panel: the control resolves its
    // own new index and the typed `EmbyLibraryCursorIndex` echo persists it as
    // the shell's resting `video_cursor` — the control is authoritative and
    // the shell follows, never the reverse.
    let wheel = model
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("Library panel mounted")
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: list_area.x + 1,
            row: list_area.y + 1,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("the wheel emits the typed cursor echo");
    assert!(
        matches!(
            wheel,
            Msg::Shell(ref shell_boxed)
                if matches!(
                    shell_boxed.as_ref(),
                    ShellRequest::EmbyLibraryCursorIndex { index } if *index == total_rows - 2
                )
        ),
        "the wheel echo carries the control's resolved index: {wheel:?}"
    );
    model.handle_terminal_message(wheel, &mut music_resize, &mut tv_resize);
    // The wheel move re-resolves the viewport and invalidates the retained
    // frame; production draws between events, so refresh the painted claim
    // here too (design.md D6).
    draw(&mut model, &mut term);
    assert_eq!(
        owner_cursor(&mut model),
        total_rows - 2,
        "the wheel moves the control one row up"
    );
    assert_eq!(
        model.app.libs[0]
            .feed_home_video
            .as_ref()
            .unwrap()
            .video_cursor,
        total_rows - 2,
        "the shell resting state follows the resolved control selection"
    );
}
