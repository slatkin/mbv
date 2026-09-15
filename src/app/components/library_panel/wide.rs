//! The Wide Library panel skeleton (task 5.2, design D4): the Hero pane
//! (always the resting surface; only the focused list flips with focus —
//! the browser list rests while the Workspace media list holds it)
//! | gap | Browser pane (Selector row, List controls row, list
//! box). One skeleton for every Wide library destination: destinations supply
//! typed [`LibraryPanelContent`] and paint nothing themselves.
//!
//! The list box's presentation is the current carrier's (Wide/Inline via the
//! owner's `ensure_presentation`); the panel views it through the provisional
//! [`PanelList`] surface until task 5.8 formalizes the trait.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::arrangements::library::{wide_library_panes, WideLibraryPanes};
use crate::app::render::arrangements::padded_rect;
use crate::app::render::{
    render_inline_search, render_placeholder, wide_hero_browser_pane, wide_hero_hero_pane,
    PillBarWindow, PANE_PAD_X, PANE_PAD_Y,
};

use super::content::{HeroImageState, LibraryPanelContent, ListSlot, PanelHeroImagePaint};
use super::hero_header::paint_hero_pane_content;
use super::slots::{
    paint_list_controls_row, paint_pill_bar_row, paint_pill_row_gap, paint_selector_row,
    SELECTOR_ROW_PREFIX,
};
use crate::app::render::place_media_list_below;

/// Blank rows between the header/overview content's painted bottom edge and
/// the Workspace box (design D3: the Workspace sits below the overview).
const WORKSPACE_GAP_ROWS: u16 = 1;

/// A full-width claim rect over `content`'s rows, reaching `panel`'s left/
/// right edges: the canonical rail's selected-row background extends to the
/// panel border while row flow/hit geometry stays on the inset `content`.
fn full_width_claim(panel: Rect, content: Rect) -> Rect {
    Rect {
        x: panel.x,
        width: panel.width,
        ..content
    }
}

/// The Workspace selector's leading label (TV's season pills, the only
/// current Workspace selector): the pre-migration wide rail's own prefix
/// (`tv_wide.rs`), distinct from the Selector row's universal `⌘` glyph.
const WORKSPACE_SELECTOR_PREFIX: &str = " Series: ";

/// The skeleton's retained irregular-chrome hit registries, one per painted
/// pill row (ADR 0024: the mounted panel owns gesture state and resolves the
/// geometry it painted). The `LibraryPanel` component (task 5.9) owns these
/// registries; the skeleton pushes into them while painting.
#[derive(Debug, Default)]
pub(in crate::app) struct SkeletonHits {
    pub selector: HitRegions<usize>,
    pub controls: HitRegions<usize>,
    pub workspace_selector: HitRegions<usize>,
    pub links: HitRegions<usize>,
}

/// The pill rows' sticky overflow windows (ADR 0024 retained paint-local
/// geometry): the painted window's first pill index per pill row, kept by
/// the panel across frames so selecting an already-painted pill never
/// slides the bar. Unlike the per-frame [`SkeletonHits`], these survive
/// `reset_frame` — they are session state, revalidated by the painter
/// against each frame's pills.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::app) struct SkeletonPillWindows {
    pub selector: PillBarWindow,
    pub workspace_selector: PillBarWindow,
}

