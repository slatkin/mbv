//! The Wide Library panel skeleton (task 5.2, design D4): the Hero pane
//! (resting surface; focused only when a Workspace is present and focused,
//! design D6) | gap | Browser pane (Selector row, List controls row, list
//! box). One skeleton for every Wide library destination: destinations supply
//! typed [`LibraryPanelContent`] and paint nothing themselves.
//!
//! The list box's presentation is the current carrier's (Wide/Inline via the
//! owner's `ensure_presentation`); the panel views it through the provisional
//! [`PanelList`] surface until task 5.8 formalizes the trait.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::arrangements::library::{wide_library_panes, WideLibraryPanes};
use crate::app::render::arrangements::padded_rect;
use crate::app::render::{
    render_inline_search, render_placeholder, wide_hero_browser_border, wide_hero_browser_pane,
    wide_hero_hero_pane, PANE_PAD_X, PANE_PAD_Y,
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
    /// The Workspace box's (panel, content) rects, when one painted.
    pub workspace: Option<(Rect, Rect)>,
    /// The projected hero image's paint (task 5.10, design D9), when the
    /// header reserved a ready image's box.
    pub hero_image: Option<PanelHeroImagePaint>,
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
/// List controls row, and the list box (fill + `wide_hero_browser_border`,
/// then the list presentation or the `ListSlot::Empty` placeholder). While
/// `ListSlot::Search` is active, the search box paints in the Selector row's
/// rect and the results in the list box, and the rest of the panel is
/// unchanged.
pub(in crate::app) fn render_wide_skeleton(
    f: &mut Frame,
    area: Rect,
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
    override_width: Option<u16>,
    hits: &mut SkeletonHits,
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
                &mut hits.selector,
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
                Some(SELECTOR_ROW_PREFIX),
                &mut hits.selector,
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

    // List box: fill + shared border, then the slot's content in the inset
    // row-flow rect.
    wide_hero_browser_border(f, list_panel, browser_focused);
    let list_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);
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
                browser_focused,
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
                focused: browser_focused,
            });
            // The canonical rail owns the full panel row (matching the
            // pre-migration wide rail): the selected background reaches
            // `list_panel`'s border while the row flow/hit geometry stays on
            // the inset `list_area`, so `wide_media_row`'s own 2-column text
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

    // Hero pane: the fill's focus is derived, not declared (design D6) —
    // focusable exactly when a Workspace is present, focused exactly when
    // that Workspace holds focus.
    let focused = content
        .hero
        .as_ref()
        .and_then(|hero| hero.workspace.as_ref())
        .is_some_and(|workspace| workspace.focused);
    let hero_area = wide_hero_hero_pane(f, area, focused, override_width)?;

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
        selected,
    };

    if let Some(hero) = content.hero.as_mut() {
        // The Hero header (task 5.5): policy arm, placeholder artwork box,
        // one title/meta painter, and the overview box when overview text
        // exists. Returns the first unpainted row and the projected image's
        // reserved box (task 5.10: `Ready` reserves; the shell paints).
        let (next_row, image_box) = paint_hero_pane_content(f, hero_area, &*hero);
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
                ));
            }
        }
    }
    Some(geometry)
}

/// The Workspace (task 5.6, design D6): an optional Selector row over one
/// Main content box holding the Workspace's `&mut dyn PanelList`. The box's
/// surface is derived, not declared — accent-soft while the workspace list
/// holds focus, backdrop otherwise (user decision: Music's behaviour for
/// all) — and the list's selected row is fixed to the owning surface by the
/// slot. Returns the (panel, content) rects.
fn paint_workspace_box(
    f: &mut Frame,
    workspace_rect: Rect,
    workspace: &mut super::content::Workspace<'_>,
    hits: &mut HitRegions<usize>,
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
                Some(WORKSPACE_SELECTOR_PREFIX),
                hits,
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
