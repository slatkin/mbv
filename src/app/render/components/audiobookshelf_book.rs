use crate::app::palette;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::audiobookshelf_books::BookHeroPlan;
use crate::app::render::components::hero::{
    inline_detail_flow, inline_display_row, inline_display_row_count, paint_hero_content,
    selected_detail_shell, wrap_overview_lines, HeroContent, HeroImage, HeroLine, InlineDisplayRow,
    HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::render::{render_pill_bar, render_placeholder, PillBar};
use crate::app::types_audiobookshelf_browse::{AudiobookshelfBookBrowseState, BookRow};
use crate::app::ui_util::{fmt_duration_approx, trunc_str};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

#[derive(Default)]
pub(in crate::app) struct AudiobookshelfBookGeometry {
    pub selector_tabs: Vec<(Rect, usize)>,
    pub book_rows: Vec<(Rect, usize)>,
    pub chapter_rows: Vec<(Rect, usize)>,
    /// Painted book-list rect: the wide right-pane browser, or the narrow
    /// content area below the pill bar. Mirrors the legacy
    /// `LayoutMain.left_area` so `lib_page_size()` regains its real stride
    /// after render ownership moved to the component (2.1j).
    pub left_area: Rect,
    /// Whether the last painted presentation is the wide hero-on-left
    /// layout (mirrors the legacy wide/narrow gate; drives
    /// `is_wide_book_active()` and the Enter activate decision).
    pub wide: bool,
    /// Hero rect the component painted for the selected book (wide left pane,
    /// or narrow inline-detail flow). Mirrors the legacy `LayoutMain.hero_area`
    /// so conformance/context-menu readers keep working after render ownership
    /// moved to the component (task 5.3d.13).
    pub hero_area: Option<Rect>,
    /// Selected-item rect the component painted (the hero when one is shown, or
    /// the selected book row otherwise). Mirrors the legacy
    /// `LayoutMain.selected_item_rect`.
    pub selected_item_rect: Option<Rect>,
}

pub(in crate::app) fn render_audiobookshelf_book_content(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    state: &mut AudiobookshelfBookBrowseState,
    images_enabled: bool,
    geometry: &mut AudiobookshelfBookGeometry,
    browser_offset: &mut usize,
) -> Option<super::home_hero::HomeImagePaint> {
    *geometry = AudiobookshelfBookGeometry::default();
    if state.books.is_empty() {
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
    if hero_left::shared_hero_presentation(area).is_some() {
        let panes = library_arrangement::wide_library_panes(area, 0, PANE_PAD_Y)?;
        let left_area = panes.left_area;
        geometry.left_area = panes.left_area;
        geometry.wide = true;
        let hero_content_area = padded_rect(left_area, PANE_PAD_X, 0);
        let hero_height = (plan.content_rows + 1).min(hero_content_area.height);
        let hero_area = Rect {
            height: hero_height,
            ..hero_content_area
        };
        geometry.hero_area = Some(hero_area);
        frame.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            Rect {
                x: panes.left_panel.x,
                y: panes.left_panel.bottom(),
                width: panes.left_panel.width,
                height: 1,
            },
        );
        frame.render_widget(
            Block::default().style(palette::resolve_surface_focus(
                focused && state.chapter_selection.is_some(),
            )),
            panes.left_panel,
        );
        let image = render_book_hero(
            frame,
            hero_area,
            state,
            focused && state.chapter_selection.is_some(),
            true,
            &plan,
        );
        let chapters_area = Rect {
            y: hero_content_area.y + hero_height,
            height: hero_content_area.height.saturating_sub(hero_height),
            ..hero_content_area
        };
        render_book_rows(
            frame,
            chapters_area,
            state,
            focused && state.chapter_selection.is_some(),
            geometry,
        );
        frame.render_widget(
            Block::default().style(palette::resolve_surface_focus(
                focused && state.chapter_selection.is_none(),
            )),
            panes.right_panel,
        );
        let right_pane =
            hero_left::hero_on_left_right_pane(panes.right_panel, panes.right_area, PANE_PAD_Y);
        let pills = right_pane.pills_area;
        geometry.selector_tabs = render_book_pills(frame, pills, state);
        let browser = padded_rect(right_pane.list_panel, PANE_PAD_X, PANE_PAD_Y);
        let image = render_book_browser(
            frame,
            browser,
            state,
            browser_offset,
            focused && state.chapter_selection.is_none(),
            &plan,
            geometry,
        )
        .or(image);
        // In the wide layout the selected book's hero (left pane) is the
        // selected item; record it so conformance/context-menu readers see the
        // same `selected_item_rect` the legacy renderer published.
        geometry.selected_item_rect = Some(hero_area);
        return image;
    }

    let parts = hero_left::pill_bar_areas(area);
    geometry.left_area = parts.content_area;
    geometry.wide = false;
    geometry.selector_tabs = render_book_pills(frame, parts.pills_area, state);
    let plan = book_hero_plan(
        state,
        parts
            .content_area
            .width
            .saturating_sub(SELECTED_BLOCK_SIDE_PADDING * 2),
        images_enabled,
    );
    render_book_browser(
        frame,
        parts.content_area,
        state,
        browser_offset,
        focused,
        &plan,
        geometry,
    )
}

fn render_book_pills(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBookBrowseState,
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
            selected_pos: state.selected_bucket.min(labels.len().saturating_sub(1)),
            prefix: Some(" ⌘ "),
        },
    )
}

