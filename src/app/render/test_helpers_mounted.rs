#![allow(dead_code, unused_imports)]

use super::super::*;
use super::buffer_to_string;
use crate::app::components::{BrowserComponent, MusicWorkspaceComponent, TvWorkspaceComponent};
use crate::app::shell::Model;
use crate::app::{App, PanelFocus};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Build a `Model` at an explicit terminal size with the library pane focused.
/// Characterization tests whose surface is now painted by a mounted component
/// (`BrowserComponent` / `MusicWorkspaceComponent` / `TvWorkspaceComponent`)
/// instead of the legacy `render_library` arm start here, then draw with
/// `draw_mounted_frame` and read geometry via `mounted_*_layout`.
pub fn mounted_model_at(mut app: App, width: u16, height: u16) -> Model {
    app.terminal_width = width;
    app.terminal_height = height;
    app.mini_view_focus = PanelFocus::Library;
    Model::new(app)
}

/// Draw one full frame through `Model::draw_frame` (the live shell paint path)
/// after re-syncing mounted surfaces, and return the painted buffer text.
pub fn draw_mounted_frame(model: &mut Model, width: u16, height: u16) -> String {
    model.sync_mounted_surfaces();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    buffer_to_string(&term)
}

/// Like `draw_mounted_frame` but hands back the terminal so a test can read
/// the painted buffer. `draw_frame` is the live shell paint path, so the
/// bottom status-bar row is painted (unlike a bare component `view`).
pub fn draw_mounted_terminal(model: &mut Model, width: u16, height: u16) -> Terminal<TestBackend> {
    model.sync_mounted_surfaces();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    term
}

/// The mounted Emby `BrowserComponent`'s own painted geometry (task 3.8: the
/// legacy `render_library` `EmbyLibrary` arm no longer publishes it).
pub fn mounted_browser_layout(model: &Model) -> &LayoutMain {
    let id = model
        .emby_browser_id
        .as_ref()
        .expect("emby browser component mounted");
    model
        .application
        .get_component(id)
        .expect("emby browser mounted")
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .expect("BrowserComponent")
        .test_layout()
}

/// The scroll offset the mounted Emby `BrowserComponent` settled on this frame
/// (task 3.8: the browser owns the persisted flow offset the legacy renderer
/// used to write back into the `BrowseLevel`).
pub fn mounted_browser_scroll(model: &Model) -> usize {
    let id = model
        .emby_browser_id
        .as_ref()
        .expect("emby browser component mounted");
    model
        .application
        .get_component(id)
        .expect("emby browser mounted")
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .expect("BrowserComponent")
        .scroll()
}

/// The mounted `MusicWorkspaceComponent`'s own painted geometry.
pub fn mounted_music_layout(model: &Model) -> &LayoutMain {
    let id = model
        .music_workspace_id
        .as_ref()
        .expect("music workspace component mounted");
    model
        .application
        .get_component(id)
        .expect("music workspace mounted")
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .expect("MusicWorkspaceComponent")
        .layout()
}

/// The album-scroll offset the mounted `MusicWorkspaceComponent` settled on.
pub fn mounted_music_scroll(model: &Model) -> usize {
    let id = model
        .music_workspace_id
        .as_ref()
        .expect("music workspace component mounted");
    model
        .application
        .get_component(id)
        .expect("music workspace mounted")
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .expect("MusicWorkspaceComponent")
        .album_scroll()
}

/// The mounted `TvWorkspaceComponent`'s own painted geometry.
pub fn mounted_tv_layout(model: &Model) -> &LayoutMain {
    let id = model
        .tv_workspace_id
        .as_ref()
        .expect("tv workspace component mounted");
    model
        .application
        .get_component(id)
        .expect("tv workspace mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TvWorkspaceComponent")
        .test_layout()
}
