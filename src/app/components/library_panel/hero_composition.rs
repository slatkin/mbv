//! Shared Library-panel Hero content composition.
//!
//! This path owns the Hero header, overview box, optional Workspace selector,
//! Workspace box, and the paint-local geometry returned to the panel. Both
//! the Wide Hero pane and the Library Hero overlay call it under the same
//! supplied-rectangle contract.
//!
//! Bespoke placement: a Panel owns its slots' placement, fills, and painting
//! (mbv-frontend, "Panel and slot composition"), so this painter lives beside
//! the Library panel's other skeleton painters (`wide.rs`, `narrow.rs`,
//! `hero_header.rs`) instead of in `src/app/render/components/`. It resolves
//! theme roles only and owns no interaction state.

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

/// Blank rows between the header/overview content's painted bottom edge and
/// the Workspace box (design D3: the Workspace sits below the overview).
const WORKSPACE_GAP_ROWS: u16 = 1;

/// Rows a Workspace header occupies: the title, the Hero separator line, and
/// the blank row below it.
const WORKSPACE_HEADER_ROWS: u16 = 3;

/// The Workspace selector's leading label: TV season pills retain the
/// pre-migration `tv_wide.rs` prefix, distinct from the Selector row's `⌘`
/// glyph (which identifies the general Selector row).
const WORKSPACE_SELECTOR_PREFIX: &str = " Series: ";

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
/// geometry for the owning Library panel. `surface` is the surface the caller
/// painted `hero_area` with (the Wide Hero pane's fill, or the Library Hero
/// overlay's sheet): shared content repaints it wherever it needs the pane's
/// own background.
pub(in crate::app) fn paint_library_hero_content(
    f: &mut Frame,
    hero_area: Rect,
    hero: &mut HeroContent<'_>,
    overview_scroll: usize,
    hovered_link: Option<usize>,
    link_hits: &mut HitRegions<usize>,
    workspace_selector_hits: &mut HitRegions<usize>,
    workspace_selector_window: &mut PillBarWindow,
    surface: palette::Surface,
    terminal_height: u16,
) -> HeroCompositionGeometry {
    let (next_row, image_box, overview) = paint_hero_pane_content(
        f,
        hero_area,
        &*hero,
        overview_scroll,
        hovered_link,
        link_hits,
        surface,
        terminal_height,
    );
    let mut geometry = HeroCompositionGeometry {
        workspace: None,
        hero_image: None,
        overview_box: overview.as_ref().map(|value| value.rect),
        overview_content_length: overview.as_ref().map_or(0, |value| value.content_length),
        overview_viewport: overview.as_ref().map_or(0, |value| value.viewport),
    };
    if let (HeroImageState::Ready { cache_key, .. }, Some(image_area)) =
        (&hero.facts.artwork.image, image_box)
    {
        geometry.hero_image = Some(PanelHeroImagePaint {
            area: image_area,
            cache_key: cache_key.clone(),
            centered: true,
        });
    }
    if let Some(workspace) = hero.workspace.as_mut() {
        // `next_row` is the first unpainted row. The shared placement helper
        // reserves WORKSPACE_GAP_ROWS above the box, leaving the required
        // blank row between Hero content and Workspace (design D3).
        if let Some(workspace_rect) =
            place_media_list_below(hero_area, next_row, WORKSPACE_GAP_ROWS, hero_area.height)
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
    let style = if header == "TRACKLIST" {
        Style::default().fg(palette::MUSIC_HEADER)
    } else {
        Style::default()
            .fg(palette::TEXT_METADATA)
            .add_modifier(ratatui::style::Modifier::BOLD)
    };
    f.render_widget(
        Paragraph::new(header).style(style),
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

/// Claim the full panel width for selected-row backgrounds while preserving
/// the inset content rows used for list flow and hit geometry.
pub(in crate::app) fn full_width_claim(panel: Rect, content: Rect) -> Rect {
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
    // The box's own body fill and the list's cursor emphasis both resolve the
    // Workspace's own focus: the box is the focused Workspace's surface, and a
    // focused list whose cursor painted unfocused is a list no key can reach.
    let box_focused = workspace.focused;
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
                Some(WORKSPACE_SELECTOR_PREFIX),
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
    // The Workspace box rests at the hero boxes' own Slate in both focus
    // states and both geometries — one unified look, no focused soft fill.
    let background = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
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
    // The list's cursor emphasis, marquee, and scrollbar follow the same bit.
    workspace
        .list
        .set_paint_policy(PanelListPaintPolicy::WideWorkspace {
            focused: box_focused,
        });
    workspace
        .list
        .set_geometry(full_width_claim(panel, content), content);
    workspace.list.view(f, content);
    (panel, content)
}
