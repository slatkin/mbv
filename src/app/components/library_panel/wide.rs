//! The Wide Library panel skeleton (task 5.2, design D4): the Hero pane
//! (always the resting surface; only the focused list flips with focus —
//! the browser list rests while the Workspace media list holds it)
//! | gap | Browser pane (Selector row, list box). One skeleton for every Wide
//! library destination: destinations supply
//! typed [`LibraryPanelContent`] and paint nothing themselves.
//!
//! The list box paints through the current carrier's fixed Wide presentation
//! (carriers never switch presentation owners); the panel views it through the
//! provisional [`PanelList`] surface until task 5.8 formalizes the trait.

use ratatui::layout::Rect;

use ratatui::Frame;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::arrangements::library::{
    wide_library_panes_with_selector, WideLibraryPanes,
};
use crate::app::render::arrangements::wide_hero::WideHeroBrowserPane;
use crate::app::render::{
    render_placeholder, render_search_box, wide_hero_hero_pane, PillBarWindow, PANE_PAD_X,
    PANE_PAD_Y,
};

use super::content::{LibraryPanelContent, ListSlot, PanelList, PanelListPaintPolicy};
use super::hero_composition::{full_width_claim, paint_library_hero_content};
use super::slots::{paint_pill_row_gap, paint_selector_row};

pub(in crate::app) fn selector_row_visible(content: &LibraryPanelContent<'_>) -> bool {
    content.selector.is_some()
        || match &content.list {
            ListSlot::Search(_) => true,
            ListSlot::Media(list) => list.search_bar().is_some(),
            ListSlot::Empty { .. } => false,
        }
}

/// The skeleton's retained irregular-chrome hit registries, one per painted
/// pill row (ADR 0024: the mounted panel owns gesture state and resolves the
/// geometry it painted). The `LibraryPanel` component (task 5.9) owns these
/// registries; the skeleton pushes into them while painting.
#[derive(Debug, Default)]
pub(in crate::app) struct SkeletonHits {
    pub selector: HitRegions<usize>,
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
    /// The Selector row's pill-bar rect, or a zero-height rect when neither
    /// a selector nor Inline Search is present.
    #[cfg(test)]
    pub selector_bar: Rect,
    /// The list box's full panel rect (fill + border).
    pub list_panel: Rect,
    /// The list box's inset row-flow rect.
    pub list_area: Rect,
    /// The Hero pane's inset content rect.
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
    /// The Selector row's pill-bar rect, or a zero-height rect when neither
    /// a selector nor Inline Search is present.
    #[cfg(test)]
    pub(in crate::app) selector_bar: Rect,
    /// The list box's full panel rect (fill + border).
    pub(in crate::app) list_panel: Rect,
    /// The list box's inset row-flow rect.
    pub(in crate::app) list_area: Rect,
    /// The selected row's rect from the list slot's view, when one painted
    /// (the context-menu anchor's painted truth).
    pub(in crate::app) selected: Option<Rect>,
}

/// Paint inputs for one Browser pane pass: the panel/list focus bits and
/// the pointer/hit plumbing the painter feeds. Bundled so the painter keeps
/// a short signature without losing the per-input docs.
pub(in crate::app) struct BrowserPanePaintParams<'a> {
    /// Whether the browser list slot holds focus (drives the list's focused
    /// surface and the Wide paint policy).
    pub(in crate::app) list_focused: bool,
    /// The whole panel's own bit (what the shell paints the column body
    /// with): the Selector row's spacer is the panel showing through, so the
    /// full-width spacer band follows that bit, not the list pane's narrower
    /// one.
    pub(in crate::app) panel_focused: bool,
    /// Selector pill row under the pointer, if any.
    pub(in crate::app) hovered_selector: Option<usize>,
    /// Skeleton hit rects, accumulated by the painter.
    pub(in crate::app) hits: &'a mut SkeletonHits,
    /// Skeleton pill windows, accumulated by the painter.
    pub(in crate::app) windows: &'a mut SkeletonPillWindows,
}

