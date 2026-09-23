//! Integration tests for context-menu anchor resolution (design §3):
//! keyboard placement must follow the *fresh* frame layout after a resize,
//! and mouse (pointer) placement must remain anchored to the click point.
//!
//! The menu is now an Interactive Component mounted by the shell from
//! `app.pending_overlay`; the shell recomputes its rect from `AppLayout`
//! each frame (task 5.3c). These tests drive `Model::render_context_menu_overlay`,
//! which sets the component's rect, then read `ContextMenuComponent::menu_rect()`.
use super::tests_podcast::add_emby_movie_library;
use super::*;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{
    ComponentId, ContextMenuComponent, Msg, OverlayId, TerminalObserverEvent,
};
use crate::app::shell::Model;
use crate::app::tests::*;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn library_app() -> App {
    let mut app = make_app_stub();
    add_emby_movie_library(&mut app);
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

/// A wide-TV-eligible library (task 3.5): a "tvshows" collection whose
/// nav-stack top level holds only `Series` items, matching `is_wide_tv_library`.
fn add_emby_tv_library(app: &mut App) {
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.collection_type = "tvshows".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
        fetched_rows: 0,
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items: vec![make_item("The Series", "Series")],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
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
}

fn mounted_context_menu_rect(model: &Model) -> Rect {
    let id = ComponentId::Overlay(OverlayId::ContextMenu);
    model
        .application
        .get_component(&id)
        .expect("context menu mounted")
        .as_any()
        .downcast_ref::<ContextMenuComponent>()
        .expect("context menu type")
        .menu_rect()
}

fn render_at(model: &mut Model, width: u16, height: u16) -> (u16, u16) {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        model.app.compose_root_frame(f);
        model.render_context_menu_overlay(f);
    })
    .unwrap();
    let rect = mounted_context_menu_rect(model);
    (rect.x, rect.y)
}

#[test]
fn pointer_placement_stays_click_anchored_not_following_selection() {
    let mut app = library_app();
    app.open_context_menu_at(70, 20, false, None);

    let mut model = Model::new(app);
    model.sync_modal_requests();

    // Render once and capture the rect placed from the click point.
    let a = render_at(&mut model, 100, 40);

    // Move the underlying selection so the keyboard anchor (selected-item
    // rect) would sit elsewhere; the pointer anchor must not chase it.
    if let Some(ref mut level) = model.app.libs[0].nav_stack.last_mut() {
        level.set_resting_cursor(level.items.len().saturating_sub(1));
    }
    let b = render_at(&mut model, 100, 40);

    // Same fresh frame size, same click point: the pointer menu does not move
    // when the selection changes.
    assert_eq!(
        a, b,
        "pointer placement must stay click-anchored and ignore the selected item: {a:?} != {b:?}"
    );
}