/// Everything the Wide skeleton placed this frame, in role-rect form. The
/// panel component (task 5.9) retains these rects; tests assert containment
/// against them, never absolute coordinates (design D15).
#[derive(Clone, Debug)]
pub(in crate::app) struct WideSkeletonGeometry {
    /// The Browser pane (list pane).
    pub browser: Rect,
    /// The Hero pane.
    pub hero: Rect,
    /// The Selector row's pill-bar rect, reserved even when the destination
    /// supplies no `SelectorRow` (the Inline Search box takes it).
    pub selector_bar: Rect,
    /// The List controls row's rect, when the destination supplies content.
    pub controls: Option<Rect>,
    /// The list box's full panel rect (fill + border).
    pub list_panel: Rect,
    /// The list box's inset row-flow rect.
    pub list_area: Rect,
    /// The Hero pane's inset content rect.
    pub hero_area: Rect,
    /// The Workspace box's panel and the rect its list was viewed into
    /// (below the optional header rows), when one painted.
    pub workspace: Option<(Rect, Rect)>,
    /// The projected hero image's paint (task 5.10, design D9), when the
    /// header reserved a ready image's box.
    pub hero_image: Option<PanelHeroImagePaint>,
    pub overview_box: Option<Rect>,
    pub overview_content_length: usize,
    pub overview_viewport: usize,
    /// The selected row's rect from the list slot's view, when one painted
    /// (the context-menu anchor's painted truth).
    pub selected: Option<Rect>,
}

