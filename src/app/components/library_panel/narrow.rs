//! The Narrow Library panel skeleton (task 5.7, design D4/D7): Selector row,
//! optional List controls row, and the list box carrying the Inline
//! presentation — whose admitted detail block is the one Narrow inline hero,
//! derived by the panel from the same [`HeroContent`] the Wide header uses.
//! One skeleton for every Narrow library destination: destinations supply
//! typed [`LibraryPanelContent`] and paint nothing themselves.
//!
//! The inline hero (design D7) has no selector, controls or constituent rows
//! inside it: the policy-chosen artwork box right-aligned at a size derived
//! from its shape's aspect, the title and the meta rows (coloured as in
//! Wide, `HERO_META_ROLES[n % 3]`) and the overview wrapping around it, and
//! the text reclaiming the full width below the image.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::components::media_list::{Presentation, SelectedRowSurface};
use crate::app::palette;
use crate::app::render::arrangements::library::selected_detail_content_area;
use crate::app::render::arrangements::wide_hero::pill_bar_areas;
use crate::app::render::{
    render_artwork_placeholder, render_inline_search, render_placeholder, selected_detail_shell,
    HERO_BLOCK_EXTRA_ROWS, SELECTED_BLOCK_SIDE_PADDING,
};

use super::content::{LibraryPanelContent, ListSlot, PanelHeroImagePaint, PanelListPaintPolicy};
use super::slots::{paint_list_controls_row, paint_pill_row_gap, paint_selector_row};
use super::wide::SkeletonHits;

/// Columns the inline hero's wrapping text keeps beside a right-aligned
/// artwork box (the Wide header's rule, applied to the Narrow form).
const INLINE_HERO_MIN_TEXT_COLS: u16 = 16;

/// Row cap for the inline hero's image box: a Narrow detail block replaces
/// the selected row inside a list viewport, so the image never grows past
/// ten painted rows regardless of its aspect.
const INLINE_HERO_MAX_IMAGE_ROWS: u16 = 10;

/// One column of quiet gap between the wrapping text and the image box.
const INLINE_HERO_TEXT_GAP_COLS: u16 = 1;

/// Everything the Narrow skeleton placed this frame, in role-rect form. The
/// panel component (task 5.9) retains these rects; tests assert containment
/// against them, never absolute coordinates (design D15).
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct NarrowSkeletonGeometry {
    /// The Selector row's pill-bar rect, reserved even when the destination
    /// supplies no `SelectorRow` (the Inline Search box takes it).
    pub selector_bar: Rect,
    /// The List controls row's rect, when the destination supplies content.
    pub controls: Option<Rect>,
    /// The list box's row-flow rect the Inline presentation painted into.
    pub list_area: Rect,
    /// The admitted inline hero's detail-block rect, when the flow admitted
    /// it (the fallback paints the ordinary selected row instead).
    pub inline_hero: Option<Rect>,
    /// The projected hero image's paint (task 5.10, design D9), when the
    /// inline hero reserved a ready image's box.
    pub inline_hero_image: Option<PanelHeroImagePaint>,
    /// The painted selection's rect: the admitted inline hero block, else
    /// the selected row's rect (the context-menu anchor's painted truth).
    pub selected: Option<Rect>,
}

/// The inline hero's paint plan (design D7): the image box's size derived
/// from the policy shape's aspect, and the detail-block row count the Inline
/// presentation needs to admit the block. One function serves both the
/// `desired_detail_rows` the panel feeds the paint policy and the hero
/// painter, so the two can never disagree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct InlineHeroPlan {
    /// The full detail-block height including the shared selected-block
    /// shell's extra rows (`HERO_BLOCK_EXTRA_ROWS`).
    pub detail_rows: usize,
    /// The right-aligned image box's cell width.
    pub image_cols: u16,
    /// The right-aligned image box's cell height.
    pub image_rows: u16,
}

