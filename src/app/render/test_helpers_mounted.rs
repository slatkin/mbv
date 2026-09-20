#![allow(dead_code, unused_imports)]

use super::super::*;
use super::buffer_to_string;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::media_list::MediaSemanticState;
use crate::app::components::music_tree::{MusicTreeBrowser, MusicTreeEntry, MusicTreeModel};
use crate::app::components::tv_content::TvContent;
use crate::app::components::ComponentId;
use crate::app::layout::PaintedRowGeometry;
use crate::app::music_grouping::ArtistKey;
use crate::app::shell::Model;
use crate::app::{App, PanelFocus};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Build a `Model` at an explicit terminal size with the library pane focused.
/// Characterization tests whose surface is now painted by an embedded owner
/// (`EmbyLibraryContent` / `MusicWorkspaceComponent` / embedded `TvContent`)
/// instead of the legacy library dispatch start here, then draw with
/// `draw_mounted_frame` and read geometry via `mounted_*_layout`.
pub fn mounted_model_at(mut app: App, width: u16, height: u16) -> Model {
    app.terminal_width = width;
    app.terminal_height = height;
    app.mini_view_focus = PanelFocus::Library;
    Model::new(app)
}

/// Draw one full frame through `Model::draw_frame` (the live shell paint path)
/// after re-syncing mounted surfaces, and return the painted buffer text.
/// One throwaway draw runs first: the chrome panels mount only once a frame
/// has published `root_frame` (tasks 2.1-2.2), so this mirrors the steady
/// state — startup draw, then sync, then the loop draw.
pub fn draw_mounted_frame(model: &mut Model, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    // One throwaway draw publishes `root_frame`; the panels mount at the
    // sync it gates, then the recorded draw paints them.
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
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
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    // One throwaway draw publishes `root_frame` (tasks 2.1-2.2); the chrome
    // panels mount at the sync it gates, then the recorded draw paints them.
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    model.sync_mounted_surfaces();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    term
}

/// The mounted `MusicWorkspaceComponent`'s own painted geometry.
pub fn mounted_music_wide_geometry(
    model: &Model,
) -> crate::app::components::library_panel::WideSkeletonGeometry {
    model
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel")
        .test_wide_geometry()
        .expect("wide Music skeleton painted")
}

/// The Grouped Music tree over the mounted fixture's settled projection: the
/// same `MusicWideRenderCtx` facts (`album_info`/`album_order`/targets) the
/// production browser flattens, grouped into artist roots and album leaves.
pub fn mounted_music_tree_browser(model: &Model) -> MusicTreeBrowser {
    let ctx = model.app.wide_music_render_ctx(0, None);
    let entries: Vec<MusicTreeEntry> = ctx
        .album_order
        .iter()
        .map(|&index| {
            let (artist, year, name) = &ctx.album_info[index];
            MusicTreeEntry {
                artist: artist.clone(),
                // The fixture's settled order groups each artist's albums
                // consecutively, so the deterministic fallback key reproduces
                // the same roots as the settled `ArtistKey` identity does.
                artist_key: ArtistKey::Fallback(artist.clone()),
                title: name.clone(),
                year: (!year.is_empty()).then(|| year.clone()),
                target: ctx.album_targets[index].clone(),
                // The tree projects the fixture's settled album facts; the
                // real pane derives this through `MediaSemanticState::from_emby`
                // (music collapse keeps it ordinary).
                semantic_state: MediaSemanticState::Ordinary,
            }
        })
        .collect();
    MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries))
}

/// The panel-hosted TV owner (task 8.4: reached through the mounted
/// `LibraryPanel`'s `LibraryKey` map, never a `ComponentId`).
pub fn tv_owner(model: &Model) -> &TvContent {
    let key = super::test_helpers::tv_owner_key(model);
    model
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel")
        .owner(&key)
        .expect("tv owner installed")
        .as_any()
        .downcast_ref::<TvContent>()
        .expect("TvContent")
}

/// The TV owner's painted geometry, surfaced as `PaintedRowGeometry` so the shared
/// role-rect assertions keep working: the mounted `LibraryPanel` owns the
/// rects and publishes them through its own retained-geometry accessor.
pub fn mounted_tv_layout(model: &Model) -> PaintedRowGeometry {
    model
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel")
        .test_painted_layout()
}

/// The TV owner's series scroll offset this frame.
pub fn mounted_tv_scroll(model: &Model) -> usize {
    tv_owner(model).scroll()
}