/// Paints the Wide Library panel skeleton into `area`. Returns the frame's
/// role-rect geometry, or `None` when `area` does not fit the shared Wide
/// presentation (the Narrow skeleton, task 5.7, owns that breakpoint).
///
/// Rows top-to-bottom in the Browser pane: the Selector row (one pill bar +
/// the panel's spacer, reserved even without a `SelectorRow`), the optional
/// List controls row, and the list box (fill, then the list presentation or
/// the `ListSlot::Empty` placeholder). While
/// `ListSlot::Search` is active, the search box paints in the Selector row's
/// rect and the results in the list box, and the rest of the panel is
/// unchanged.
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn render_wide_skeleton(
    f: &mut Frame,
    area: Rect,
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
    override_width: Option<u16>,
    overview_scroll: usize,
    hovered_selector: Option<usize>,
    hovered_link: Option<usize>,
    hits: &mut SkeletonHits,
    windows: &mut SkeletonPillWindows,
) -> Option<WideSkeletonGeometry> {
    let WideLibraryPanes {
        hero_panel,
        browser_panel,
        browser_area,
        ..
    } = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y, override_width)?;
    let pane = wide_hero_browser_pane(browser_panel, browser_area);

    // Selector row: one pill bar + the panel's spacer, reserved even without
    // a SelectorRow — while a search is active the box takes the bar's rect
    // (spec: the Selector row's place is reserved for the search box) and
    // the spacer keeps the reserved gap.
    let searching = matches!(content.list, ListSlot::Search(_));
    match (&content.selector, searching) {
        (Some(selector), false) => {
            paint_selector_row(
                f,
                pane.pills_area,
                pane.spacer_area,
                selector,
                hovered_selector,
                &mut hits.selector,
                &mut windows.selector,
            );
        }
        (None, false) => {
            // No `SelectorRow`: still repaint the reserved pills row's own
            // background (task 12.2) through the shared pill-row painter,
            // the same one `paint_selector_row` calls for an empty pill
            // list, so the row never keeps whatever was painted underneath
            // it before this panel owned the placement.
            paint_pill_bar_row(
                f,
                pane.pills_area,
                &[],
                None,
                None,
                Some(SELECTOR_ROW_PREFIX),
                &mut hits.selector,
                &mut windows.selector,
            );
            paint_pill_row_gap(f, pane.spacer_area);
        }
        (_, true) => paint_pill_row_gap(f, pane.spacer_area),
    }

    // List controls row: reserved only when the destination supplies
    // content; without it the rows below move up (spec scenario).
    let (list_panel, controls_area) = match &content.controls {
        Some(controls) if pane.list_panel.height > 0 => {
            let row = Rect {
                height: 1,
                ..pane.list_panel
            };
            paint_list_controls_row(f, row, controls, &mut hits.controls);
            (
                Rect {
                    y: row.bottom(),
                    height: pane.list_panel.height.saturating_sub(1),
                    ..pane.list_panel
                },
                Some(row),
            )
        }
        _ => (pane.list_panel, None),
    };

    // List box focus: the panel's bit, minus the Workspace. When the hero's
    // media list holds focus the browser list drops its green and rests —
    // green marks the focused list, never both at once.
    let list_focused = browser_focused && !content.workspace_focused();

    // List box: fill, then the slot's content in the inset row-flow rect.
    crate::app::render::components::widgets::fill_surface(
        f,
        list_panel,
        palette::Surface::LibraryPanel,
        list_focused,
    );
    // The rail's row flow keeps the side pads and both vertical pads,
    // matching the Workspace box's own `padded_rect` inset: one blank spacer
    // row between the rail's last row and the panel's bottom edge.
    let list_area = Rect {
        x: list_panel.x.saturating_add(PANE_PAD_X),
        y: list_panel.y.saturating_add(PANE_PAD_Y),
        width: list_panel.width.saturating_sub(PANE_PAD_X * 2),
        height: list_panel.height.saturating_sub(PANE_PAD_Y * 2),
    };
    match &mut content.list {
        ListSlot::Search(search) => {
            let items = search.ordered_items();
            let query = search.query().to_string();
            let loading = search.loading();
            let cursor = search.cursor();
            let scroll_in = search.scroll();
            let new_scroll = render_inline_search(
                f,
                pane.pills_area,
                list_area,
                &query,
                loading,
                items,
                cursor,
                scroll_in,
                list_focused,
                1,
                search.layout_mut(),
            );
            search.set_scroll(new_scroll);
        }
        ListSlot::Media(list) => {
            // The panel drives the presentation and the paint policy (design
            // D3): the Wide breakpoint selects the Wide presentation, and the
            // slot fixes focus and the list-backdrop selected row (design D6).
            list.set_presentation(
                crate::app::components::media_list::Presentation::Wide,
                list_area.height.max(1) as usize,
            );
            list.set_paint_policy(super::content::PanelListPaintPolicy::Wide {
                focused: list_focused,
            });
            // The canonical rail owns the full panel row (matching the
            // pre-migration wide rail): the selected background reaches
            // `list_panel`'s border while the row flow/hit geometry stays on
            // the inset `list_area`, so `media_list_row`'s own 2-column text
            // indent is the row's only indent instead of stacking atop
            // `list_area`'s inset.
            list.set_geometry(full_width_claim(list_panel, list_area), list_area);
            list.view(f, list_area);
        }
        ListSlot::Empty { loading, text } => {
            let msg = if *loading {
                " Loading\u{2026}"
            } else {
                text.as_str()
            };
            if !msg.is_empty() {
                render_placeholder(f, list_area, msg);
            }
        }
    }

    // Hero pane: always the resting fill. It never takes the focused
    // surface when the media list (or its Workspace) holds focus — only
    // the list box above flips with the panel's focus. A focused Workspace
    // still shows through its own content-box tint.
    let hero_area = wide_hero_hero_pane(f, area, false, override_width)?;

    // The list slot's painted selection: the context-menu anchor's painted
    // truth (the selected row's rect, or the admitted inline hero block).
    let selected = match &mut content.list {
        ListSlot::Media(list) => list.selected_row_rect(),
        _ => None,
    };

    let mut geometry = WideSkeletonGeometry {
        browser: browser_panel,
        hero: hero_panel,
        selector_bar: pane.pills_area,
        controls: controls_area,
        list_panel,
        list_area,
        hero_area,
        workspace: None,
        hero_image: None,
        overview_box: None,
        overview_content_length: 0,
        overview_viewport: 0,
        selected,
    };

    if let Some(hero) = content.hero.as_mut() {
        // The Hero header (task 5.5): policy arm, placeholder artwork box,
        // one title/meta painter, and the overview box when overview text
        // exists. Returns the first unpainted row and the projected image's
        // reserved box (task 5.10: `Ready` reserves; the shell paints).
        let (next_row, image_box, overview) = paint_hero_pane_content(
            f,
            hero_area,
            &*hero,
            overview_scroll,
            hovered_link,
            &mut hits.links,
        );
        if let Some(overview) = overview {
            geometry.overview_box = Some(overview.rect);
            geometry.overview_content_length = overview.content_length;
            geometry.overview_viewport = overview.viewport;
        }
        if let (HeroImageState::Ready { cache_key, .. }, Some(box_rect)) =
            (&hero.facts.artwork.image, image_box)
        {
            geometry.hero_image = Some(PanelHeroImagePaint {
                area: box_rect,
                cache_key: cache_key.clone(),
                centered: true,
            });
        }
        if let Some(workspace) = hero.workspace.as_mut() {
            // One blank row below the painted content before the Workspace
            // box (task 5.6): `next_row` is the first *unpainted* row, and
            // `place_media_list_below` adds `WORKSPACE_GAP_ROWS` blank rows
            // above the box, so passing `next_row` leaves that row blank.
            if let Some(workspace_rect) =
                place_media_list_below(hero_area, next_row, WORKSPACE_GAP_ROWS, hero_area.height)
            {
                geometry.workspace = Some(paint_workspace_box(
                    f,
                    workspace_rect,
                    workspace,
                    &mut hits.workspace_selector,
                    &mut windows.workspace_selector,
                ));
            }
        }
    }
    Some(geometry)
}