/// The cell-folded width:height ratios per artwork shape — the same
/// cell-aspect fold the Wide header's `hero_artwork_box` applies (a terminal
/// cell is roughly twice as tall as it is wide, so a 2:3 portrait box is
/// `h*4/3` cells wide, a square `h*2`, and 16:9 landscape is `w*9/32` tall).
fn shape_ratio(shape: super::content::ArtworkShape) -> (u16, u16) {
    match shape {
        super::content::ArtworkShape::Landscape => (32, 9),
        super::content::ArtworkShape::Square => (2, 1),
        super::content::ArtworkShape::Portrait => (4, 3),
    }
}

/// The paint-free inline-hero layout for one list row's width (design D7):
/// the artwork box is right-aligned, sized from its aspect — the decoded
/// source's aspect when the projection projected a ready image (task 5.10's
/// decoded-size arm), the policy shape's aspect otherwise — capped so the
/// wrapping text keeps [`INLINE_HERO_MIN_TEXT_COLS`] columns and the image
/// never exceeds [`INLINE_HERO_MAX_IMAGE_ROWS`] rows; the detail block is
/// the taller of the image and the wrapped text, plus the shared shell's
/// extra rows. One layout site: the panel calls this for the Inline
/// presentation's `desired_detail_rows` and the hero painter calls it again
/// with the admitted block's width.
pub(in crate::app) fn inline_hero_plan(
    block_width: u16,
    hero: &super::content::HeroContent<'_>,
) -> InlineHeroPlan {
    let content_width = block_width.saturating_sub(SELECTED_BLOCK_SIDE_PADDING.saturating_mul(2));
    // Decoded-size arm (design D7): a ready image's own pixel aspect sets
    // the box ratio (cells are ~2x taller than wide), the policy shape's
    // aspect otherwise.
    let (num, den) = match &hero.facts.artwork.image {
        super::content::HeroImageState::Ready {
            decoded: Some((w, h)),
            ..
        } if *w > 0 && *h > 0 => (
            (*w * 2).min(u16::MAX as u32) as u16,
            (*h).min(u16::MAX as u32) as u16,
        ),
        _ => shape_ratio(hero.facts.artwork.shape),
    };
    let max_cols = content_width.saturating_sub(INLINE_HERO_MIN_TEXT_COLS);
    let image_cols = max_cols
        .min(
            (INLINE_HERO_MAX_IMAGE_ROWS
                .saturating_mul(num)
                .saturating_add(den - 1))
                / den,
        )
        .min(content_width);
    let image_rows = (image_cols.saturating_mul(den) / num.max(1)).min(INLINE_HERO_MAX_IMAGE_ROWS);
    let plan = InlineHeroPlan {
        detail_rows: 0,
        image_cols,
        image_rows,
    };
    let text_rows = inline_hero_text_rows(block_width, hero, &plan);
    InlineHeroPlan {
        detail_rows: text_rows.max(image_rows as usize) + HERO_BLOCK_EXTRA_ROWS as usize,
        image_cols,
        image_rows,
    }
}

/// Wraps the hero's text segments across the changing row widths of the
/// wrap-around form: rows beside the image use the beside-image width, rows
/// at or below the image's bottom reclaim the full content width (design
/// D7). Each segment (title, one meta row, overview) starts a fresh row and
/// occupies at least one row; every row keeps its segment's style.
fn inline_hero_lines(
    block_width: u16,
    hero: &super::content::HeroContent<'_>,
    plan: &InlineHeroPlan,
) -> Vec<(String, Style)> {
    let content_width = block_width.saturating_sub(SELECTED_BLOCK_SIDE_PADDING.saturating_mul(2));
    let content_x = SELECTED_BLOCK_SIDE_PADDING;
    let image_x = content_x.saturating_add(
        content_width
            .saturating_sub(plan.image_cols)
            .min(content_width),
    );
    let beside_width = image_x.saturating_sub(content_x).saturating_sub(1);
    let width_at_row = |row: usize| -> usize {
        if (row as u16) < plan.image_rows {
            beside_width as usize
        } else {
            content_width as usize
        }
    };

    let mut segments: Vec<(String, Style)> =
        Vec::with_capacity(1 + hero.facts.meta_rows.len() + usize::from(hero.overview.is_some()));
    segments.push((
        hero.facts.title.clone(),
        Style::default().fg(palette::TEXT_STRONG),
    ));
    for (index, row) in hero.facts.meta_rows.iter().enumerate() {
        segments.push((
            row.clone(),
            Style::default().fg(palette::HERO_META_ROLES[index % palette::HERO_META_ROLES.len()]),
        ));
    }
    if let Some(overview) = hero
        .overview
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        segments.push((
            overview.to_string(),
            Style::default().fg(palette::TEXT_MUTED),
        ));
    }

    let mut lines: Vec<(String, Style)> = Vec::new();
    for (text, style) in segments {
        let mut current = String::new();
        for word in text.split_whitespace() {
            let available = width_at_row(lines.len());
            if current.is_empty() {
                current.push_str(word);
            } else if current.width() + 1 + word.width() <= available {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push((std::mem::take(&mut current), style));
                current.push_str(word);
            }
        }
        // Each segment occupies at least one row, so the meta rows stay on
        // distinct lines exactly as the Wide header paints them.
        lines.push((current, style));
    }
    lines
}

