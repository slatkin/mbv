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
use crate::app::components::{
    ComponentId, ContextMenuComponent, HomeComponent, Msg, OverlayId, ShellRequest,
    TerminalObserverEvent,
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
        model.app.compose_base_frame(f, None);
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

/// Task 5.3d, Home menu-placement geometry: when Home is the active
/// destination with Library focus, the shell places the context menu from the
/// mounted `HomeComponent`'s own painted geometry — never the legacy
/// `AppLayout.left_area`/`selected_item_rect` copies. To prove the source, the
/// legacy copies are poisoned far outside the panel and the menu must still
/// land exactly where the component's paint implies, while a fallback to the
/// poisoned rect would land elsewhere. Paint only the Home component (and the
/// overlay), never the legacy `App::render` underpaint, so the poisoned legacy
/// copies stay stale for the whole placement.
#[test]
fn home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout() {
    use crate::app::types_context_menu::ContextMenu;
    let _guard = crate::config::TestStateDirGuard::new();

    let mut model = crate::app::shell::Model::new(make_app_stub());
    model.app.tab = TabSelection::Home;
    model.app.panel_focus = PanelFocus::Library;
    model.home_content.continue_items = make_items(5);
    model.handle_home_request(ShellRequest::HomeContextMenu {
        home_cw_selected: model.home_continue_watching_selected(),
        cw_item: model.home_cw_item(),
    });
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(super::types_overlay::OverlayRequest::ContextMenu(_))
        ),
        "'.' should open the context menu"
    );

    model.sync_modal_requests();
    model.push_home_content();
    model.app.layout.main.home_area = Rect::new(0, 0, 80, 24);

    let backend = TestBackend::new(80, 24);
    let mut term = Terminal::new(backend).unwrap();
    // Paint only the Home component so its `view()` produces the authoritative
    // placed geometry while the legacy `AppLayout` copies stay untouched.
    term.draw(|f| model.render_home_component(f)).unwrap();

    // Capture the component-painted geometry the shell must anchor to, then
    // poison the corresponding legacy copies far outside the panel.
    let home = model
        .application
        .get_component(&ComponentId::Home)
        .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
        .expect("Home component mounted");
    let (panel, selected) = home.menu_placement_geometry();
    model.app.layout.main.left_area = Rect::new(0, 0, 200, 200);
    model.app.layout.main.selected_item_rect = Some(Rect::new(500, 500, 1, 1));

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

    // A fallback to the poisoned legacy geometry would land the menu at the
    // panel's bottom-right corner instead of the component's placement.
    let (px, py) = ContextMenu::place(
        Rect::new(0, 0, 200, 200),
        size,
        Some(&Rect::new(500, 500, 1, 1)),
        None,
    );
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
            model.app.compose_base_frame(f, None);
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
/// refresh `layout.main.tv_wide_left_area`/`tv_wide_right_area`.
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

    // Poison the narrow-branch and wide-branch panel rects to distinct,
    // stale values that this tick never repaints, so the rendered menu
    // position reveals which branch `context_menu_rect` took.
    let narrow_panel = Rect::new(1, 1, 5, 5);
    let wide_panel = Rect::new(50, 0, 10, 20);
    model.app.layout.main.left_area = narrow_panel;
    model.app.layout.main.tv_wide_left_area = wide_panel;
    model.app.layout.main.tv_wide_right_area = Rect::new(70, 0, 10, 20);

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

    // Read the branch back immediately, with no intervening repaint of the
    // TV workspace: `tv_wide_left_area`/`tv_wide_right_area` still hold the
    // poisoned values set above, yet the pointer anchor must already resolve
    // to the wide-TV panel because the gate is paint-free.
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
