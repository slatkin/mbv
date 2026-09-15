use super::*;
use crate::app::components::library_panel::narrow::InlineHeroPlan;
use crate::app::components::library_panel::{
    inline_hero_plan, render_narrow_skeleton, HeroCredit, LibraryPanelContent, ListSlot,
};
use crate::app::render::arrangements::library::selected_detail_content_area;
use crate::app::render::{HERO_BLOCK_EXTRA_ROWS, SELECTED_BLOCK_SIDE_PADDING};
use ratatui::layout::Rect;

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
        secondary: None,
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
        links: Vec::new(),
        artwork: crate::app::components::library_panel::HeroArtwork {
            shape,
            source: None,
            decoration: None,
            image: crate::app::components::library_panel::content::HeroImageState::None,
        },
    }
}

#[test]
fn inline_hero_plan_ignores_wide_only_credits() {
    let plain = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("Overview".into()),
        credits: None,
        workspace: None,
    };
    let with_credits = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: Some("Overview".into()),
        credits: Some(vec![HeroCredit {
            name: "Director".into(),
            role: "Director".into(),
        }]),
        workspace: None,
    };
    assert_eq!(
        inline_hero_plan(60, &plain),
        inline_hero_plan(60, &with_credits)
    );
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
            credits: None,
            workspace: None,
        }),
    };
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    let area = Rect::new(0, 0, WIDTH, HEIGHT);
    let mut hits = super::super::wide::SkeletonHits::default();
    let mut windows = super::super::wide::SkeletonPillWindows::default();
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = Some(render_narrow_skeleton(
                f,
                area,
                &mut content,
                false,
                None,
                &mut hits,
                &mut windows,
            ));
        })
        .unwrap();
    (
        terminal.backend().buffer().clone(),
        geometry.expect("narrow skeleton painted"),
    )
}

/// The painted image box for one plan inside an admitted hero block, derived
/// from the same content area the painter uses so the two cannot drift.
fn image_box(block: Rect, plan: &InlineHeroPlan) -> Rect {
    let content =
        selected_detail_content_area(block, SELECTED_BLOCK_SIDE_PADDING, HERO_BLOCK_EXTRA_ROWS);
    Rect {
        x: content.right().saturating_sub(plan.image_cols),
        y: content.y,
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
            credits: None,
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
    let inside = (image.y..image.bottom()).any(|row| line_text(&buf, row, image).contains("Dune"));
    assert!(!inside, "no title inside the artwork box");

    // The overview wraps around the image and reclaims the full width
    // below it: some row at or below the image's bottom edge paints
    // overview text starting at the block's left content edge.
    let below = (image.bottom()..block.bottom())
        .filter(|&row| !line_text(&buf, row, block).trim().is_empty())
        .count();
    assert!(below > 0, "text continues below the image at full width");
    assert!(
        (image.bottom()..block.bottom()).any(|row| line_text(&buf, row, block).contains("desert")
            || line_text(&buf, row, block).contains("years")),
        "the overview wraps to full width below the image"
    );
    // Soft white at all times: the overview is body content, never muted.
    let overview_row = (image.bottom()..block.bottom())
        .find(|&row| {
            let text = line_text(&buf, row, block);
            text.contains("desert") || text.contains("years")
        })
        .expect("an overview row below the image");
    let first = (block.left()..block.right())
        .find(|&x| !buf[(x, overview_row)].symbol().trim().is_empty())
        .expect("the overview row paints text");
    assert_eq!(
        buf[(first, overview_row)].style().fg,
        Some(palette::TEXT_EMPHASIS),
        "narrow overview text is soft white"
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
            credits: None,
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
        credits: None,
        workspace: None,
    };

    // The Wide arm: the same HeroContent through the Wide header painter.
    // A short pane keeps the Portrait box small, so the meta rows wrap
    // within the text block the way the Narrow beside-image rows do.
    let mut terminal = Terminal::new(TestBackend::new(70, 14)).unwrap();
    let pane = Rect::new(0, 0, 70, 14);
    terminal
        .draw(|f| {
            super::super::hero_header::paint_hero_pane_content(
                f,
                pane,
                &hero,
                0,
                None,
                &mut crate::app::components::mouse::hit::HitRegions::new(),
            );
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
            label: "12 items".into(),
        }),
        list: ListSlot::Media(&mut carrier),
        hero: None,
    };
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    let area = Rect::new(0, 0, WIDTH, HEIGHT);
    let mut hits = super::super::wide::SkeletonHits::default();
    let mut windows = super::super::wide::SkeletonPillWindows::default();
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = Some(render_narrow_skeleton(
                f,
                area,
                &mut content,
                false,
                None,
                &mut hits,
                &mut windows,
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