/// The wrapped text row count for one plan: the flow the hero painter paints.
fn inline_hero_text_rows(
    block_width: u16,
    hero: &super::content::HeroContent<'_>,
    plan: &InlineHeroPlan,
) -> usize {
    inline_hero_lines(block_width, hero, plan).len()
}

/// Paints one inline hero into the admitted detail block (design D7): the
/// shared selected-block shell, the artwork box right-aligned (sized from
/// the projected image's decoded aspect when ready, the policy shape's
/// aspect otherwise), and the title/meta/overview text wrapping around it in
/// the three meta colours, reclaiming the full width below the image. No
/// selector, controls or constituent rows render inside the block. Returns
/// the image box's rect when a projected ready image should paint there
/// (task 5.10: `Ready` reserves; the shell paints after view).
pub(in crate::app) fn paint_inline_hero(
    f: &mut Frame,
    block: Rect,
    hero: &super::content::HeroContent<'_>,
    focused: bool,
) -> Option<Rect> {
    let plan = inline_hero_plan(block.width, hero);
    if plan.detail_rows == 0 || block.height == 0 {
        return None;
    }
    // The shared selected-block shell: the same ▁/▔ framed shell every
    // inline selected-detail block paints (one implementation, no second
    // loop as underpaint).
    selected_detail_shell(f, block, plan.detail_rows as u16, focused);
    let content =
        selected_detail_content_area(block, SELECTED_BLOCK_SIDE_PADDING, HERO_BLOCK_EXTRA_ROWS);
    // The image box: right-aligned, sized from its aspect — the projected
    // image's decoded aspect when ready, the policy shape's otherwise.
    let image = Rect {
        x: content
            .right()
            .saturating_sub(plan.image_cols)
            .max(content.x),
        y: content.y,
        width: plan.image_cols.min(content.width),
        height: plan.image_rows.min(content.height),
    };
    let image_ready = matches!(
        hero.facts.artwork.image,
        super::content::HeroImageState::Ready { .. }
    );
    if image.width > 0 && image.height > 0 && !image_ready {
        // Placeholder at full box size while loading or imageless (spec: the
        // box keeps its size and shows the shared placeholder).
        render_artwork_placeholder(f, image);
    }

    for (offset, (text, style)) in inline_hero_lines(block.width, hero, &plan)
        .into_iter()
        .enumerate()
    {
        let row = content.y.saturating_add(offset as u16);
        if row >= content.bottom() {
            break;
        }
        let row_width = if (offset as u16) < plan.image_rows {
            image.x.saturating_sub(content.x).saturating_sub(1)
        } else {
            content.width
        };
        if row_width == 0 {
            continue;
        }
        f.render_widget(
            Paragraph::new(text).style(style),
            Rect {
                x: content.x,
                y: row,
                width: row_width.min(content.width),
                height: 1,
            },
        );
    }
    (image.width > 0 && image.height > 0 && image_ready).then_some(image)
}