/// Paints the Browser pane's list slot in the inset row-flow rect: the
/// slot's content (search chrome + results, media list, or placeholder),
/// viewport-clamped and geometry-set for this pass.
fn paint_browser_list_slot(
    f: &mut Frame,
    pills_area: Rect,
    list_panel: Rect,
    list_area: Rect,
    content: &mut LibraryPanelContent<'_>,
    list_focused: bool,
) {
    match &mut content.list {
        ListSlot::Search(search) => {
            // The search bar is the Selector row's chrome; the result rows are
            // the session's embedded canonical carrier painted through the
            // same PanelList surface as `ListSlot::Media` (design D3).
            let query = search.query().to_string();
            let loading = search.loading();
            render_search_box(f, pills_area, &query, loading);
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
                search.clamp_viewport(list_area.height.max(1) as usize);
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
            if let Some((query, loading)) = list.search_bar() {
                render_search_box(f, pills_area, &query, loading);
            }
            // The panel drives the viewport clamp and the paint policy
            // (design D3): the slot fixes focus and the list-backdrop
            // selected row (design D6).
            list.clamp_viewport(list_area.height.max(1) as usize);
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
}

/// Paints the Browser pane (Selector row and list box) and returns the role
/// rects it placed. Pure painting over the supplied pane geometry; the hero
/// pane and workspace content are painted elsewhere.
pub(in crate::app) fn paint_browser_pane(
    f: &mut Frame,
    pane: WideHeroBrowserPane,
    content: &mut LibraryPanelContent<'_>,
    params: BrowserPanePaintParams<'_>,
) -> BrowserPaneGeometry {
    let BrowserPanePaintParams {
        list_focused,
        panel_focused,
        hovered_selector,
        hits,
        windows,
    } = params;
    // Selector row: one pill bar + the panel's spacer. When neither a
    // SelectorRow nor search is present, the arrangement gives the content
    // area directly to the list; while searching, the box takes the bar's rect.
    let searching = match &content.list {
        ListSlot::Search(_) => true,
        ListSlot::Media(list) => list.search_bar().is_some(),
        ListSlot::Empty { .. } => false,
    };
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
                // The spacer is the panel showing through: it keeps the
                // column body's fill for the panel's focus bit.
                palette::Surface::PillRowGap,
                panel_focused,
            );
        }
        (None, false) => {}
        (_, true) => paint_pill_row_gap(
            f,
            pane.spacer_area,
            palette::Surface::PillRowGap,
            panel_focused,
        ),
    }

    // List box: fill, then the slot's content in the inset row-flow rect.
    let list_panel = pane.list_panel;
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
    paint_browser_list_slot(
        f,
        pane.pills_area,
        list_panel,
        list_area,
        content,
        list_focused,
    );

    // The list slot's painted selection is the context-menu anchor's painted
    // truth.
    let selected = match &mut content.list {
        ListSlot::Media(list) => list.selected_row_rect(),
        ListSlot::Search(search) => search.selected_row_rect(),
        ListSlot::Empty { .. } => None,
    };

    BrowserPaneGeometry {
        #[cfg(test)]
        selector_bar: pane.pills_area,
        list_panel,
        list_area,
        selected,
    }
}