fn render_book_browser(
    frame: &mut Frame,
    area: Rect,
    state: &mut AudiobookshelfBookBrowseState,
    browser_offset: &mut usize,
    focused: bool,
    plan: &BookHeroPlan,
    geometry: &mut AudiobookshelfBookGeometry,
) -> Option<super::home_hero::HomeImagePaint> {
    let Some(bucket) = state.buckets.get(state.selected_bucket).copied() else {
        render_placeholder(frame, area, " (empty)");
        return None;
    };
    if bucket.end <= bucket.start || area.height == 0 {
        render_placeholder(frame, area, " (empty)");
        return None;
    }
    let cursor = state.cursor();
    let count = bucket.end - bucket.start;
    let cursor_pos = cursor.saturating_sub(bucket.start).min(count - 1);
    let detail_rows = plan.content_rows + HERO_BLOCK_EXTRA_ROWS;
    let flow = inline_detail_flow(cursor_pos, detail_rows, area.height, *browser_offset);
    let (detail_rows, offset) = match flow {
        Some(flow) => (detail_rows, flow.offset),
        None => (
            0,
            (*browser_offset).min(count.saturating_sub(area.height as usize)),
        ),
    };
    *browser_offset = offset;
    let total_display = inline_display_row_count(count, cursor_pos, detail_rows);
    let display_rows = (offset..total_display).take(area.height as usize);
    let mut image = None;
    for (screen_row, display_row) in display_rows.enumerate() {
        let Some(source) = inline_display_row(count, cursor_pos, detail_rows, display_row) else {
            continue;
        };
        let y = area.y + screen_row as u16;
        match source {
            InlineDisplayRow::Replacement => {
                let hero_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: detail_rows,
                };
                selected_detail_shell(frame, hero_area, detail_rows, focused);
                image = render_book_hero(frame, hero_area, state, focused, true, plan);
                geometry.hero_area = Some(hero_area);
                geometry.selected_item_rect = Some(hero_area);
                let content_height = detail_rows.saturating_sub(HERO_BLOCK_EXTRA_ROWS);
                render_book_rows(
                    frame,
                    Rect {
                        y: y + plan.content_rows.min(detail_rows),
                        height: content_height.saturating_sub(plan.content_rows),
                        ..hero_area
                    },
                    state,
                    focused,
                    geometry,
                );
                break;
            }
            InlineDisplayRow::Source(row) => {
                let book_idx = bucket.start + row;
                let row_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                };
                let selected = book_idx == cursor;
                if selected && focused {
                    frame.render_widget(
                        Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
                        row_area,
                    );
                }
                let marker = crate::app::render::selection_marker(
                    selected,
                    crate::app::render::MarkerEdge::Left,
                );
                let title = trunc_str(
                    &state.books[book_idx].title,
                    area.width.saturating_sub(2) as usize,
                );
                frame.render_widget(
                    ratatui::widgets::Paragraph::new(Line::from(vec![marker, Span::raw(title)])),
                    row_area,
                );
                geometry.book_rows.push((row_area, book_idx));
                if selected && geometry.selected_item_rect.is_none() {
                    geometry.selected_item_rect = Some(row_area);
                }
            }
        }
    }
    image
}

