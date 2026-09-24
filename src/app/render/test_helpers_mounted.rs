#![allow(dead_code, unused_imports)]

use super::super::*;
use super::buffer_to_string;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::list::tree_browser::{
    TreeBrowser, TreeMarkPolicy, TreeNode, TreeTitleRole,
};
use crate::app::components::media_list::MediaSemanticState;
use crate::app::components::music_tree_target::MusicTreeTarget;
use crate::app::components::tv_content::TvContent;
use crate::app::components::ComponentId;
use crate::app::layout::PaintedRowGeometry;
use crate::app::shell::Model;
use crate::app::state::music_grouping::ArtistKey;
use crate::app::{App, PanelFocus};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::HashMap;

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
/// production projection flattens, grouped into artist roots and album leaves
/// and reconciled into one shared `TreeBrowser`.
pub fn mounted_music_tree_browser(model: &Model) -> TreeBrowser<MusicTreeTarget> {
    let ctx = model.app.wide_music_render_ctx(0, None);
    let mut browser = TreeBrowser::new();
    browser
        .reconcile(music_tree_fixture_projection(&ctx))
        .expect("the fixture's settled albums form a forest");
    browser
}

/// The fixture's plain node projection (test data, not a second production
/// destination): first-occurrence artist roots over their settled album
/// leaves, in settled order.
fn music_tree_fixture_projection(
    ctx: &crate::app::render::MusicWideRenderCtx,
) -> Vec<TreeNode<MusicTreeTarget>> {
    let mut nodes: Vec<TreeNode<MusicTreeTarget>> = Vec::new();
    let mut root_of_key: HashMap<ArtistKey, MusicTreeTarget> = HashMap::new();
    for &index in &ctx.album_order {
        let Some((artist, year, name)) = ctx.album_info.get(index) else {
            continue;
        };
        let Some(key) = ctx.album_artist_keys.get(index) else {
            continue;
        };
        let Some(album_target) = ctx.album_targets.get(index) else {
            continue;
        };
        let artist_target = if let Some(existing) = root_of_key.get(key) {
            existing.clone()
        } else {
            let target = MusicTreeTarget::Artist(key.clone());
            root_of_key.insert(key.clone(), target.clone());
            nodes.push(
                TreeNode::new(
                    target.clone(),
                    None,
                    artist.clone(),
                    artist.clone(),
                    MediaSemanticState::Ordinary,
                    TreeMarkPolicy::Aggregate,
                )
                .with_title_role(TreeTitleRole::Heading),
            );
            target
        };
        let album_node = TreeNode::new(
            MusicTreeTarget::Album(album_target.clone()),
            Some(artist_target),
            name.clone(),
            format!("{name} {year}"),
            // The fixture's settled album facts are ordinary; the real pane
            // derives playback emphasis through `MediaSemanticState`.
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Direct,
        )
        .with_title_role(TreeTitleRole::Secondary);
        nodes.push(if year.is_empty() {
            album_node
        } else {
            album_node.with_trailing(year.clone())
        });
    }
    nodes
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