/// Task 5.3d + 5.11, Home menu-placement geometry: when the Home owner is
/// the migrated active destination with Library focus, the shell places the
/// context menu from the mounted `LibraryPanel`'s own painted geometry —
/// never the legacy `AppLayout.left_area`/`selected_item_rect` copies. To
/// prove the source, the legacy copies are poisoned far outside the panel
/// and the menu must still land exactly where the panel's paint implies,
/// while a fallback to the poisoned rect would land elsewhere. Paint only
/// the panel (and the overlay), never the legacy `App::render` underpaint,
/// so the poisoned legacy copies stay stale for the whole placement.
#[test]
fn home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout() {
    use crate::app::types_context_menu::ContextMenu;
    let _guard = crate::config::TestStateDirGuard::new();

    let mut model = crate::app::shell::Model::new(make_app_stub());
    model.app.tab = TabSelection::Home;
    model.app.panel_focus = PanelFocus::Library;
    model.home_content.continue_items = make_items(5);
    model.handle_terminal_message(Msg::Shell(crate::app::components::ShellRequest::RowContextMenu(
        crate::app::types_context_menu::ContextMenuTargets::Home(vec![crate::app::components::msg::HomeRowTarget {
            item_id: Some("id0".into()),
            source: None,
            from_continue_watching: true,
        }]),
        None,
    )), &mut false, &mut false);
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(super::types_overlay::OverlayRequest::ContextMenu(_))
        ),
        "'.' should open the context menu"
    );

    model.sync_modal_requests();
    model.push_home_content();
    // The panel paints from `RootFrame.library` (task 5.9); publish it
    // directly, wide enough for the Wide skeleton's split.
    let library_area = Rect::new(0, 0, 100, 30);
    model.app.layout.root_frame.library = Some(library_area);
    // The mounted panel only paints its active owner's surface; the shell's
    // sync pass points it at the Home owner (`sync_library_panel`), which
    // this test drives directly rather than through `sync_mounted_surfaces`.
    model.sync_library_panel();

    let backend = TestBackend::new(100, 30);
    let mut term = Terminal::new(backend).unwrap();
    // Paint only the panel so its `view()` produces the authoritative placed
    // geometry while the legacy `AppLayout` copies stay untouched.
    let area = model.app.layout.root_frame.library.unwrap();
    term.draw(|f| model.render_library_panel_at(f, area)).unwrap();

    // Capture the panel-painted geometry the shell must anchor to, then
    // poison the corresponding legacy copies far outside the panel.
    let painted = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel mounted")
        .menu_geometry()
        .expect("panel painted geometry");
    let (panel, selected) = painted;
    model.app.layout.left_area = Rect::new(0, 0, 200, 200);

    term.draw(|f| model.render_context_menu_overlay(f)).unwrap();

    let rect = mounted_context_menu_rect(&model);

    let id = ComponentId::Overlay(OverlayId::ContextMenu);
    let (size, anchor) = {
        let comp = model
            .application
            .get_component(&id)
            .expect("context menu mounted")
            .as_any()
            .downcast_ref::<ContextMenuComponent>()
            .expect("context menu type");
        (ContextMenu::rendered_size(comp.entries()), comp.anchor())
    };
    // Keyboard anchor: `SelectedItem(Library)` resolved to the component's
    // painted panel + selected rect (no pointer).
    assert!(matches!(
        anchor,
        ContextMenuAnchor::SelectedItem(PanelFocus::Library)
    ));
    let (ex, ey) = ContextMenu::place(panel, size, selected.as_ref(), None);
    let expected = Rect::new(ex, ey, size.0, size.1);
    assert_eq!(
        rect, expected,
        "menu must anchor to the component-painted Home geometry, got {rect:?}"
    );

    // A fallback to the poisoned legacy `left_area` (the shell mirror no
    // longer carries a `selected_item_rect` to poison at all) would land the
    // menu at the panel's bottom-right corner instead of the component's
    // placement.
    let (px, py) = ContextMenu::place(Rect::new(0, 0, 200, 200), size, None, None);
    assert_ne!(
        (rect.x, rect.y),
        (px, py),
        "menu must not fall back to the poisoned AppLayout geometry"
    );
}

