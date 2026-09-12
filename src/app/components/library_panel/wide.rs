//! The Wide Library panel skeleton (task 5.2, design D4): the Browser pane
//! (Selector row, List controls row, list box) | gap | Hero pane (resting
//! surface; focused only when a Workspace is present and focused, design
//! D6). One skeleton for every Wide library destination: destinations supply
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
};
use crate::app::render::place_media_list_below;

/// Blank rows between the header/overview content's painted bottom edge and
/// the Workspace box (design D3: the Workspace sits below the overview).
const WORKSPACE_GAP_ROWS: u16 = 1;

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
            paint_pill_bar_row(f, pane.pills_area, &[], None, &mut hits.selector);
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
                throbber: None,
            });
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
            // still own its own paint.
            paint_pill_bar_row(f, bar, &selector.pills, selector.active, hits);
        }
        box_area = Rect {
            y: bar.bottom(),
            height: workspace_rect.height.saturating_sub(1),
            ..workspace_rect
        };
    }
    // The panel's own surface derivation (design D6): accent-soft while the
    // workspace list holds focus, backdrop otherwise. The deleted
    // `WideHeroContentBoxSurface` enum's focused arm is this one bool.
    let panel = Rect {
        x: box_area.x.saturating_add(PANE_PAD_X),
        width: box_area.width.saturating_sub(PANE_PAD_X * 2),
        ..box_area
    };
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
        .set_paint_policy(super::content::PanelListPaintPolicy::Wide {
            focused: workspace.focused,
            throbber: None,
        });
    workspace.list.view(f, content);
    (panel, content)
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod wide_skeleton_tests {
    use super::*;
    use crate::app::components::inline_search::{InlineSearch, SearchPool};
    use crate::app::components::library_panel::content::{
        ArtworkShape, HeroArtwork, HeroContent, HeroFacts, ListControls, PanelList, SelectorRow,
        Workspace,
    };
    use crate::app::render::arrangements::library::wide_library_panes;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;

    // Tall enough for a Landscape header (16:9 artwork box) to leave the
    // title and meta rows visible below it.
    const AREA: Rect = Rect::new(0, 0, 100, 30);

    /// A test double over [`PanelList`]: paints its rows and retains the rect
    /// it was viewed into.
    struct StubList {
        rows: Vec<&'static str>,
        painted: Option<Rect>,
    }

    impl StubList {
        fn with_rows(rows: Vec<&'static str>) -> Self {
            Self {
                rows,
                painted: None,
            }
        }
    }

    impl PanelList for StubList {
        fn set_presentation(
            &mut self,
            _presentation: crate::app::components::media_list::Presentation,
            _viewport_height: usize,
        ) {
        }

        fn set_paint_policy(
            &mut self,
            _policy: crate::app::components::library_panel::content::PanelListPaintPolicy,
        ) {
        }

        fn view(&mut self, f: &mut Frame, rect: Rect) {
            self.painted = Some(rect);
            for (index, row) in self.rows.iter().enumerate() {
                if index as u16 >= rect.height {
                    break;
                }
                f.render_widget(
                    Paragraph::new(*row),
                    Rect {
                        y: rect.y + index as u16,
                        height: 1,
                        ..rect
                    },
                );
            }
        }
    }

    fn hero_facts(title: &str) -> HeroFacts {
        HeroFacts {
            title: title.into(),
            meta_rows: vec!["2020".into()],
            artwork: HeroArtwork {
                shape: ArtworkShape::Landscape,
                source: None,
                image: crate::app::components::library_panel::content::HeroImageState::None,
            },
        }
    }

    fn draw_skeleton(
        content: &mut LibraryPanelContent<'_>,
    ) -> (ratatui::buffer::Buffer, WideSkeletonGeometry, SkeletonHits) {
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        let mut hits = SkeletonHits::default();
        let mut geometry = None;
        terminal
            .draw(|f| {
                geometry = render_wide_skeleton(f, AREA, content, false, None, &mut hits);
            })
            .unwrap();
        (
            terminal.backend().buffer().clone(),
            geometry.expect("wide skeleton painted"),
            hits,
        )
    }

    fn text_in(buf: &ratatui::buffer::Buffer, area: Rect, needle: &str) -> bool {
        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                line.push_str(buf[(x, y)].symbol());
            }
            if line.contains(needle) {
                return true;
            }
        }
        false
    }

    fn browser_pane(
        area: Rect,
    ) -> crate::app::render::arrangements::wide_hero::WideHeroBrowserPane {
        let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y, None).expect("wide area");
        wide_hero_browser_pane(panes.browser_panel, panes.browser_area)
    }

    #[test]
    fn read_only_hero_renders_resting_with_selector_and_list() {
        let mut list = StubList::with_rows(vec!["Alpha", "Beta"]);
        let mut content = LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into()],
                active: Some(0),
            }),
            controls: None,
            list: ListSlot::Media(&mut list),
            hero: Some(HeroContent {
                facts: hero_facts("Dune"),
                overview: None,
                workspace: None,
            }),
        };
        let (buf, geo, hits) = draw_skeleton(&mut content);

        // Hero pane: read-only, so it always renders the resting surface.
        assert_eq!(
            buf[(geo.hero.x, geo.hero.y)].bg,
            palette::surface_colors(palette::Surface::HeroPane, false).fill
        );
        assert!(geo.workspace.is_none());

        // Hero facts painted inside the hero pane's content rect.
        assert!(rect_contains(geo.hero, geo.hero_area));
        assert!(text_in(&buf, geo.hero_area, "Dune"));
        assert!(text_in(&buf, geo.hero_area, "2020"));

        // Selector row: pill bar painted, spacer below, hit retained.
        assert!(text_in(&buf, geo.selector_bar, "All"));
        let (pill_rect, pill_id) = hits.selector.regions()[0];
        assert_eq!(pill_id, 0);
        let point = ratatui::layout::Position {
            x: pill_rect.x + 1,
            y: pill_rect.y,
        };
        assert_eq!(hits.selector.resolve(point), Some(&0));
        // The spacer row directly below the bar paints the panel's gap band.
        let pane = browser_pane(AREA);
        assert_eq!(pane.spacer_area.bottom(), pane.pills_area.bottom() + 1);
        assert_eq!(
            buf[(pane.spacer_area.x, pane.spacer_area.y)].bg,
            palette::surface_colors(palette::Surface::PillRowGap, false).fill
        );

        // List rows painted inside the list box's row-flow rect, which sits
        // inside the browser pane; the stub saw exactly that rect.
        assert!(rect_contains(geo.browser, geo.list_area));
        assert_eq!(list.painted, Some(geo.list_area));
        assert!(text_in(&buf, geo.list_area, "Alpha"));
        assert!(text_in(&buf, geo.list_area, "Beta"));

        // No controls row content: the row is absent and the list box starts
        // exactly where the shared browser-pane primitive places it.
        assert!(geo.controls.is_none());
        assert_eq!(geo.list_panel.y, browser_pane(AREA).list_panel.y);
        // The list box is bordered: the fill runs under the row flow.
        assert_eq!(
            buf[(geo.list_panel.x, geo.list_panel.y)].bg,
            palette::surface_colors(palette::Surface::LibraryPanel, false).fill
        );
    }

    #[test]
    fn list_controls_row_moves_the_list_box_down_one_row() {
        let mut list = StubList::with_rows(vec!["Alpha"]);
        let mut content = LibraryPanelContent {
            selector: None,
            controls: Some(ListControls {
                pills: None,
                label: Some("17 items".into()),
            }),
            list: ListSlot::Media(&mut list),
            hero: None,
        };
        let (buf, geo, _hits) = draw_skeleton(&mut content);
        let pane = browser_pane(AREA);
        let controls = geo.controls.expect("controls row reserved");
        assert!(text_in(&buf, controls, "17 items"));
        // Relational: the controls row occupies the pane's first list row and
        // the list box starts directly below it.
        assert_eq!(controls.y, pane.list_panel.y);
        assert_eq!(geo.list_panel.y, controls.bottom());
    }

    #[test]
    fn workspace_selector_with_active_none_paints_no_active_pill() {
        let mut list = StubList::with_rows(vec!["Alpha"]);
        let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut list),
            hero: Some(HeroContent {
                facts: hero_facts("Series"),
                overview: None,
                workspace: Some(Workspace {
                    selector: Some(SelectorRow {
                        pills: vec!["Seasons".into(), "Episodes".into()],
                        active: None,
                    }),
                    list: &mut workspace_list,
                    focused: true,
                }),
            }),
        };
        let (buf, _geo, hits) = draw_skeleton(&mut content);

        // The workspace selector's pills paint and their hitboxes are kept;
        // `active: None` paints no active pill — no cell in any retained
        // pill rect carries the selected chip's surface, and every pill's
        // label cell paints the unselected pill foreground.
        assert_eq!(hits.workspace_selector.regions().len(), 2);
        for (rect, _) in hits.workspace_selector.regions() {
            for y in rect.top()..rect.bottom() {
                for x in rect.left()..rect.right() {
                    assert_ne!(
                        buf[(x, y)].bg,
                        palette::PILL_SELECTED_BG,
                        "active: None must paint no active pill"
                    );
                }
            }
            let cell = &buf[(rect.x + 1, rect.y)];
            assert_eq!(cell.style().fg, Some(palette::PILL_FG));
        }
        // A hit test inside a painted pill still resolves its index.
        let (first_rect, first_id) = hits.workspace_selector.regions()[0];
        let point = ratatui::layout::Position {
            x: first_rect.x + 1,
            y: first_rect.y,
        };
        assert_eq!(hits.workspace_selector.resolve(point), Some(&first_id));
    }

    #[test]
    fn focused_workspace_hero_uses_the_focused_surfaces() {
        let mut list = StubList::with_rows(vec!["Alpha"]);
        let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut list),
            hero: Some(HeroContent {
                facts: hero_facts("Series"),
                overview: None,
                workspace: Some(Workspace {
                    selector: None,
                    list: &mut workspace_list,
                    focused: true,
                }),
            }),
        };
        let (buf, geo, _hits) = draw_skeleton(&mut content);

        // A hero with a Workspace is focusable and its surface is focused.
        assert_eq!(
            buf[(geo.hero.x, geo.hero.y)].bg,
            palette::surface_colors(palette::Surface::HeroPane, true).fill
        );
        let (box_panel, box_content) = geo.workspace.expect("workspace box painted");
        // Accent-soft box surface while the workspace list holds focus.
        assert_eq!(
            buf[(box_panel.x, box_panel.y)].bg,
            palette::surface_colors(palette::Surface::MainContentBox, true).fill
        );
        // The workspace list is viewed into the box's content rect.
        assert_eq!(workspace_list.painted, Some(box_content));
        assert!(text_in(&buf, box_content, "Ep 1"));
        assert!(rect_contains(geo.hero, box_panel));
    }

    #[test]
    fn unfocused_workspace_hero_renders_resting_surfaces() {
        let mut list = StubList::with_rows(vec!["Alpha"]);
        let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut list),
            hero: Some(HeroContent {
                facts: hero_facts("Series"),
                overview: None,
                workspace: Some(Workspace {
                    selector: None,
                    list: &mut workspace_list,
                    focused: false,
                }),
            }),
        };
        let (buf, geo, _hits) = draw_skeleton(&mut content);
        assert_eq!(
            buf[(geo.hero.x, geo.hero.y)].bg,
            palette::surface_colors(palette::Surface::HeroPane, false).fill
        );
        let (box_panel, _) = geo.workspace.expect("workspace box painted");
        assert_eq!(
            buf[(box_panel.x, box_panel.y)].bg,
            palette::surface_colors(palette::Surface::MainContentBox, false).fill
        );
    }

    #[test]
    fn workspace_box_sits_one_blank_row_below_the_hero_content() {
        let mut list = StubList::with_rows(vec!["Alpha"]);
        let mut workspace_list = StubList::with_rows(vec!["Ep 1"]);
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut list),
            hero: Some(HeroContent {
                facts: hero_facts("Dune"),
                overview: None,
                workspace: Some(Workspace {
                    selector: None,
                    list: &mut workspace_list,
                    focused: false,
                }),
            }),
        };
        let (buf, geo, _hits) = draw_skeleton(&mut content);
        let (box_panel, _) = geo.workspace.expect("workspace box painted");
        assert!(rect_contains(geo.hero, box_panel));

        // One blank row between the painted hero content's bottom and the
        // Workspace box (task 5.6): the row directly above the box carries
        // only the hero pane's resting fill across the pane's width.
        let fill = palette::surface_colors(palette::Surface::HeroPane, false).fill;
        let gap_row = box_panel.y - 1;
        assert!(gap_row > geo.hero.y, "gap row inside the hero pane");
        for x in geo.hero.left()..geo.hero.right() {
            assert_eq!(
                buf[(x, gap_row)].bg,
                fill,
                "row {gap_row} must stay blank between the content and the box"
            );
        }
        // The row above the gap is the painted content's bottom — the meta
        // row, compared by text, not by absolute coordinates.
        let mut content_row = String::new();
        for x in geo.hero.left()..geo.hero.right() {
            content_row.push_str(buf[(x, gap_row - 1)].symbol());
        }
        assert!(
            content_row.contains("2020"),
            "content ends directly above the gap"
        );
    }

    #[test]
    fn active_search_takes_the_selector_row_and_the_list_box() {
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(SearchPool::Items(vec![crate::app::tests::make_item(
            "Alpha", "Movie",
        )]));
        search.restore_query("Alp".into());
        let mut content = LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into()],
                active: Some(0),
            }),
            controls: None,
            list: ListSlot::Search(&mut search),
            hero: Some(HeroContent {
                facts: hero_facts("Dune"),
                overview: None,
                workspace: None,
            }),
        };
        let (buf, geo, _hits) = draw_skeleton(&mut content);

        // The search box occupies the Selector row's place: query text in the
        // bar rect, and no selector pill painted there instead.
        assert!(text_in(&buf, geo.selector_bar, "SEARCH:"));
        assert!(text_in(&buf, geo.selector_bar, "Alp"));
        assert!(
            !text_in(&buf, geo.selector_bar, "\u{25e2}"),
            "no selector pill under the search box"
        );
        // The results occupy the list box; the search session retained the
        // row-flow rect the skeleton gave it.
        assert!(text_in(&buf, geo.list_area, "Alpha"));
        assert_eq!(*search.layout(), geo.list_area);
        // The rest of the panel is unchanged: hero pane resting with its facts.
        assert_eq!(
            buf[(geo.hero.x, geo.hero.y)].bg,
            palette::surface_colors(palette::Surface::HeroPane, false).fill
        );
        assert!(text_in(&buf, geo.hero_area, "Dune"));
    }

    #[test]
    fn empty_list_slot_paints_its_placeholder_in_the_list_box() {
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Empty {
                loading: false,
                text: "Nothing here".into(),
            },
            hero: None,
        };
        let (buf, geo, _hits) = draw_skeleton(&mut content);
        assert!(text_in(&buf, geo.list_area, "Nothing here"));
    }

    #[test]
    fn empty_loading_list_slot_paints_the_loading_placeholder() {
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Empty {
                loading: true,
                text: String::new(),
            },
            hero: None,
        };
        let (buf, geo, _hits) = draw_skeleton(&mut content);
        assert!(text_in(&buf, geo.list_area, "Loading"));
    }

    #[test]
    fn sub_breakpoint_area_paints_nothing() {
        let mut list = StubList::with_rows(vec!["Alpha"]);
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut list),
            hero: None,
        };
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        let mut hits = SkeletonHits::default();
        let narrow = Rect {
            width: crate::app::TWO_COLUMN_THRESHOLD - 1,
            ..AREA
        };
        let mut geometry = None;
        terminal
            .draw(|f| {
                geometry = render_wide_skeleton(f, narrow, &mut content, false, None, &mut hits);
            })
            .unwrap();
        assert!(geometry.is_none());
        let buf = terminal.backend().buffer();
        // Untouched: every cell still carries the empty terminal's style,
        // compared relatively so no raw colour primitive is named here.
        let untouched = buf[(0, 0)].bg;
        for y in 0..AREA.height {
            for x in 0..AREA.width {
                assert_eq!(buf[(x, y)].bg, untouched);
            }
        }
    }

    fn rect_contains(outer: Rect, inner: Rect) -> bool {
        outer.left() <= inner.left()
            && outer.right() >= inner.right()
            && outer.top() <= inner.top()
            && outer.bottom() >= inner.bottom()
    }
}
