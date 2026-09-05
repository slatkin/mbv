use super::*;
use crate::app::components::msg::{AlbumCursorKind, ShellRequest};
use crate::app::components::Msg;
pub(crate) use crate::app::render::make_music_group_app;
pub(crate) use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
#[test]
fn music_mouse_album_click_emits_and_shell_applies_cursor() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    let mut model = Model::new(app);
    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), 0);
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    model.sync_active_destination();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();
    let id = model.music_workspace_id.clone().unwrap();
    let (browser_area, left_row_targets) = {
        let layout = model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()
            .unwrap()
            .layout();
        (
            layout.wide_music_browser_area,
            layout.left_row_targets.clone(),
        )
    };
    let (row, target) = left_row_targets
        .iter()
        .enumerate()
        .filter_map(|(row, target)| target.as_ref().map(|album| (row, *album)))
        .nth(1)
        .expect("second painted album target");
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: browser_area.x + 1,
            row: browser_area.y + row as u16,
            modifiers: KeyModifiers::NONE,
        }));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target,
            kind: AlbumCursorKind::Move
        }))
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        message.expect("album cursor request"),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), target);
}
#[test]
fn shell_music_shortcuts_use_component_selection() {
    let mut model = Model::new(make_music_group_app());
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.sync_music_workspace();
    model.sync_active_destination();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    let key = |code| {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
    };
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&key(Key::Char('.')));
    let Some(Msg::Shell(ShellRequest::EmbyLibraryContextMenu { item })) = message else {
        panic!("Music '.' must emit a library context-menu request");
    };
    assert_eq!(item.item_type, "MusicAlbum");
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        Msg::Shell(ShellRequest::EmbyLibraryContextMenu { item }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(matches!(
        model.app.pending_overlay,
        Some(crate::app::types_overlay::OverlayRequest::ContextMenu(_))
    ));
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&key(Key::Char('/')));
    assert_eq!(message, Some(Msg::Shell(ShellRequest::OpenInlineSearch)));
}

#[test]
fn shell_mounts_and_syncs_music_workspace() {
    let mut model = Model::new(make_music_group_app());
    // The mounted browser is in the post-hero right rail, whose width—not
    // the pre-hero left area—determines horizontal navigation availability.
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 200, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(100, 0, 100, 30);
    model.sync_music_workspace();
    model.sync_active_destination();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    ));
    // Grouped Music paints one album per row; horizontal keys fall through to
    // the central router rather than being claimed by this component.
    for code in [Key::Left, Key::Right, Key::Char('h'), Key::Char('l')] {
        let message = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }));
        assert_eq!(message, None, "unexpected claim for {code:?}");
    }
}
#[test]
fn push_music_workspace_fetches_selected_album_tracks() {
    let mut model = Model::new(make_music_group_app());
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    model.sync_music_workspace();
    model.sync_active_destination();
    assert!(model.app.album_tracks_loading.contains("album-1"));
    let component = model
        .application
        .get_component(&model.music_workspace_id.clone().unwrap())
        .unwrap()
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .unwrap();
    assert!(
        component.album_tracks_loading(),
        "first mounted content push must project album track loading"
    );
}

#[test]
fn grouped_music_cursor_no_fallthrough_when_left_sorted_indices_empty() {
    let mut model = Model::new(make_music_group_app());
    // Add sibling albums so the display order (sorted by name) differs
    // from raw insertion order: raw [0 "First Album", 1 "Zebra Album",
    // 2 "Mango Album"] sorts to display order [0, 2, 1].
    let mut zebra = crate::app::tests::make_item("Zebra Album", "MusicAlbum");
    zebra.artist = "Charlie".into();
    let mut mango = crate::app::tests::make_item("Mango Album", "MusicAlbum");
    mango.artist = "Bravo".into();
    model.app.libs[0].nav_stack[1].items.extend([zebra, mango]);
    // Force a single column so the display-order move is deterministic.
    model.app.layout.main.left_area.width = 40;

    // No library-list render has run, so the render-output order the
    // legacy fallback would have read is empty.
    assert!(model.app.layout.main.left_sorted_indices.is_empty());

    model.sync_music_workspace();

    model.sync_active_destination();
    let id = model.music_workspace_id.clone().expect("mounted");

    let order = model.app.wide_music_render_ctx(0, None).album_order.clone();
    assert_eq!(order, vec![0, 2, 1], "display order must differ from raw");

    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    let target = match message {
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target,
            kind: AlbumCursorKind::Move,
        })) => target,
        other => panic!("Down must emit an album cursor intent, got {other:?}"),
    };
    // The target is the display-order successor of raw index 0 (== order[1]),
    // never the raw successor (1) the legacy empty-left_sorted_indices path used.
    assert_eq!(target, order[1]);
    assert_ne!(target, 1, "must not fall through to raw-index navigation");

    // The shell arm applies the target via the display-order cursor setter,
    // which must not fall through to raw-index navigation.
    assert!(model.app.move_music_group_display_cursor(0, target));
    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), order[1]);
}

#[test]
fn shell_executes_grouped_music_image_paint() {
    let mut model = Model::new(make_music_group_app());
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.image_protocol_enabled = true;
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    model.sync_active_destination();

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();

    assert!(model.app.card_image_loading.contains("album-1:P"));
}

#[test]
fn narrow_music_workspace_emits_selected_album_image_request() {
    let mut model = Model::new(make_music_group_app());
    model.app.image_protocol_enabled = true;
    model.app.layout.main.left_area = ratatui::layout::Rect::new(0, 0, 81, 20);
    model.sync_music_workspace();
    model.sync_active_destination();

    let mut terminal = Terminal::new(TestBackend::new(81, 20)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();

    assert!(
        model.app.card_image_loading.contains("album-1:P"),
        "narrow selected album must emit a typed image-loading request"
    );
}

#[test]
fn shell_mounts_music_workspace_in_narrow_mode() {
    let mut model = Model::new(make_music_group_app());
    assert!(model.app.is_music_group_view(0));
    assert!(model.app.is_viewing_album_folders(0));
    assert!(
        !(model.app.layout.main.wide_music_right_area.width > 0
            && model.app.layout.main.wide_music_right_area.height > 0)
    );

    let wide_area = model.app.layout.main.wide_music_area;
    assert_eq!(wide_area.width, 0);
    assert_eq!(wide_area.height, 0);
    model.sync_music_workspace();
    model.sync_active_destination();
    let id = model
        .music_workspace_id
        .clone()
        .expect("narrow Music workspace mounted");
    assert!(model.application.mounted(&id));
    assert_eq!(model.app.layout.main.wide_music_area, wide_area);
}