/// Task 5.11 review correction: the Narrow-mode counterpart of
/// `home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout`.
/// `LibraryPanel::menu_geometry()` only read `wide_geometry`, so when Home is
/// active in Narrow mode with `PanelFocus::Library` it returned `None` and the
/// shell fell through to the poisoned legacy `AppLayout` copies. Force the
/// Narrow skeleton (width below `TWO_COLUMN_THRESHOLD`) and prove the menu
/// still anchors to the panel's own Narrow-painted geometry.
#[test]
fn home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout_narrow() {
    use crate::app::types_context_menu::ContextMenu;
    let _guard = crate::config::TestStateDirGuard::new();

    let mut model = crate::app::shell::Model::new(make_app_stub());
    model.app.tab = TabSelection::Home;
    model.app.panel_focus = PanelFocus::Library;
    model.home_content.continue_items = make_items(5);
    model.handle_terminal_message(Msg::Shell(crate::app::components::ShellRequest::RowContextMenu(
        crate::app::types_context_menu::ContextMenuTargets::Home(vec![crate::app::components::msg::HomeRowTarget {
            item_id: Some("id0".into()),
            source: None,
            from_continue_watching: true,
        }]),
        None,
    )), &mut false, &mut false);
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(super::types_overlay::OverlayRequest::ContextMenu(_))
        ),
        "'.' should open the context menu"
    );

    model.sync_modal_requests();
    model.push_home_content();
    // Narrow enough (below `TWO_COLUMN_THRESHOLD`) to force the Narrow
    // skeleton instead of the Wide split.
    let library_area = Rect::new(0, 0, 60, 30);
    model.app.layout.root_frame.library = Some(library_area);
    model.sync_library_panel();

    let backend = TestBackend::new(60, 30);
    let mut term = Terminal::new(backend).unwrap();
    // Paint only the panel so its `view()` produces the authoritative placed
    // geometry while the legacy `AppLayout` copies stay untouched.
    let area = model.app.layout.root_frame.library.unwrap();
    term.draw(|f| model.render_library_panel_at(f, area)).unwrap();

    // Capture the panel-painted geometry the shell must anchor to, then
    // poison the corresponding legacy copies far outside the panel.
    let painted = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel mounted")
        .menu_geometry()
        .expect("panel painted geometry");
    let (panel, selected) = painted;
    // The non-Wide anchor is the Wide pane's panel rect (unify-narrow 6.1):
    // the pane the frame painted and retained, not the row-flow inset the
    // pre-parity path anchored to.
    let narrow = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.test_narrow_geometry())
        .expect("the non-Wide frame painted");
    assert_eq!(
        panel, narrow.list_panel,
        "the anchor is the painted Browser pane"
    );
    assert_ne!(
        panel, narrow.list_area,
        "the anchor is the pane, not the row-flow inset"
    );
    model.app.layout.left_area = Rect::new(0, 0, 200, 200);

    term.draw(|f| model.render_context_menu_overlay(f)).unwrap();

    let rect = mounted_context_menu_rect(&model);

    let id = ComponentId::Overlay(OverlayId::ContextMenu);
    let (size, anchor) = {
        let comp = model
            .application
            .get_component(&id)
            .expect("context menu mounted")
            .as_any()
            .downcast_ref::<ContextMenuComponent>()
            .expect("context menu type");
        (ContextMenu::rendered_size(comp.entries()), comp.anchor())
    };
    // Keyboard anchor: `SelectedItem(Library)` resolved to the component's
    // painted panel + selected rect (no pointer).
    assert!(matches!(
        anchor,
        ContextMenuAnchor::SelectedItem(PanelFocus::Library)
    ));
    let (ex, ey) = ContextMenu::place(panel, size, selected.as_ref(), None);
    let expected = Rect::new(ex, ey, size.0, size.1);
    assert_eq!(
        rect, expected,
        "menu must anchor to the component-painted Narrow Home geometry, got {rect:?}"
    );

    // A fallback to the poisoned legacy `left_area` (the shell mirror no
    // longer carries a `selected_item_rect` to poison at all) would land the
    // menu at the panel's bottom-right corner instead of the component's
    // placement.
    let (px, py) = ContextMenu::place(Rect::new(0, 0, 200, 200), size, None, None);
    assert_ne!(
        (rect.x, rect.y),
        (px, py),
        "menu must not fall back to the poisoned AppLayout geometry"
    );
}