fn render_book_hero(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBookBrowseState,
    focused: bool,
    show_title: bool,
    plan: &BookHeroPlan,
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
    let lines = wrap_overview_lines(&overview, |line| {
        crate::app::render::components::hero::inline_hero_text_width(
            area.width,
            plan.image_width,
            plan.image_height,
            overview_start + line as u16,
        ) as usize
    });
    let mut hero_lines = Vec::new();
    if !author.is_empty() {
        hero_lines.push(HeroLine::Plain(author.into()));
    }
    hero_lines.extend(lines.into_iter().map(HeroLine::Plain));
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
            image: (plan.image_height > 0).then_some(HeroImage {
                actual_w: plan.image_width,
                height: plan.image_height,
            }),
        },
        focused,
    );
    (plan.image_key.is_some() && result.img_rect.is_some()).then(|| {
        super::home_hero::HomeImagePaint::AudiobookshelfCover {
            area: result.img_rect.unwrap(),
            library_item_id: book.library_item_id.clone(),
            show_placeholder: plan.placeholder,
        }
    })
}

fn render_book_rows(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBookBrowseState,
    focused: bool,
    geometry: &mut AudiobookshelfBookGeometry,
) {
    if area.height == 0 {
        return;
    }
    let Some(id) = state.selected_id.as_deref() else {
        return;
    };
    if state.detail_loading {
        render_placeholder(frame, area, " Loading…");
        return;
    }
    let rows = state.visible_rows(id);
    if rows.is_empty() {
        render_placeholder(frame, area, " No chapters available");
        return;
    }
    let show_length = area.width > 40;
    let duration_width = if show_length { 7 } else { 0 };
    let title_width =
        (area.width as usize).saturating_sub(1 + usize::from(show_length) * (duration_width + 1));
    let table_rows = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let selected = state.chapter_selection == Some(index);
            let style = if selected && focused {
                Style::default().fg(palette::TEXT_FOCUS_ACCENT)
            } else if focused {
                Style::default().fg(palette::TEXT_STRONG)
            } else {
                Style::default().fg(palette::TEXT_SECONDARY)
            };
            let marker = crate::app::render::selection_marker(
                selected,
                crate::app::render::MarkerEdge::Left,
            );
            let (title, duration) = match row {
                BookRow::Chapter { title, start, end } => (
                    title.clone(),
                    fmt_duration_approx((end - start).max(0.0) as i64),
                ),
                BookRow::AudioFile { index, duration } => (
                    format!("Part {index}"),
                    fmt_duration_approx(*duration as i64),
                ),
            };
            let title = Cell::from(Line::from(vec![
                marker,
                Span::raw(trunc_str(&title, title_width)),
            ]));
            if show_length {
                Row::new([title, Cell::from(duration), Cell::from("")]).style(style)
            } else {
                Row::new([title, Cell::from(""), Cell::from("")]).style(style)
            }
        })
        .collect::<Vec<_>>();
    let mut table_state = TableState::default();
    table_state.select(state.chapter_selection);
    frame.render_stateful_widget(
        Table::new(
            table_rows,
            [
                Constraint::Min(10),
                Constraint::Length(duration_width as u16),
                Constraint::Length(1),
            ],
        )
        .column_spacing(1),
        area,
        &mut table_state,
    );
    geometry.chapter_rows = rows
        .iter()
        .enumerate()
        .skip(table_state.offset())
        .take(area.height as usize)
        .enumerate()
        .map(|(screen, (index, _))| {
            (
                Rect {
                    y: area.y + screen as u16,
                    height: 1,
                    ..area
                },
                table_state.offset() + index,
            )
        })
        .collect();
}

fn book_hero_plan(
    state: &AudiobookshelfBookBrowseState,
    width: u16,
    images_enabled: bool,
) -> BookHeroPlan {
    let Some(book) = state.selected_book() else {
        return BookHeroPlan {
            image_key: None,
            image_width: 0,
            image_height: 0,
            placeholder: false,
            content_rows: HERO_TITLE_ROWS,
        };
    };
    let has_cover = images_enabled && book.cover_path.is_some();
    let image_width = has_cover.then_some(18).unwrap_or(0).min(width);
    let image_height = has_cover.then_some(12).unwrap_or(0);
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
        image_key: has_cover.then(|| book.library_item_id.clone()),
        image_width,
        image_height,
        placeholder: has_cover,
        content_rows: image_height
            .saturating_add(1)
            .max(HERO_TITLE_ROWS + 2 + author_rows + overview_rows),
    }
}
