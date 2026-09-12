#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::components::{
    BrowserComponent, ComponentId, MusicWorkspaceComponent, QueueComponent, TvWorkspaceComponent,
};
use crate::app::layout::{AppLayout, LayoutPlayback};
use crate::app::render::components::widgets::render_right_scrollbar_with_viewport;
use crate::app::shell::Model;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::types_audiobookshelf_browse::{
    build_surname_buckets, AudiobookshelfBookBrowseState,
};
use crate::app::{App, PanelFocus};
use crate::app::{BrowseLevel, LibraryTab, QueueScope, RemoteSlotState, TabSelection};
use crate::config::Config;
use mbv_core::api::EmbyClient;
use mbv_core::api::EmbyItem;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

#[path = "test_helpers_mounted.rs"]
mod mounted;
pub use mounted::*;
#[path = "test_helpers_fixtures.rs"]
mod fixtures;
pub use fixtures::*;

pub fn set_browser_cursor_for_test(model: &mut crate::app::shell::Model, cursor: usize) {
    model.sync_mounted_surfaces();
    let id = model.emby_browser_id.clone().expect("browser mounted");
    model
        .application
        .get_component_mut(&id)
        .expect("browser component")
        .as_any_mut()
        .downcast_mut::<BrowserComponent>()
        .expect("browser component type")
        .set_cursor_for_test(cursor);
}

/// Seed the merged TV owner's authoritative selection directly (mirrors
/// `set_browser_cursor_for_test`, task 8.1: TV routes through
/// `TvWorkspaceComponent` at every breakpoint now).
pub fn set_tv_cursor_for_test(model: &mut crate::app::shell::Model, cursor: usize) {
    model.sync_mounted_surfaces();
    let id = model.tv_workspace_id.clone().expect("tv workspace mounted");
    model
        .application
        .get_component_mut(&id)
        .expect("tv workspace component")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("tv workspace component type")
        .set_cursor_for_test(cursor);
}

pub fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
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