#[test]
fn context_menu_entries_render_below_the_reserved_top_row() {
    let mut app = library_app();
    app.open_context_menu(false, None);

    let mut model = Model::new(app);
    model.sync_modal_requests();

    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    // The shell sets the component rect from `AppLayout` and the component
    // paints via `render_context_menu_content` (task 2.5 / 5.3c).
    terminal
        .draw(|f| {
            model.app.compose_root_frame(f);
            model.render_context_menu_overlay(f);
        })
        .unwrap();

    let rect = mounted_context_menu_rect(&model);
    let first_label = terminal
        .backend()
        .buffer()
        .cell((rect.x + 1, rect.y + 1))
        .unwrap();
    assert_eq!(first_label.symbol(), "P");
}

/// Task 3.5 (replace-wide-paint-inference): the pointer-anchor branch at
/// `shell_overlays_menus.rs`'s `context_menu_rect` gates on
/// `App::wide_tv_library_area`, a paint-free predicate driven solely by the
/// terminal size (not by the previous frame's `AppLayout` paint). This must
/// select the wide-TV branch on the very tick a resize lands — before
/// `Application::view`/any repaint of the TV workspace has a chance to
/// refreshes the TV component's retained geometry.
///
/// The resize is driven through the real `Msg::TerminalEvent(Resize)` path
/// (`Model::handle_terminal_message`), and the branch is read back via
/// `render_context_menu_overlay`, which never repaints the TV workspace
/// itself (it only recomputes+paints the context-menu overlay from already-
/// mounted geometry). The narrow/wide `AppLayout` panel rects are poisoned to
/// distinct, deliberately stale values beforehand, so the assertion proves
/// the branch decision — not the geometry it happens to land on.
#[test]
fn pointer_anchor_selects_wide_tv_branch_on_resize_tick_before_repaint() {
    use crate::app::types_context_menu::ContextMenu;

    let mut app = make_app_stub();
    add_emby_tv_library(&mut app);
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 60;
    app.terminal_height = 24;
    // Below `MINI_VIEW_THRESHOLD`, `effective_panel_focus` reads
    // `mini_view_focus` (default `Queue`) instead of `panel_focus`.
    app.mini_view_focus = PanelFocus::Library;
    let click = (55u16, 10u16);
    app.open_context_menu_at(click.0, click.1, false, None);

    let mut model = Model::new(app);
    model.sync_modal_requests();

    // Poison the narrow fallback; the Wide branch derives its panel from the
    // current terminal geometry rather than a previous-frame layout field.
    let narrow_panel = Rect::new(1, 1, 5, 5);
    model.app.layout.left_area = narrow_panel;

    let entries = {
        let id = ComponentId::Overlay(OverlayId::ContextMenu);
        model
            .application
            .get_component(&id)
            .expect("context menu mounted")
            .as_any()
            .downcast_ref::<ContextMenuComponent>()
            .expect("context menu type")
            .entries()
            .to_vec()
    };
    let size = ContextMenu::rendered_size(&entries);

    // Still at the narrow terminal size: the pointer anchor must not take
    // the wide-TV branch, resolving against the plain `left_area` instead.
    let backend = TestBackend::new(60, 24);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.render_context_menu_overlay(f)).unwrap();
    let narrow_rect = mounted_context_menu_rect(&model);
    let (nx, ny) = ContextMenu::place(narrow_panel, size, None, Some(click));
    assert_eq!(
        (narrow_rect.x, narrow_rect.y),
        (nx, ny),
        "narrow terminal must anchor to the plain left_area, not the wide-TV panel: {narrow_rect:?}"
    );

    // Resize to a wide terminal on this same tick via the real dispatch path.
    let mut music_resize = false;
    let mut tv_resize = false;
    model.handle_terminal_message(
        Msg::TerminalEvent(TerminalObserverEvent::Resize {
            width: 150,
            height: 24,
        }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.terminal_width, 150);

    // Read the branch back immediately, before repainting the TV workspace;
    // the pointer anchor must already resolve to the derived Wide panel.
    let wide_panel = model
        .app
        .wide_tv_library_area(0)
        .expect("TV is wide after resize");
    let backend = TestBackend::new(150, 24);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.render_context_menu_overlay(f)).unwrap();
    let wide_rect = mounted_context_menu_rect(&model);
    let (wx, wy) = ContextMenu::place(wide_panel, size, None, Some(click));
    assert_eq!(
        (wide_rect.x, wide_rect.y),
        (wx, wy),
        "after resize, before any TV-workspace repaint, the pointer anchor must already select the wide-TV branch: {wide_rect:?}"
    );
}

