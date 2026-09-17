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

use ratatui::Frame;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::arrangements::library::{wide_library_panes, WideLibraryPanes};
use crate::app::render::arrangements::wide_hero::WideHeroBrowserPane;
use crate::app::render::{
    render_placeholder, render_search_box, wide_hero_browser_pane, wide_hero_hero_pane,
    PillBarWindow, PANE_PAD_X, PANE_PAD_Y,
};

use super::content::{LibraryPanelContent, ListSlot, PanelList, PanelListPaintPolicy};
use super::hero_composition::{full_width_claim, paint_library_hero_content};
use super::slots::{
    paint_list_controls_row, paint_pill_bar_row, paint_pill_row_gap, paint_selector_row,
    SELECTOR_ROW_PREFIX,
};

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
/// against them, never absolute coordinates (design D15). `Default` is the
/// non-Wide panel's geometry: it supplies the Browser pane's rects and leaves
/// every Hero/Workspace/overview field unset (design D1).
#[derive(Clone, Debug, Default)]
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
    pub hero_image: Option<super::content::PanelHeroImagePaint>,
    pub overview_box: Option<Rect>,
    pub overview_content_length: usize,
    pub overview_viewport: usize,
    /// The selected row's rect from the list slot's view, when one painted
    /// (the context-menu anchor's painted truth).
    pub selected: Option<Rect>,
}

/// The Browser pane's role rects for this frame, in [`WideSkeletonGeometry`]
/// field form (the pane painter's contribution to the skeleton geometry).
/// Both skeletons consume it: the non-Wide panel delegates to
/// [`paint_browser_pane`] and folds the rects into its own geometry.
pub(in crate::app) struct BrowserPaneGeometry {
    /// The Selector row's pill-bar rect, reserved even when the destination
    /// supplies no `SelectorRow` (the Inline Search box takes it).
    pub(in crate::app) selector_bar: Rect,
    /// The List controls row's rect, when the destination supplies content.
    pub(in crate::app) controls: Option<Rect>,
    /// The list box's full panel rect (fill + border).
    pub(in crate::app) list_panel: Rect,
    /// The list box's inset row-flow rect.
    pub(in crate::app) list_area: Rect,
    /// The selected row's rect from the list slot's view, when one painted
    /// (the context-menu anchor's painted truth).
    pub(in crate::app) selected: Option<Rect>,
}

/// Paints the Browser pane (Selector row, optional List controls row, list
/// box) and returns the role rects it placed. Pure painting over the supplied
/// pane geometry; the hero pane and workspace content are painted elsewhere.
pub(in crate::app) fn paint_browser_pane(
    f: &mut Frame,
    pane: WideHeroBrowserPane,
    content: &mut LibraryPanelContent<'_>,
    list_focused: bool,
    hovered_selector: Option<usize>,
    hits: &mut SkeletonHits,
    windows: &mut SkeletonPillWindows,
) -> BrowserPaneGeometry {
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
                // Wide's spacer stays the chrome gap band it has always been.
                palette::Surface::PillRowGap,
                false,
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
            paint_pill_row_gap(f, pane.spacer_area, palette::Surface::PillRowGap, false);
        }
        (_, true) => paint_pill_row_gap(f, pane.spacer_area, palette::Surface::PillRowGap, false),
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
    // A slot with no rows to spare keeps its single row rather than
    // collapsing to an empty rect. Wide never reaches this — `wide_hero_fits`
    // gates short areas out of the Wide skeleton — but the non-Wide panel is
    // the whole pane and can be one or two rows tall.
    let list_area = if list_area.height == 0 {
        list_panel
    } else {
        list_area
    };
    match &mut content.list {
        ListSlot::Search(search) => {
            // The search bar is the Selector row's chrome; the result rows are
            // the session's embedded canonical carrier painted through the
            // same PanelList surface as `ListSlot::Media` (design D3).
            let query = search.query().to_string();
            let loading = search.loading();
            render_search_box(f, pane.pills_area, &query, loading);
            if search.results_len() == 0 {
                // Zero rows: the same placeholder states an empty list box
                // paints (string parity with the legacy empty branch).
                let msg = if loading {
                    " Loading\u{2026}"
                } else {
                    " (empty)"
                };
                render_placeholder(f, list_area, msg);
            } else {
                search.sync_viewport(list_area.height.max(1) as usize);
                search.set_paint_policy(PanelListPaintPolicy::Wide {
                    focused: list_focused,
                });
                // The canonical rail owns the full panel row, exactly as for
                // `ListSlot::Media` (see that arm below).
                search.set_geometry(full_width_claim(list_panel, list_area), list_area);
                search.view(f, list_area);
            }
        }
        ListSlot::Media(list) => {
            // The panel drives the viewport clamp and the paint policy
            // (design D3): the slot fixes focus and the list-backdrop
            // selected row (design D6).
            list.sync_viewport(list_area.height.max(1) as usize);
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

    // The list slot's painted selection is the context-menu anchor's painted
    // truth.
    let selected = match &mut content.list {
        ListSlot::Media(list) => list.selected_row_rect(),
        ListSlot::Search(search) => search.selected_row_rect(),
        _ => None,
    };

    BrowserPaneGeometry {
        selector_bar: pane.pills_area,
        controls: controls_area,
        list_panel,
        list_area,
        selected,
    }
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

    // List box focus: the panel's bit, minus the Workspace. When the hero's
    // media list holds focus the browser list drops its green and rests —
    // green marks the focused list, never both at once.
    let list_focused = browser_focused && !content.workspace_focused();
    let browser = paint_browser_pane(
        f,
        pane,
        content,
        list_focused,
        hovered_selector,
        hits,
        windows,
    );

    // Hero pane: always the resting fill. It never takes the focused
    // surface when the media list (or its Workspace) holds focus — only
    // the list box above flips with the panel's focus. A focused Workspace
    // still shows through its own content-box tint.
    let hero_area = wide_hero_hero_pane(f, area, false, override_width)?;

    let mut geometry = WideSkeletonGeometry {
        browser: browser_panel,
        hero: hero_panel,
        selector_bar: browser.selector_bar,
        controls: browser.controls,
        list_panel: browser.list_panel,
        list_area: browser.list_area,
        hero_area,
        workspace: None,
        hero_image: None,
        overview_box: None,
        overview_content_length: 0,
        overview_viewport: 0,
        selected: browser.selected,
    };

    if let Some(hero) = content.hero.as_mut() {
        let composition = paint_library_hero_content(
            f,
            hero_area,
            hero,
            overview_scroll,
            hovered_link,
            &mut hits.links,
            &mut hits.workspace_selector,
            &mut windows.workspace_selector,
            crate::app::palette::Surface::HeroPane,
        );
        geometry.workspace = composition.workspace;
        geometry.hero_image = composition.hero_image;
        geometry.overview_box = composition.overview_box;
        geometry.overview_content_length = composition.overview_content_length;
        geometry.overview_viewport = composition.overview_viewport;
    }
    Some(geometry)
}

#[cfg(test)]
#[path = "wide_tests.rs"]
mod wide_tests;