/// Rows a Workspace header occupies: the title, the Hero separator line,
/// and the blank row below it.
const WORKSPACE_HEADER_ROWS: u16 = 3;

/// Paints a Workspace box's header: the title in bold foam, then the
/// separator line the Movie hero's overview box uses under its overview text
/// (same `▁` block characters and role) spanning the box's content width.
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

/// The Workspace (task 5.6, design D6): an optional header band and Selector
/// row over one Main content box holding the Workspace's `&mut dyn PanelList`.
/// The box's surface is derived, not declared — accent-soft while the
/// workspace list holds focus, backdrop otherwise (user decision: Music's
/// behaviour for all) — and the list's selected row is fixed to the owning
/// surface by the slot. Returns the (panel, content) rects, where `content`
/// is the rect the list was viewed into below the header band.
fn paint_workspace_box(
    f: &mut Frame,
    workspace_rect: Rect,
    workspace: &mut super::content::Workspace<'_>,
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
            // `render_pill_bar` fully repaints the row's background even
            // with no pills (task 12.2): the row stays reserved, so it must
            // still own its own paint. TV's season pills are this selector
            // (`tv_content/mod.rs`); the legacy wide rail's own prefix
            // (`tv_wide.rs`, pre-migration) is restored here rather than the
            // Selector row's universal `⌘` glyph, which this row never used.
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
    // The box fills `box_area` flush (matching the list panel and hero pane's
    // own single inset): `workspace_rect` is already inset from the hero
    // panel's edge by `hero_area`'s padding, so the panel must not add a
    // second outer margin on top of it. Only `content` carries the box's own
    // interior padding, matching every other recessed box.
    let panel = box_area;
    let background =
        palette::surface_colors(palette::Surface::MainContentBox, workspace.focused).fill;
    f.render_widget(
        Block::default().style(Style::default().bg(background)),
        panel,
    );
    let content = padded_rect(panel, PANE_PAD_X, PANE_PAD_Y);
    // The destination's box header (Grouped Music's `Tracks`): title, the
    // Hero separator line, one blank row, then the list. Skipped when the
    // box cannot keep a list row under it -- the same "omitted when no room"
    // convention as the other slot arrangements.
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
    // The owning-surface selected row is fixed by the slot (design D6): the
    // panel sets the paint policy, destinations pass none.
    workspace
        .list
        .set_paint_policy(super::content::PanelListPaintPolicy::WideWorkspace {
            focused: workspace.focused,
        });
    // Full-width claim so the selected row's background reaches the box's
    // own border, matching the Browser pane's list (see above).
    workspace
        .list
        .set_geometry(full_width_claim(panel, content), content);
    workspace.list.view(f, content);
    (panel, content)
}

#[cfg(test)]
#[path = "wide_tests.rs"]
mod wide_tests;