/// Paints the Narrow Library panel skeleton into `area`. Rows top-to-bottom:
/// the Selector row (one pill bar + the panel's spacer, reserved even without
/// a `SelectorRow` — the Inline Search box takes the bar's rect while a
/// search is active), the optional List controls row, and the list box
/// carrying the Inline presentation. The panel computes the Inline
/// presentation's `desired_detail_rows` from the hero content (design D7) and
/// paints the admitted inline hero into the detail block the presentation
/// reserved.
pub(in crate::app) fn render_narrow_skeleton(
    f: &mut Frame,
    area: Rect,
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
    hits: &mut SkeletonHits,
) -> NarrowSkeletonGeometry {
    // Selector row: one pill bar + the panel's spacer, reserved even without
    // a SelectorRow — while a search is active the box takes the bar's rect
    // (spec: the Selector row's place is reserved for the search box) and
    // the spacer keeps the reserved gap.
    let areas = pill_bar_areas(area);
    let searching = matches!(content.list, ListSlot::Search(_));
    match (&content.selector, searching) {
        (Some(selector), false) => {
            paint_selector_row(
                f,
                areas.pills_area,
                areas.spacer_area,
                selector,
                &mut hits.selector,
            );
        }
        _ => paint_pill_row_gap(f, areas.spacer_area),
    }

    // List controls row: reserved only when the destination supplies
    // content; without it the rows below move up (spec scenario).
    let (list_panel, controls_area) = match &content.controls {
        Some(controls) if areas.content_area.height > 0 => {
            let row = Rect {
                height: 1,
                ..areas.content_area
            };
            paint_list_controls_row(f, row, controls, &mut hits.controls);
            (
                Rect {
                    y: row.bottom(),
                    height: areas.content_area.height.saturating_sub(1),
                    ..areas.content_area
                },
                Some(row),
            )
        }
        _ => (areas.content_area, None),
    };

    let list_area = list_panel;
    // The panel computes the Inline `desired_detail_rows` from the hero
    // content before the view (design D3/D7): the policy carries it.
    let desired_detail_rows = content
        .hero
        .as_ref()
        .map(|hero| inline_hero_plan(list_area.width, hero).detail_rows)
        .unwrap_or(0);

    let mut inline_hero = None;
    let mut inline_hero_image = None;
    match &mut content.list {
        ListSlot::Search(search) => {
            let items = search.ordered_items();
            let query = search.query().to_string();
            let loading = search.loading();
            let cursor = search.cursor();
            let scroll_in = search.scroll();
            let new_scroll = render_inline_search(
                f,
                areas.pills_area,
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
            // D3): the Narrow breakpoint selects the Inline presentation, and
            // the slot fixes focus and the list-backdrop selected row.
            list.set_presentation(Presentation::Inline, list_area.height.max(1) as usize);
            list.set_paint_policy(PanelListPaintPolicy::Inline {
                focused: browser_focused,
                selected: SelectedRowSurface::ListBackdrop,
                desired_detail_rows,
            });
            list.view(f, list_area);
            if let Some(hero) = content.hero.as_ref() {
                if let Some(block) = list.detail_rect() {
                    if let Some(image_box) = paint_inline_hero(f, block, hero, browser_focused) {
                        if let super::content::HeroImageState::Ready { cache_key, .. } =
                            &hero.facts.artwork.image
                        {
                            inline_hero_image = Some(PanelHeroImagePaint {
                                area: image_box,
                                cache_key: cache_key.clone(),
                                centered: false,
                            });
                        }
                    }
                    inline_hero = Some(block);
                }
            }
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

    NarrowSkeletonGeometry {
        selector_bar: areas.pills_area,
        controls: controls_area,
        list_area,
        inline_hero,
        inline_hero_image,
        selected: inline_hero.or(match &mut content.list {
            ListSlot::Media(list) => list.selected_row_rect(),
            _ => None,
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod narrow_skeleton_tests {
    use super::*;
    use crate::app::components::library_panel::{
        ArtworkShape, HeroContent, HeroFacts, ListControls, SelectorRow,
    };
    use crate::app::components::media_list::{
        MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation,
    };
    use crate::app::palette;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Narrow terminal: below `TWO_COLUMN_THRESHOLD` (82) on width.
    const WIDTH: u16 = 60;
    const HEIGHT: u16 = 30;

    fn item(target: &str) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: target.into(),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    fn facts(shape: ArtworkShape) -> HeroFacts {
        HeroFacts {
            title: "Dune".into(),
            meta_rows: vec!["2021".into(), "Science Fiction".into()],
            artwork: crate::app::components::library_panel::HeroArtwork {
                shape,
                source: None,
                image: crate::app::components::library_panel::content::HeroImageState::None,
            },
        }
    }

    fn draw_narrow(
        shape: ArtworkShape,
        overview: Option<&str>,
    ) -> (ratatui::buffer::Buffer, NarrowSkeletonGeometry) {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
        let mut content = LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut carrier),
            hero: Some(HeroContent {
                facts: facts(shape),
                overview: overview.map(Into::into),
                workspace: None,
            }),
        };
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
        let area = Rect::new(0, 0, WIDTH, HEIGHT);
        let mut hits = super::super::wide::SkeletonHits::default();
        let mut geometry = None;
        terminal
            .draw(|f| {
                geometry = Some(render_narrow_skeleton(
                    f,
                    area,
                    &mut content,
                    false,
                    &mut hits,
                ));
            })
            .unwrap();
        (
            terminal.backend().buffer().clone(),
            geometry.expect("narrow skeleton painted"),
        )
    }

    /// The painted image box for one plan inside an admitted hero block.
    fn image_box(block: Rect, plan: &InlineHeroPlan) -> Rect {
        Rect {
            x: block
                .right()
                .saturating_sub(SELECTED_BLOCK_SIDE_PADDING)
                .saturating_sub(plan.image_cols),
            y: block.y.saturating_add(2),
            width: plan.image_cols,
            height: plan.image_rows,
        }
    }

    fn line_text(buf: &ratatui::buffer::Buffer, row: u16, area: Rect) -> String {
        (area.left()..area.right())
            .map(|x| buf[(x, row)].symbol())
            .collect::<String>()
    }

    fn placeholder_fill() -> ratatui::style::Color {
        palette::surface_colors(palette::Surface::ArtworkPlaceholder, false).fill
    }

    /// The portrait arm: the artwork box hugs the block's right content
    /// edge, the title and meta rows paint beside it (left of the box), and
    /// the overview reclaims the full width below the image (design D7).
    /// The overview is long enough to outlive the ten-row image cap, so the
    /// continuation rows genuinely land below the box.
    #[test]
    fn portrait_hero_paints_right_aligned_wrap_around_text() {
        let overview = "A sprawling desert epic across many years of war. ".repeat(6);
        let (buf, geo) = draw_narrow(ArtworkShape::Portrait, Some(overview.as_str()));
        let block = geo.inline_hero.expect("the inline hero block was admitted");
        let plan = inline_hero_plan(
            block.width,
            &HeroContent {
                facts: facts(ArtworkShape::Portrait),
                overview: Some(overview.clone()),
                workspace: None,
            },
        );
        assert!(plan.image_cols > 0 && plan.image_rows > 0);

        // The image box is right-aligned and carries the shared placeholder
        // at its full box size.
        let image = image_box(block, &plan);
        assert_eq!(
            buf[(image.right() - 1, image.y + 1)].bg,
            placeholder_fill(),
            "the portrait image box paints the shared placeholder"
        );
        assert_eq!(
            buf[(image.x + 1, image.bottom() - 1)].bg,
            placeholder_fill()
        );

        // Wrap-around: the title paints BESIDE the image (left of the box,
        // within the box's row span), never inside it.
        let title_row = (block.y..image.bottom())
            .find(|&row| line_text(&buf, row, block).contains("Dune"))
            .expect("the title paints inside the block beside the image");
        let beside = Rect {
            x: block.x,
            width: image.x.saturating_sub(block.x),
            ..block
        };
        assert!(
            line_text(&buf, title_row, beside).contains("Dune"),
            "the title sits left of the right-aligned image"
        );
        let inside =
            (image.y..image.bottom()).any(|row| line_text(&buf, row, image).contains("Dune"));
        assert!(!inside, "no title inside the artwork box");

        // The overview wraps around the image and reclaims the full width
        // below it: some row at or below the image's bottom edge paints
        // overview text starting at the block's left content edge.
        let below = (image.bottom()..block.bottom())
            .filter(|&row| !line_text(&buf, row, block).trim().is_empty())
            .count();
        assert!(below > 0, "text continues below the image at full width");
        assert!(
            (image.bottom()..block.bottom())
                .any(|row| line_text(&buf, row, block).contains("desert")
                    || line_text(&buf, row, block).contains("years")),
            "the overview wraps to full width below the image"
        );

        // No selector, controls or constituent rows inside the hero block.
        for row in block.y..block.bottom() {
            let text = line_text(&buf, row, block);
            assert!(
                !text.contains("alpha") && !text.contains("beta"),
                "no constituent rows inside the inline hero"
            );
        }
    }

    /// The landscape arm: the same right-aligned wrap-around form, with the
    /// wide artwork box sized from its aspect (wider than tall).
    #[test]
    fn landscape_hero_paints_right_aligned_wrap_around_text() {
        let (buf, geo) = draw_narrow(ArtworkShape::Landscape, None);
        let block = geo.inline_hero.expect("the inline hero block was admitted");
        let plan = inline_hero_plan(
            block.width,
            &HeroContent {
                facts: facts(ArtworkShape::Landscape),
                overview: None,
                workspace: None,
            },
        );
        assert!(
            plan.image_cols > plan.image_rows,
            "a landscape box is wider than tall: {:?}",
            plan
        );
        let image = image_box(block, &plan);
        assert_eq!(
            buf[(image.right() - 1, image.y + 1)].bg,
            placeholder_fill(),
            "the landscape image box paints the shared placeholder"
        );
        let title_row = (block.y..image.bottom())
            .find(|&row| line_text(&buf, row, block).contains("Dune"))
            .expect("title paints");
        let beside = Rect {
            x: block.x,
            width: image.x.saturating_sub(block.x),
            ..block
        };
        assert!(
            line_text(&buf, title_row, beside).contains("Dune"),
            "the title paints beside (left of) the right-aligned image"
        );
    }

    /// No image source: the policy shape's box still reserves its size and
    /// paints the shared placeholder, with the same wrap-around text form.
    #[test]
    fn imageless_hero_keeps_the_box_and_wraps_around_it() {
        let (buf, geo) = draw_narrow(ArtworkShape::Portrait, None);
        let block = geo.inline_hero.expect("the inline hero block was admitted");
        assert!(
            (block.y..block.bottom()).any(|row| {
                (block.left()..block.right()).any(|x| buf[(x, row)].bg == placeholder_fill())
            }),
            "the placeholder paints at the policy shape's box"
        );
        assert!(
            (block.y..block.bottom()).any(|row| line_text(&buf, row, block).contains("2021")),
            "the meta rows paint beside the empty box"
        );
    }

    /// Wide and Narrow render the same title, meta rows and image for one
    /// `HeroContent` (task 5.7): the same title and meta texts appear in
    /// both, the meta rows use the same role colours, and both image boxes
    /// paint the same shared placeholder surface.
    #[test]
    fn wide_and_narrow_render_the_same_title_meta_and_image() {
        let (narrow_buf, geo) = draw_narrow(ArtworkShape::Portrait, None);
        let block = geo.inline_hero.expect("inline hero admitted");
        let hero = HeroContent {
            facts: facts(ArtworkShape::Portrait),
            overview: None,
            workspace: None,
        };

        // The Wide arm: the same HeroContent through the Wide header painter.
        // A short pane keeps the Portrait box small, so the meta rows wrap
        // within the text block the way the Narrow beside-image rows do.
        let mut terminal = Terminal::new(TestBackend::new(70, 14)).unwrap();
        let pane = Rect::new(0, 0, 70, 14);
        terminal
            .draw(|f| {
                super::super::hero_header::paint_hero_pane_content(f, pane, &hero);
            })
            .unwrap();
        let wide_buf = terminal.backend().buffer().clone();
        let wide_line = |row: u16| {
            (0..70)
                .map(|x| wide_buf[(x, row)].symbol())
                .collect::<String>()
        };
        let wide_all = |needle: &str| (0..14).any(|row| wide_line(row).contains(needle));
        let narrow_all = |needle: &str| {
            (block.y..block.bottom()).any(|row| line_text(&narrow_buf, row, block).contains(needle))
        };

        // Same title, same meta rows.
        for needle in ["Dune", "2021", "Science Fiction"] {
            assert!(wide_all(needle), "wide paints {needle}");
            assert!(narrow_all(needle), "narrow paints {needle}");
        }

        // Same image: both artwork boxes paint the shared placeholder.
        assert!(
            (0..14).any(|row| (0..70).any(|x| wide_buf[(x, row)].bg == placeholder_fill())),
            "wide paints the shared placeholder"
        );
        assert!(
            (block.y..block.bottom()).any(|row| {
                (block.left()..block.right()).any(|x| narrow_buf[(x, row)].bg == placeholder_fill())
            }),
            "narrow paints the same shared placeholder"
        );

        // Same meta colouring: the first meta row uses the first role in
        // both presentations.
        let wide_meta_row = (0..14)
            .find(|&row| wide_line(row).contains("2021"))
            .expect("wide meta row");
        let narrow_meta_row = (block.y..block.bottom())
            .find(|&row| line_text(&narrow_buf, row, block).contains("2021"))
            .expect("narrow meta row");
        let wide_x = (0..70)
            .find(|&x| wide_buf[(x, wide_meta_row)].symbol() == "2")
            .unwrap();
        let narrow_x = (block.left()..block.right())
            .find(|&x| narrow_buf[(x, narrow_meta_row)].symbol() == "2")
            .unwrap();
        assert_eq!(
            wide_buf[(wide_x, wide_meta_row)].style().fg,
            narrow_buf[(narrow_x, narrow_meta_row)].style().fg,
            "the first meta row uses the same role colour in Wide and Narrow"
        );
        assert_eq!(
            narrow_buf[(narrow_x, narrow_meta_row)].style().fg,
            Some(palette::HERO_META_ROLES[0]),
            "the first meta row uses the first of the three meta roles"
        );
    }

    /// The Narrow skeleton's rows: the Selector row (bar + spacer), an
    /// optional List controls row, and the list box carrying the Inline
    /// presentation.
    #[test]
    fn narrow_skeleton_places_selector_controls_and_list() {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![item("alpha"), item("beta")]);
        let mut content = LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into()],
                active: Some(0),
            }),
            controls: Some(ListControls {
                pills: None,
                label: Some("12 items".into()),
            }),
            list: ListSlot::Media(&mut carrier),
            hero: None,
        };
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
        let area = Rect::new(0, 0, WIDTH, HEIGHT);
        let mut hits = super::super::wide::SkeletonHits::default();
        let mut geometry = None;
        terminal
            .draw(|f| {
                geometry = Some(render_narrow_skeleton(
                    f,
                    area,
                    &mut content,
                    false,
                    &mut hits,
                ));
            })
            .unwrap();
        let geo = geometry.unwrap();
        let buf = terminal.backend().buffer();
        // The Selector row's bar sits at the panel's top; the pill paints.
        assert!(line_text(buf, geo.selector_bar.y, geo.selector_bar).contains("All"));
        // The controls row carries the label; the list box starts below it.
        let controls = geo.controls.expect("controls row reserved");
        assert!(line_text(buf, controls.y, controls).contains("12 items"));
        assert_eq!(geo.list_area.y, controls.bottom());
        // The Inline presentation painted the rows into the list box.
        assert!(line_text(buf, geo.list_area.y, geo.list_area).contains("alpha"));
        // No hero: no inline hero block reserved.
        assert!(geo.inline_hero.is_none());
    }
}