/// Wide hero-left regression (2026-09-14): the hero pane moved to the left
/// and the list pane to the right, but the pointer-anchor fallback in
/// `context_menu_rect` still clamped non-Home/non-Books migrated destinations
/// (Music, Browser, Feeds, ...) inside the legacy `AppLayout.left_area` —
/// the queue column on the far left. A right-click on a list album therefore
/// opened at the right height but on the left. The shell must clamp to the
/// panel's own painted list geometry (tab-agnostic `library_menu_geometry`),
/// so the menu stays at the click point inside the right-hand list pane.
#[test]
fn migrated_music_pointer_menu_stays_in_painted_list_pane_not_queue_column() {
    use crate::app::render::make_music_group_app;
    use crate::app::types_context_menu::ContextMenu;

    let mut app = make_music_group_app();
    app.panel_focus = PanelFocus::Library;
    let mut model = Model::new(app);
    // Wide terminal so the panel paints the Wide skeleton (hero left, list
    // right) and `effective_panel_focus` reads `panel_focus`.
    model.app.terminal_width = 150;
    model.app.terminal_height = 40;
    model.app.layout.root_frame.library = Some(Rect::new(0, 0, 150, 40));
    model.sync_library_panel();

    let backend = TestBackend::new(150, 40);
    let mut term = Terminal::new(backend).unwrap();
    let area = model.app.layout.root_frame.library.unwrap();
    term.draw(|f| model.render_library_panel_at(f, area)).unwrap();

    let (list_panel, _) = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel mounted")
        .menu_geometry()
        .expect("panel painted geometry");
    // Click inside the painted list pane (the right-hand pane after the
    // hero-left swap); the menu is small enough to fit without clamping.
    let click = (list_panel.x + 4, list_panel.y + 4);
    assert!(
        click.0 < list_panel.right() && click.1 < list_panel.bottom(),
        "click {click:?} must sit inside the painted list pane {list_panel:?}"
    );

    model.app.open_context_menu_at(click.0, click.1, false, None);
    model.sync_modal_requests();
    // Poison the legacy queue-column copy far from the list pane: the stale
    // fallback would clamp the right-hand click to this left-hand column.
    let stale_queue_column = Rect::new(0, 0, 30, 40);
    model.app.layout.left_area = stale_queue_column;

    term.draw(|f| model.render_context_menu_overlay(f)).unwrap();
    let rect = mounted_context_menu_rect(&model);

    let id = ComponentId::Overlay(OverlayId::ContextMenu);
    let (size, anchor) = {
        let comp = model
            .application
            .get_component(&id)
            .expect("context menu mounted")
            .as_any()
            .downcast_ref::<ContextMenuComponent>()
            .expect("context menu type");
        (ContextMenu::rendered_size(comp.entries()), comp.anchor())
    };
    assert!(matches!(
        anchor,
        ContextMenuAnchor::Pointer { x, y } if (x, y) == click
    ));
    let (ex, ey) = ContextMenu::place(list_panel, size, None, Some(click));
    assert_eq!(
        (rect.x, rect.y),
        (ex, ey),
        "pointer menu must stay at the click inside the painted list pane, got {rect:?}"
    );
    let stale = ContextMenu::place(stale_queue_column, size, None, Some(click));
    assert_ne!(
        (rect.x, rect.y),
        stale,
        "menu must not fall back to the stale queue-column clamp"
    );
}