pub fn render_sidebar_scrollbar_column(total: usize, visible: u16, scroll: usize) -> String {
    let backend = TestBackend::new(1, visible);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        super::components::chrome::render_sidebar_scrollbar(
            f,
            Rect::new(0, 0, 0, visible),
            total,
            scroll,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

pub fn render_scrollbar_column(height: u16, max_offset: usize, offset: usize) -> String {
    let backend = TestBackend::new(1, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_right_scrollbar(
            f,
            Rect::new(0, 0, 1, height),
            max_offset,
            offset,
            palette::TEXT_METADATA,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

pub fn render_scrollbar_column_with_viewport(
    height: u16,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) -> String {
    let backend = TestBackend::new(1, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_right_scrollbar_with_viewport(
            f,
            Rect::new(0, 0, 1, height),
            content_length,
            viewport_content_length,
            offset,
            palette::TEXT_METADATA,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

pub fn render_pill_bar_hitboxes(
    labels: &[String],
    ids: &[usize],
    selected_pos: usize,
    width: u16,
) -> Vec<(Rect, usize)> {
    let backend = TestBackend::new(width, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut tabs = Vec::new();
    term.draw(|f| {
        tabs = render_pill_bar(
            f,
            Rect::new(0, 0, width, 1),
            PillBar {
                labels,
                ids,
                selected_pos,
                prefix: None,
            },
        );
    })
    .unwrap();
    tabs
}

pub fn assert_surface_pills(
    terminal: &Terminal<TestBackend>,
    layout: &LayoutMain,
    panel: Rect,
    expected_pill_rows: usize,
    spacer_bg: Color,
    expected_ids: &[usize],
    expected_labels: &[&str],
    selected_id: usize,
) {
    assert_eq!(
        layout
            .selector_tabs
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<_>>(),
        expected_ids,
        "surface pill targets"
    );
    let first = layout
        .selector_tabs
        .first()
        .expect("surface should publish pill targets")
        .0;
    assert!(
        layout
            .selector_tabs
            .iter()
            .all(|(rect, _)| rect.y == first.y && rect.height == 1),
        "pill hitboxes must occupy one shared row: {:?}",
        layout.selector_tabs
    );
    let buffer = terminal.backend().buffer();
    let painted_rows = (panel.y..panel.bottom())
        .filter(|y| (panel.x..panel.right()).any(|x| matches!(buffer[(x, *y)].symbol(), "◢" | "◤")))
        .collect::<Vec<_>>();
    assert_eq!(
        painted_rows.len(),
        expected_pill_rows,
        "painted pill rows in designated panel: panel={panel:?} targets={:?}",
        layout.selector_tabs
    );
    assert!(
        painted_rows.contains(&first.y),
        "target row is not a painted pill row: targets={:?} rows={painted_rows:?}",
        layout.selector_tabs
    );
    let row_text = (0..buffer.area().width)
        .map(|x| buffer[(x, first.y)].symbol())
        .collect::<String>();
    for label in expected_labels {
        assert!(
            row_text.contains(label),
            "pill row missing {label:?}: {row_text:?}"
        );
    }
    assert_eq!(
        buffer[(first.x, first.y)].style().bg,
        Some(palette::PILL_ROW_BG),
        "pill row background"
    );
    for pill_y in &painted_rows {
        assert!(
            *pill_y + 1 < panel.bottom(),
            "reserved spacer must fit in panel"
        );
        for x in panel.x..panel.right() {
            assert_eq!(
                buffer[(x, *pill_y + 1)].style().bg,
                Some(spacer_bg),
                "reserved spacer background at x={x}, y={}",
                *pill_y + 1
            );
        }
    }
    let painted_spans = (first.x..panel.right())
        .filter(|x| buffer[(*x, first.y)].symbol() == "◢")
        .filter_map(|start| {
            (start + 1..panel.right())
                .find(|x| buffer[(*x, first.y)].symbol() == "◤")
                .map(|end| Rect::new(start, first.y, end - start + 1, 1))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        painted_spans,
        layout
            .selector_tabs
            .iter()
            .map(|(rect, _)| *rect)
            .collect::<Vec<_>>(),
        "pill hitboxes must match painted horizontal spans"
    );
    for rect in layout.selector_tabs.iter().map(|(rect, _)| *rect) {
        assert!(
            panel.contains((rect.x, rect.y).into())
                && panel.contains((rect.right() - 1, rect.bottom() - 1).into()),
            "pill target outside designated panel: {rect:?} panel={panel:?}"
        );
    }
    let selected = layout
        .selector_tabs
        .iter()
        .find(|(_, id)| *id == selected_id)
        .expect("selected pill id should have a hitbox")
        .0;
    assert_eq!(
        buffer[(selected.x + 1, selected.y)].style().bg,
        Some(palette::PILL_SELECTED_BG),
        "selected pill appearance"
    );
}

pub fn render_library_to_terminal(app: &mut App, layout: &mut LayoutMain) -> Terminal<TestBackend> {
    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut model = crate::app::shell::Model::new(std::mem::replace(app, make_app_stub()));
    model.sync_mounted_surfaces();
    term.draw(|f| {
        model
            .app
            .render_library(f, Rect::new(0, 0, 60, 20), layout, None);
        model.render_emby_browser_component(f);
        model.render_music_workspace_component(f);
    })
    .unwrap();
    *app = model.app;
    term
}

pub fn render_library_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
    let term = render_library_to_terminal(app, layout);
    buffer_to_string(&term)
}

/// Like `render_library_to_string` but at an explicit terminal size, for
/// tests that need more rows than the default 60x20 (e.g. music-group views
/// whose hero panel reserves most of a short terminal).
pub fn render_library_to_string_sized(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut model = crate::app::shell::Model::new(std::mem::replace(app, make_app_stub()));
    model.sync_mounted_surfaces();
    term.draw(|f| {
        model
            .app
            .render_library(f, Rect::new(0, 0, width, height), layout, None);
        model.render_emby_browser_component(f);
        model.render_music_workspace_component(f);
    })
    .unwrap();
    *app = model.app;
    buffer_to_string(&term)
}

pub fn render_view_to_terminal(
    app: &mut App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    // Mirror the real shell path (task 3.1): the sync pass + `draw_frame`,
    // which composes the base frame and paints the mounted components —
    // including the queue panel, which now paints its own surface. Only
    // terminal_width is touched before the Model is built (the historical
    // helper contract); the shell path itself normalizes terminal size.
    app.terminal_width = width;
    let mut model = Model::new(std::mem::replace(app, make_app_stub()));
    model.sync_mounted_surfaces();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    let layout = model.app.layout.main.clone();
    *app = model.app;
    (term, layout)
}

/// Queue panel geometry the mounted `QueuePanel` retained after a real shell
/// draw (task 3.1): the framed list content area and the title band. The
/// `LayoutMain.queue_*` mirror is gone — the panel owns its geometry.
#[derive(Clone, Copy, Debug)]
pub struct QueuePanelView {
    pub content_area: Rect,
    pub title_area: Option<Rect>,
}

/// Read the queue panel's component-retained geometry from a model.
pub fn queue_panel_view(model: &Model) -> QueuePanelView {
    let queue = model
        .application
        .get_component(&crate::app::components::ComponentId::Queue)
        .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
        .expect("QueueComponent mounted");
    QueuePanelView {
        content_area: queue.content_area(),
        title_area: queue.test_title_area(),
    }
}

/// Render one frame through the real shell path (sync pass + `draw_frame`,
/// which composes the base frame and paints the mounted `QueuePanel`) and
/// return the terminal plus the panel's component-retained geometry. `app` is
/// restored afterwards, so tests can keep reading published `AppLayout`
/// fields.
pub fn render_queue_view_to_terminal(
    app: &mut App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, QueuePanelView) {
    // Only terminal_width is touched before the Model is built (the
    // historical helper contract): the queue panel's paint pass reads the
    // terminal sizes `compose_base_frame` normalized, so the placement
    // follows the drawn frame while the card's reservation geometry keeps
    // the stub's default height cap.
    app.terminal_width = width;
    let mut model = Model::new(std::mem::replace(app, make_app_stub()));
    model.sync_mounted_surfaces();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    let view = queue_panel_view(&model);
    *app = model.app;
    (term, view)
}

pub fn render_app_to_terminal(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.compose_base_frame(f, None)).unwrap();
    term
}

/// Render the Home destination exactly as the live shell does (task 5.11,
/// Home as the first panel owner): run the real sync pass (which mounts the
/// Library panel, points it at the Home owner, and projects the hero images),
/// then draw the base frame — which for Home only reserves the library area —
/// and paint the migrated `LibraryPanel` through the real
/// `Model::render_library_panel` shell path. Returns the model, so tests can
/// read the panel's own painted geometry and App state, together with the
/// terminal.
///
/// Home content is Model-owned (task 5.3d), so a test that needs seeded
/// Continue Watching rows/pills uses `render_home_shell_with` and seeds
/// `model.home_content` before the push.
pub fn render_queue_shell(
    mut app: App,
    width: u16,
    height: u16,
) -> (crate::app::shell::Model, Terminal<TestBackend>) {
    app.terminal_width = width;
    app.terminal_height = height;
    let mut model = crate::app::shell::Model::new(app);
    model.sync_queue();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        model.app.compose_base_frame(f, None);
        model.render_queue_component(f);
    })
    .unwrap();
    (model, term)
}

pub fn render_home_shell(
    app: App,
    width: u16,
    height: u16,
) -> (crate::app::shell::Model, Terminal<TestBackend>) {
    render_home_shell_with(app, width, height, |_| {})
}

/// `render_home_shell` with a content-seeding callback: the test seeds
/// Model-owned `home_content` (task 5.3d) right after `Model::new` and
/// before `push_home_content` projects it into the mounted `HomeComponent`.
pub fn render_home_shell_with(
    mut app: App,
    width: u16,
    height: u16,
    seed: impl FnOnce(&mut crate::app::shell::Model),
) -> (crate::app::shell::Model, Terminal<TestBackend>) {
    app.terminal_width = width;
    app.terminal_height = height;
    let mut model = crate::app::shell::Model::new(app);
    seed(&mut model);
    model.push_home_content();
    model.sync_mounted_surfaces();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        model.app.compose_base_frame(f, None);
        model.render_library_panel(f);
    })
    .unwrap();
    (model, term)
}

pub fn render_view(app: &mut App, width: u16, height: u16) -> LayoutMain {
    render_view_to_terminal(app, width, height).1
}

/// The Home content owner inside the mounted `LibraryPanel` (task 5.11),
/// for the panel-output test path: the characterization tests read the
/// owner's own cursor/section/scroll — the same painted-truth contract the
/// deleted mounted `HomeComponent` served.
pub(in crate::app) fn home_owner(
    model: &crate::app::shell::Model,
) -> Option<&crate::app::components::home_content::HomeContent> {
    use crate::app::components::library_panel::LibraryKey;
    model
        .application
        .get_component(&crate::app::components::ComponentId::Library)
        .and_then(|c| {
            c.as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.owner(&LibraryKey::Home))
        .and_then(|owner| {
            owner
                .as_any()
                .downcast_ref::<crate::app::components::home_content::HomeContent>()
        })
}

/// Assert, against a *painted* buffer, that a Wide hero list pane leaves
/// exactly one blank row between its framed bottom border and the status-bar
/// row that `wide_hero_presentation` reserves (migrate-home-feeds slice 3.2
/// §5.1). This is the per-family §5 geometry check: re-derived layout rects
/// cannot catch a one-row vertical shift, so it reads the glyphs instead.
///
/// `pane` is the list pane's rect; `status_row_y` is the row the status bar
/// occupies (one below the pane's bottom edge). The framed panel paints its
/// `▁` bottom border on `status_row_y - 2`, and `status_row_y - 1` must be
/// blank. A one-row shift up moves the border off `status_row_y - 2`; a shift
/// down paints the reserve row — either way an assertion here fails.
pub fn assert_list_pane_reserves_one_row_above_status(
    buffer: &ratatui::buffer::Buffer,
    pane: ratatui::layout::Rect,
    status_row_y: u16,
) {
    let border_y = status_row_y - 2;
    let reserve_y = status_row_y - 1;
    assert_eq!(
        buffer[(pane.x, border_y)].symbol(),
        "▁",
        "framed list panel must paint its bottom border on row {border_y}"
    );
    for x in pane.x..pane.right() {
        assert_eq!(
            buffer[(x, reserve_y)].symbol(),
            " ",
            "the reserve row {reserve_y} between the list panel and the status bar must be blank (x={x})"
        );
    }
}
