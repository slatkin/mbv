use super::*;
use crate::app::components::QueueComponent;
use crate::app::shell::Model;
use crate::app::tests::make_app_stub;
use crate::app::App;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

mod mounted;
pub use mounted::*;
mod fixtures;
pub use fixtures::*;

mod music_tree;
pub use music_tree::*;

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
