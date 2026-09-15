//! Shared Library-panel Hero content composition.
//!
//! This path owns the Hero header, overview box, optional Workspace selector,
//! Workspace box, and the paint-local geometry returned to the panel. Wide and
//! the Library Hero overlay use the same supplied-rectangle contract.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::content::{
    HeroContent, HeroImageState, PanelHeroImagePaint, PanelListPaintPolicy, Workspace,
};
use super::hero_header::paint_hero_pane_content;
use super::slots::paint_pill_bar_row;
use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::{place_media_list_below, PillBarWindow, PANE_PAD_X, PANE_PAD_Y};

const WORKSPACE_GAP_ROWS: u16 = 1;
const WORKSPACE_HEADER_ROWS: u16 = 3;

/// Geometry produced by painting shared Hero content.
#[derive(Clone, Debug)]
pub(in crate::app) struct HeroCompositionGeometry {
    pub workspace: Option<(Rect, Rect)>,
    pub hero_image: Option<PanelHeroImagePaint>,
    pub overview_box: Option<Rect>,
    pub overview_content_length: usize,
    pub overview_viewport: usize,
}

/// Paint the shared Hero content into `area`, returning image and Workspace
/// geometry for the owning Library panel.
pub(in crate::app) fn paint_library_hero_content(
    f: &mut Frame,
    area: Rect,
    hero: &mut HeroContent<'_>,
    overview_scroll: usize,
    hovered_link: Option<usize>,
    link_hits: &mut HitRegions<usize>,
    workspace_selector_hits: &mut HitRegions<usize>,
    workspace_selector_window: &mut PillBarWindow,
) -> HeroCompositionGeometry {
    let (next_row, image_box, overview) =
        paint_hero_pane_content(f, area, &*hero, overview_scroll, hovered_link, link_hits);
    let mut geometry = HeroCompositionGeometry {
        workspace: None,
        hero_image: None,
        overview_box: overview.as_ref().map(|value| value.rect),
        overview_content_length: overview.as_ref().map_or(0, |value| value.content_length),
        overview_viewport: overview.as_ref().map_or(0, |value| value.viewport),
    };
    if let (HeroImageState::Ready { cache_key, .. }, Some(area)) =
        (&hero.facts.artwork.image, image_box)
    {
        geometry.hero_image = Some(PanelHeroImagePaint {
            area,
            cache_key: cache_key.clone(),
            centered: true,
        });
    }
    if let Some(workspace) = hero.workspace.as_mut() {
        if let Some(workspace_rect) =
            place_media_list_below(area, next_row, WORKSPACE_GAP_ROWS, area.height)
        {
            geometry.workspace = Some(paint_workspace_box(
                f,
                workspace_rect,
                workspace,
                workspace_selector_hits,
                workspace_selector_window,
            ));
        }
    }
    geometry
}

fn paint_workspace_header(f: &mut Frame, content: Rect, header: &str) {
    f.render_widget(
        Paragraph::new(header).style(
            Style::default()
                .fg(palette::TEXT_METADATA)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Rect {
            height: 1,
            ..content
        },
    );
    crate::app::render::components::widgets::render_block_separator(
        f,
        Rect {
            y: content.y.saturating_add(1),
            height: 1,
            ..content
        },
    );
}

fn full_width_claim(panel: Rect, content: Rect) -> Rect {
    Rect {
        x: panel.x,
        width: panel.width,
        ..content
    }
}

fn paint_workspace_box(
    f: &mut Frame,
    workspace_rect: Rect,
    workspace: &mut Workspace<'_>,
    hits: &mut HitRegions<usize>,
    window: &mut PillBarWindow,
) -> (Rect, Rect) {
    let mut box_area = workspace_rect;
    if let Some(selector) = &workspace.selector {
        let bar = Rect {
            height: 1,
            ..workspace_rect
        };
        if bar.height > 0 {
            paint_pill_bar_row(
                f,
                bar,
                &selector.pills,
                selector.active,
                None,
                Some(" Series: "),
                hits,
                window,
            );
        }
        box_area = Rect {
            y: bar.bottom(),
            height: workspace_rect.height.saturating_sub(1),
            ..workspace_rect
        };
    }
    let panel = box_area;
    let background =
        palette::surface_colors(palette::Surface::MainContentBox, workspace.focused).fill;
    f.render_widget(
        Block::default().style(Style::default().bg(background)),
        panel,
    );
    let content = padded_rect(panel, PANE_PAD_X, PANE_PAD_Y);
    let content = match workspace.header {
        Some(header) if content.height > WORKSPACE_HEADER_ROWS => {
            paint_workspace_header(f, content, header);
            Rect {
                y: content.y.saturating_add(WORKSPACE_HEADER_ROWS),
                height: content.height.saturating_sub(WORKSPACE_HEADER_ROWS),
                ..content
            }
        }
        _ => content,
    };
    workspace
        .list
        .set_paint_policy(PanelListPaintPolicy::WideWorkspace {
            focused: workspace.focused,
        });
    workspace
        .list
        .set_geometry(full_width_claim(panel, content), content);
    workspace.list.view(f, content);
    (panel, content)
}
