#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::components::library_panel::LibraryKey;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::tv_content::TvContent;
use crate::app::components::LibraryKind;
use crate::app::components::{ComponentId, QueueComponent};
use crate::app::layout::AppLayout;
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
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

#[path = "test_helpers_mounted.rs"]
mod mounted;
pub use mounted::*;
#[path = "test_helpers_fixtures.rs"]
mod fixtures;
pub use fixtures::*;

#[path = "test_helpers_music_tree.rs"]
mod music_tree;
pub use music_tree::*;

/// The active TV library's owner key, derived exactly as production does
/// (task 8.4: one `Service` key for a `tvshows` library; the owner lives
/// inside the mounted `LibraryPanel`).
fn tv_owner_key(model: &crate::app::shell::Model) -> LibraryKey {
    let index = model
        .app
        .tab
        .emby_library_index()
        .expect("Emby library tab");
    LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: model.app.libs[index].library.id.clone(),
        kind: LibraryKind::TvShows,
    }
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
    render_pill_bar_hitboxes_with_window(labels, ids, selected_pos, width, PillBarWindow::default())
        .0
}

/// The hitbox helper with marker flags and the bar's retained overflow window.
pub fn render_pill_bar_hitboxes_with_markers(
    labels: &[String],
    markers: &[bool],
    ids: &[usize],
    selected_pos: usize,
    width: u16,
) -> Vec<(Rect, usize)> {
    render_pill_bar_hitboxes_with_markers_and_window(
        labels,
        markers,
        ids,
        selected_pos,
        width,
        PillBarWindow::default(),
    )
    .0
}

/// The hitbox helper with the bar's retained overflow window: returns the
/// painted hitboxes plus the window to pass back on the next frame, so the
/// sticky-window tests can drive two-frame sequences.
pub fn render_pill_bar_hitboxes_with_window(
    labels: &[String],
    ids: &[usize],
    selected_pos: usize,
    width: u16,
    window: PillBarWindow,
) -> (Vec<(Rect, usize)>, PillBarWindow) {
    render_pill_bar_hitboxes_with_markers_and_window(labels, &[], ids, selected_pos, width, window)
}

fn render_pill_bar_hitboxes_with_markers_and_window(
    labels: &[String],
    markers: &[bool],
    ids: &[usize],
    selected_pos: usize,
    width: u16,
    window: PillBarWindow,
) -> (Vec<(Rect, usize)>, PillBarWindow) {
    let backend = TestBackend::new(width, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut tabs = Vec::new();
    let mut painted_window = window;
    term.draw(|f| {
        (tabs, painted_window) = render_pill_bar(
            f,
            Rect::new(0, 0, width, 1),
            PillBar {
                labels,
                markers,
                ids,
                selected_pos,
                hovered: None,
                prefix: None,
                window,
            },
        );
    })
    .unwrap();
    (tabs, painted_window)
}

pub fn render_library_to_terminal(app: &mut App, layout: &mut Rect) -> Terminal<TestBackend> {
    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut model = crate::app::shell::Model::new(std::mem::replace(app, make_app_stub()));
    model.sync_mounted_surfaces();
    term.draw(|f| {
        model
            .app
            .reserve_library_area(f, Rect::new(0, 0, 60, 20), layout, None);
        if let Some(area) = model.app.layout.root_frame.library {
            model.render_library_panel_at(f, area);
        }
    })
    .unwrap();
    *app = model.app;
    term
}

pub fn render_library_to_string(app: &mut App, layout: &mut Rect) -> String {
    let term = render_library_to_terminal(app, layout);
    buffer_to_string(&term)
}

/// Like `render_library_to_string` but at an explicit terminal size, for
/// tests that need more rows than the default 60x20 (e.g. music-group views
/// whose hero panel reserves most of a short terminal).
pub fn render_library_to_string_sized(
    app: &mut App,
    layout: &mut Rect,
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
            .reserve_library_area(f, Rect::new(0, 0, width, height), layout, None);
        if let Some(area) = model.app.layout.root_frame.library {
            model.render_library_panel_at(f, area);
        }
    })
    .unwrap();
    *app = model.app;
    buffer_to_string(&term)
}

pub fn render_view_to_terminal(
    app: &mut App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, Rect) {
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
    let layout = model.app.layout.left_area;
    *app = model.app;
    (term, layout)
}

/// Queue panel geometry the mounted `QueuePanel` retained after a real shell
/// draw (task 3.1): the framed list content area. The legacy queue geometry
/// mirror is gone — the panel owns its geometry.
#[derive(Clone, Copy, Debug)]
pub struct QueuePanelView {
    pub content_area: Rect,
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
    // terminal sizes normalized by root frame composition, so the placement
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
    term.draw(|f| app.compose_root_frame(f)).unwrap();
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
        model.app.compose_root_frame(f);
        if let Some(area) = model.app.layout.root_frame.queue {
            model.render_queue_panel_at(f, area);
        }
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
        model.app.compose_root_frame(f);
        if let Some(area) = model.app.layout.root_frame.library {
            model.render_library_panel_at(f, area);
        }
    })
    .unwrap();
    (model, term)
}

pub fn render_view(app: &mut App, width: u16, height: u16) -> Rect {
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
