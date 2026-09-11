use crate::app::components::media_list::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, MediaKind, MediaListRow, MediaSemanticState,
    SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
};
use crate::app::palette;
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::arrangements::wide_hero::{
    self, wide_hero_browser_border, wide_hero_browser_pane, PANE_PAD_X, PANE_PAD_Y,
};
use crate::app::render::components::audiobookshelf_books::BookHeroPlan;
use crate::app::render::components::detail_series_view::{SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS};
use crate::app::render::components::hero::{
    paint_hero_content, selected_detail_shell, wrap_overview_lines, HeroContent, HeroImage,
    HeroLine, HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::render::{render_pill_bar, render_placeholder, PillBar};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use crate::app::ui_util::fmt_duration_approx;
use ratatui::layout::Rect;
use ratatui::style::Style;

use ratatui::widgets::Block;
use ratatui::Frame;
use tuirealm::component::Component;

/// Narrow replacement keeps a bounded overview so chapter rows remain visible.
pub(in crate::app::render) const BOOK_WIDE_OVERVIEW_ROWS: u16 = 4;
pub(in crate::app::render) const BOOK_NARROW_OVERVIEW_ROWS: u16 = 4;

/// The component-owned interaction values the book renderer needs, passed in
/// rather than read off the projected content type
/// (split-browse-state-interaction-fields task 2.2). `chapter_focused` is the
/// parent-owned chapter-pane focus, separate from the chapter owner's selected
/// row (design.md D5).
#[derive(Clone, Copy)]
pub(in crate::app) struct BookInteraction {
    pub chapter_focused: bool,
    pub selected_bucket: usize,
}

/// The active book-row presentation the book destination paints this frame
/// (design.md D1/D2): the Wide presentation for the Wide hero rail or the
/// Inline presentation for inline Narrow. Exactly one is handed over per view;
/// the same shared `MediaList` owner moves between them.
pub(in crate::app) enum BookPresentation<'a> {
    Wide(&'a mut WideMediaList<String>),
    Inline(&'a mut InlineMediaBrowser<String>),
}

/// The active chapter/audio-part presentation. Chapter rows always paint fixed
/// one-column rows through the Wide presentation (design.md D1/D2), in the
/// wide hero and the narrow inline detail alike.
pub(in crate::app) enum BookChapterPresentation<'a> {
    Wide(&'a mut WideMediaList<usize>),
}

#[derive(Default)]
pub(in crate::app) struct AudiobookshelfBookGeometry {
    pub selector_tabs: Vec<(Rect, usize)>,
    /// Painted book-list rect: the Wide browser pane on the left, or the
    /// narrow content area below the pill bar. This is the list geometry used
    /// by `lib_page_size()` for its real stride (2.1j).
    pub left_area: Rect,
    /// Whether the last painted presentation uses the Wide hero layout. The
    /// Enter activation decision uses `App::is_right_panel_wide()` instead.
    pub wide: bool,
    /// Hero rect the component painted for the selected book (Wide right pane,
    /// or narrow inline-detail flow). This is the painted hero geometry used
    /// by conformance/context-menu readers (task 5.3d.13).
    pub hero_area: Option<Rect>,
    /// Selected-item rect the component painted (the hero when one is shown, or
    /// the selected book row otherwise).
    pub selected_item_rect: Option<Rect>,
}

/// Canonical row projection for the book catalog: one selectable `Item` per
/// book in the selected surname bucket, keyed by its stable `library_item_id`.
/// Books carry no in-list letter headings (the surname buckets are a pill row)
/// and no played/active semantic state (matching the legacy book rows).
pub(in crate::app) fn book_rows(
    state: &AudiobookshelfBookBrowseState,
    selected_bucket: usize,
) -> Vec<MediaListRow<String>> {
    let Some(bucket) = state.buckets.get(selected_bucket).copied() else {
        return Vec::new();
    };
    state
        .books
        .get(bucket.start..bucket.end)
        .unwrap_or_default()
        .iter()
        .map(|book| MediaListRow::Item {
            target: book.library_item_id.clone(),
            primary: book.title.clone(),
            trailing: None,
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn render_audiobookshelf_book_content(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    state: &AudiobookshelfBookBrowseState,
    interaction: BookInteraction,
    images_enabled: bool,
    geometry: &mut AudiobookshelfBookGeometry,
    book_presentation: BookPresentation<'_>,
    chapter_presentation: BookChapterPresentation<'_>,
    list_pane_width: Option<u16>,
) -> Option<super::home_hero::HomeImagePaint> {
    *geometry = AudiobookshelfBookGeometry::default();
    if state.books.is_empty() {
        match book_presentation {
            BookPresentation::Wide(book_list) => book_list.invalidate_paint(),
            BookPresentation::Inline(book_list) => book_list.invalidate_paint(),
        }
        render_placeholder(
            frame,
            area,
            state
                .error
                .as_deref()
                .unwrap_or(if state.loading_pages.is_empty() {
                    "No audiobooks"
                } else {
                    "Loading audiobooks…"
                }),
        );
        return None;
    }

    let plan = book_hero_plan(state, area.width, images_enabled);
    if wide_hero::wide_hero_fits(area) {
        let BookPresentation::Wide(book_list) = book_presentation else {
            return None;
        };
        let panes = library_arrangement::wide_library_panes(area, 0, PANE_PAD_Y, list_pane_width)?;
        geometry.left_area = panes.hero_area;
        geometry.wide = true;
        let hero_content_area = wide_hero::wide_hero_hero_pane(
            frame,
            area,
            wide_hero::LeftPaneFocus::Workspace(focused && interaction.chapter_focused),
            list_pane_width,
        )
        .expect("wide branch already confirmed wide_hero_presentation fits");
        let hero_height = (book_hero_content_rows(&plan, BOOK_WIDE_OVERVIEW_ROWS) + 1)
            .min(hero_content_area.height);
        let hero_area = Rect {
            height: hero_height,
            ..hero_content_area
        };
        geometry.hero_area = Some(hero_area);
        let image = render_book_hero(
            frame,
            hero_area,
            state,
            focused && interaction.chapter_focused,
            true,
            &plan,
            None,
        );
        let chapters_area = Rect {
            y: hero_content_area.y + hero_height,
            height: hero_content_area.height.saturating_sub(hero_height),
            ..hero_content_area
        };
        let (_, chapters_content_area) =
            wide_hero::wide_hero_hero_content_box(frame, chapters_area);
        render_book_rows(
            frame,
            chapters_content_area,
            state,
            focused && interaction.chapter_focused,
            chapter_presentation,
        );
        let rail_focused = focused && !interaction.chapter_focused;
        let right_pane = wide_hero_browser_pane(panes.browser_panel, panes.browser_area);
        geometry.selector_tabs = render_book_pills(
            frame,
            right_pane.pills_area,
            state,
            interaction.selected_bucket,
        );
        let list_panel = right_pane.list_panel;
        let content_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);
        if list_panel.height > 0 {
            frame.render_widget(
                Block::default().style(Style::default().bg(
                    palette::surface_colors(palette::Surface::LibraryPanel, rail_focused).fill,
                )),
                list_panel,
            );
        }
        // Paint the rail frame before the rows: the border primitive rewrites
        // every panel cell background, so it must not run after the list.
        wide_hero_browser_border(frame, list_panel, rail_focused);

        book_list.set_geometry(list_panel, content_area);
        book_list.set_paint_policy(WideMediaListPaintPolicy::new(
            rail_focused,
            SelectedRowSurface::ListBackdrop,
            None,
        ));
        Component::view(book_list, frame, content_area);
        // In the Wide layout the selected book's hero (right pane) is the
        // selected item; record it so conformance/context-menu readers see the
        // same `selected_item_rect` the legacy renderer published.
        geometry.selected_item_rect = Some(hero_area);
        return image;
    }

    render_narrow_book(
        frame,
        area,
        focused,
        state,
        interaction,
        images_enabled,
        geometry,
        book_presentation,
        chapter_presentation,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_narrow_book(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    state: &AudiobookshelfBookBrowseState,
    interaction: BookInteraction,
    images_enabled: bool,
    geometry: &mut AudiobookshelfBookGeometry,
    book_presentation: BookPresentation<'_>,
    chapter_presentation: BookChapterPresentation<'_>,
) -> Option<super::home_hero::HomeImagePaint> {
    let BookPresentation::Inline(book_list) = book_presentation else {
        return None;
    };
    let parts = wide_hero::pill_bar_areas(area);
    geometry.left_area = parts.content_area;
    geometry.wide = false;
    geometry.selector_tabs =
        render_book_pills(frame, parts.pills_area, state, interaction.selected_bucket);
    let content_area = parts.content_area;
    let plan = book_hero_plan(
        state,
        content_area
            .width
            .saturating_sub(SELECTED_BLOCK_SIDE_PADDING * 2),
        images_enabled,
    );

    book_list.set_geometry(content_area, content_area);
    let visible = content_area.height.max(1) as usize;
    let hero_rows = book_hero_content_rows(&plan, BOOK_NARROW_OVERVIEW_ROWS);
    let chapter_count = state
        .selected_id
        .as_deref()
        .map(|id| state.visible_rows(id).len())
        .unwrap_or(0);
    // Admit the detail block using only the chapter rows that can fit. The
    // chapter painter owns the remaining area and clips/scrolls the full list.
    let hero_block_base = hero_rows as usize + 1 + HERO_BLOCK_EXTRA_ROWS as usize;
    let chapter_budget = visible.saturating_sub(hero_block_base).max(1);
    let chapter_rows = chapter_count.min(chapter_budget);
    // Preserve the ordinary-row fallback when even the bounded hero shell
    // cannot fit; otherwise cap the projected chapter rows below the strict
    // inline admission boundary and let the chapter list scroll the rest.
    let desired_detail_rows = if hero_block_base >= content_area.height as usize {
        hero_block_base
    } else {
        (hero_block_base + chapter_rows).min(content_area.height.saturating_sub(1).max(1) as usize)
    };
    book_list.set_paint_policy(InlineMediaBrowserPaintPolicy::new(
        focused,
        SelectedRowSurface::ListBackdrop,
        desired_detail_rows,
    ));
    Component::view(book_list, frame, content_area);
    let selected_row_rect = book_list.current_selected_row_rect();
    let hero = book_list.current_detail_rect();

    let Some(hero_area) = hero else {
        // Ordinary-row fallback: no inline hero, no selected-item shell.
        geometry.selected_item_rect = selected_row_rect;
        return None;
    };
    selected_detail_shell(frame, hero_area, hero_area.height, focused);
    geometry.hero_area = Some(hero_area);
    geometry.selected_item_rect = Some(hero_area);
    let image = render_book_hero(
        frame,
        hero_area,
        state,
        focused,
        true,
        &plan,
        Some(BOOK_NARROW_OVERVIEW_ROWS as usize),
    );
    let chapter_area = Rect {
        y: hero_area.y + SELECTED_BLOCK_SIDE_PADDING + hero_rows + 1,
        height: hero_area
            .height
            .saturating_sub(SELECTED_BLOCK_SIDE_PADDING + hero_rows + 1),
        ..hero_area
    };
    render_book_rows(frame, chapter_area, state, false, chapter_presentation);
    image
}

fn render_book_pills(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBookBrowseState,
    selected_bucket: usize,
) -> Vec<(Rect, usize)> {
    if state.buckets.is_empty() || area.width == 0 {
        return Vec::new();
    }
    let labels: Vec<String> = state
        .buckets
        .iter()
        .map(|bucket| bucket.label.into())
        .collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    render_pill_bar(
        frame,
        area,
        PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: selected_bucket.min(labels.len().saturating_sub(1)),
            prefix: Some(" ⌘ "),
        },
    )
}

fn render_book_hero(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBookBrowseState,
    focused: bool,
    show_title: bool,
    plan: &BookHeroPlan,
    overview_limit: Option<usize>,
) -> Option<super::home_hero::HomeImagePaint> {
    let book = state.selected_book()?;
    let mut meta = Vec::new();
    if book.duration_seconds > 0.0 {
        meta.push(fmt_duration_approx(book.duration_seconds as i64));
    }
    if let Some(progress) = state.progress.get(&book.library_item_id) {
        meta.push(if progress.is_finished {
            "Finished".into()
        } else if progress.current_time_seconds > 0.0 && book.duration_seconds > 0.0 {
            format!(
                "{}%",
                ((progress.current_time_seconds * 100.0 / book.duration_seconds).floor() as u8)
                    .clamp(1, 99)
            )
        } else {
            "Not started".into()
        });
    }
    if let Some(narrator) = book.narrator.as_deref().filter(|value| !value.is_empty()) {
        meta.push(format!("Read by {narrator}"));
    }
    if let Some(year) = book
        .published_year
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        meta.push(year.into());
    }
    let overview = book
        .description
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(crate::app::ui_util::trunc_overview)
        .unwrap_or_default();
    let author = book.author_display.as_deref().unwrap_or("");
    let overview_start = HERO_TITLE_ROWS
        + usize::from(!author.is_empty()) as u16
        + usize::from(!meta.is_empty()) as u16 * 2;
    // HeroImage is right-aligned by the painter; keep its fixed cover width
    // separate from the full-width text area. The old slot calculation passed
    // the whole pane as the artwork width, leaving no room for text.
    let (image_width, image_height) = if plan.has_image {
        (plan.image_width, plan.image_height)
    } else {
        (0, 0)
    };
    let lines = wrap_overview_lines(&overview, |line| {
        crate::app::render::components::hero::inline_hero_text_width(
            area.width,
            image_width,
            image_height,
            overview_start + line as u16,
        ) as usize
    });
    let mut hero_lines = Vec::new();
    if !author.is_empty() {
        hero_lines.push(HeroLine::Plain(author.into()));
    }
    hero_lines.extend(
        lines
            .into_iter()
            .take(overview_limit.unwrap_or(usize::MAX))
            .map(HeroLine::Plain),
    );
    let result = paint_hero_content(
        frame,
        Rect {
            x: area.x + SELECTED_BLOCK_SIDE_PADDING,
            y: area.y + SELECTED_BLOCK_SIDE_PADDING,
            width: area.width.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
            height: area.height.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
        },
        &HeroContent {
            title: show_title.then_some(book.title.as_str()),
            meta_line: (!meta.is_empty()).then(|| meta.join("  ")).as_deref(),
            meta_color: palette::TEXT_DETAIL_META,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &hero_lines,
            image: (image_height > 0).then_some(HeroImage {
                actual_w: image_width,
                height: image_height,
            }),
        },
        focused,
    );
    (plan.has_image && result.img_rect.is_some()).then(|| {
        super::home_hero::HomeImagePaint::AudiobookshelfBookCover {
            area: result.img_rect.unwrap(),
            library_item_id: book.library_item_id.clone(),
            show_placeholder: true,
        }
    })
}

fn render_book_rows(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBookBrowseState,
    focused: bool,
    chapter_presentation: BookChapterPresentation<'_>,
) {
    let BookChapterPresentation::Wide(chapter_list) = chapter_presentation;
    if area.height == 0 {
        chapter_list.invalidate_paint();
        return;
    }
    let Some(id) = state.selected_id.as_deref() else {
        chapter_list.invalidate_paint();
        return;
    };
    if state.detail_loading {
        chapter_list.invalidate_paint();
        render_placeholder(frame, area, " Loading…");
        return;
    }
    if state.visible_rows(id).is_empty() {
        chapter_list.invalidate_paint();
        render_placeholder(frame, area, " No chapters available");
        return;
    }
    // The chapter owner already holds the projected rows (design.md D6); the
    // painter only paints and retains the current-frame hit geometry.
    chapter_list.set_geometry(area, area);
    chapter_list.set_paint_policy(WideMediaListPaintPolicy::new(
        focused,
        SelectedRowSurface::ListBackdrop,
        None,
    ));
    Component::view(chapter_list, frame, area);
}

fn book_hero_content_rows(plan: &BookHeroPlan, overview_limit: u16) -> u16 {
    plan.image_height
        .saturating_add(1)
        .max(HERO_TITLE_ROWS + 2 + plan.author_rows + plan.overview_rows.min(overview_limit))
}

fn book_hero_plan(
    state: &AudiobookshelfBookBrowseState,
    width: u16,
    images_enabled: bool,
) -> BookHeroPlan {
    let Some(book) = state.selected_book() else {
        return BookHeroPlan {
            has_image: false,
            image_width: 0,
            image_height: 0,
            author_rows: 0,
            overview_rows: 0,
        };
    };
    let has_cover = images_enabled && book.cover_path.is_some();
    let image_width = if has_cover { SERIES_IMAGE_COLS } else { 0 }.min(width);
    let image_height = if has_cover { SERIES_IMAGE_ROWS } else { 0 };
    let author_rows = u16::from(
        book.author_display
            .as_deref()
            .is_some_and(|value| !value.is_empty()),
    );
    let overview = book
        .description
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(crate::app::ui_util::trunc_overview)
        .unwrap_or_default();
    let overview_rows = wrap_overview_lines(&overview, |line| {
        crate::app::render::components::hero::inline_hero_text_width(
            width,
            image_width,
            image_height,
            HERO_TITLE_ROWS + 2 + author_rows + line as u16,
        ) as usize
    })
    .len() as u16;
    BookHeroPlan {
        author_rows,
        overview_rows,
        has_image: has_cover,
        image_width,
        image_height,
    }
}