/// Paints the Wide Library panel skeleton into `area`. Returns the frame's
/// role-rect geometry, or `None` when `area` does not fit the shared Wide
/// presentation (the Narrow skeleton, task 5.7, owns that breakpoint).
///
/// The Selector row is the panel's full-width band above both panes: its pill
/// bar spans the panel's full width and its spacer spans it too. The list box
/// fills the Browser pane below the band (then the
/// list presentation or the `ListSlot::Empty` placeholder). While
/// `ListSlot::Search` is active, the search box paints in the Selector band's
/// rect and the results in the list box, and the rest of the panel is
/// unchanged.
/// Paint inputs for the Wide skeleton pass: focus, pointer, hit plumbing,
/// and the breakpoint/layout knobs the skeleton needs. Bundled so the
/// painter keeps a short signature without losing the per-input docs.
pub(in crate::app) struct WideSkeletonPaintParams<'a> {
    /// Whether the browser panel holds focus.
    pub(in crate::app) browser_focused: bool,
    /// Width override for the hero pane, when one is active.
    pub(in crate::app) override_width: Option<u16>,
    /// The hero overview's scroll offset.
    pub(in crate::app) overview_scroll: usize,
    /// Selector pill row under the pointer, if any.
    pub(in crate::app) hovered_selector: Option<usize>,
    /// Hero overview link under the pointer, if any.
    pub(in crate::app) hovered_link: Option<usize>,
    /// Skeleton hit rects, accumulated by the painter.
    pub(in crate::app) hits: &'a mut SkeletonHits,
    /// Skeleton pill windows, accumulated by the painter.
    pub(in crate::app) windows: &'a mut SkeletonPillWindows,
    /// Terminal height, feeding the hero overview's viewport math.
    pub(in crate::app) terminal_height: u16,
    /// Whether the hero pane participates in this breakpoint's layout.
    pub(in crate::app) show_hero_pane: bool,
}

pub(in crate::app) fn render_wide_skeleton(
    f: &mut Frame,
    area: Rect,
    content: &mut LibraryPanelContent<'_>,
    params: WideSkeletonPaintParams<'_>,
) -> Option<WideSkeletonGeometry> {
    let WideSkeletonPaintParams {
        browser_focused,
        override_width,
        overview_scroll,
        hovered_selector,
        hovered_link,
        hits,
        windows,
        terminal_height,
        show_hero_pane,
    } = params;
    let WideLibraryPanes {
        pills_area,
        spacer_area,
        content_area,
        hero_panel,
        browser_panel,
        ..
    } = wide_library_panes_with_selector(
        area,
        PANE_PAD_X,
        PANE_PAD_Y,
        override_width,
        show_hero_pane,
        selector_row_visible(content),
    )?;
    let pane = WideHeroBrowserPane {
        pills_area,
        spacer_area,
        list_panel: browser_panel,
    };

    // List box focus: the panel's bit, minus the Workspace. When the hero's
    // media list holds focus the browser list drops its green and rests —
    // green marks the focused list, never both at once.
    let list_focused = browser_focused && !content.workspace_focused();
    let browser = paint_browser_pane(
        f,
        pane,
        content,
        BrowserPanePaintParams {
            list_focused,
            panel_focused: browser_focused,
            hovered_selector,
            hits,
            windows,
        },
    );

    // Hero pane: always the resting fill. It never takes the focused
    // surface when the media list (or its Workspace) holds focus — only
    // the list box above flips with the panel's focus. A focused Workspace
    // still shows through its own content-box tint. It is fed the band-reduced
    // `content_area` (D2), so the hero starts below the full-width Selector
    // band instead of at the raw panel top. Flat TV episode modes omit the
    // pane in the arrangement, giving their list the entire content area.
    let hero_area = if show_hero_pane {
        wide_hero_hero_pane(f, content_area, false, override_width)
    } else {
        Rect::new(content_area.x, content_area.y, 0, content_area.height)
    };

    let mut geometry = WideSkeletonGeometry {
        browser: browser_panel,
        hero: hero_panel,
        #[cfg(test)]
        selector_bar: browser.selector_bar,
        list_panel: browser.list_panel,
        list_area: browser.list_area,
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
            terminal_height,
        );
        geometry.workspace = composition.workspace;
        geometry.hero_image = composition.hero_image;
        geometry.overview_box = composition.overview_box;
        geometry.overview_content_length = composition.overview_content_length;
        geometry.overview_viewport = composition.overview_viewport;
    }
    Some(geometry)
}
