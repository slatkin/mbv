//! The `Hero` component (design.md "Component catalogue"): the reserved
//! panel that shows the selected item's artwork, metadata and overview
//! above (legacy top placement), beside (hero-on-left), or inline with its list.
//!
//! This file holds the legacy top placement geometry and outer shell, extracted from
//! movies/TV (`list.rs`'s former `placement-neutral geometry` path, legacy top placement's
//! source per design.md decision 4): the fixed-height block reservation
//! (`placement-neutral geometry`) and its `▁`/`▔` bordered shell (`selected_detail_shell`),
//! shared today by every legacy top placement content kind (movie, series, album)
//! that `list.rs` paints into the block this module reserves.
//!
//! Grouped Music's hero-on-left arrangement (`music_wide.rs`, hero-on-left's
//! source per design.md decision 4) supplies this module's hero-on-left
//! geometry (`hero_on_left_panes`, `hero_on_left_right_pane`) and text paint
//! (`paint_hero_on_left_text`), extracted here in phase 5 ("Assemble
//! hero-on-left") so future hero-on-left screens (Home, audiobooks — phase
//! 6) share them rather than re-deriving their own. `compute_wide_left_layout`
//! itself (the hero/track vertical split and artwork sizing) stays in
//! `music_wide.rs`: its constants (`PANE_PAD_X`, `PANE_PAD_Y`, ...) are
//! shared with that file's non-hero track-list panel, so splitting it out
//! would scatter one padding convention across two files for no consumer
//! that exists yet.

use super::super::ui_util::trunc_str;
use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Height reserved for the hero panel while it has no content to size to.
/// A letter-pill switch clears the slice (so the selected item disappears)
/// before the new one loads in; without a reserved slot the panel collapses
/// to zero rows and the whole list jumps up each switch. The placeholder is
/// a minimum stand-in only -- the panel grows to fit its content once a
/// Movie/Series is actually selected.
pub(super) const HERO_PLACEHOLDER_ROWS: u16 = 18;
/// Row budget for the selected item's title on the hero's top row, rendered
/// in yellow. Reserved only in two-column lists (`show_title`), where the
/// list row's own title is truncated to a narrow cell; one-column lists
/// skip it since the full-width row title right above the hero already shows
/// the name.
pub(super) const HERO_TITLE_ROWS: u16 = 1;
/// Rows the hero *block* adds beyond the content rows, matching the
/// selected-block look of music/homevideo: a `▁` top border row and a `▔`
/// bottom border row (painted in `palette::SEEK_TRACK`) plus one bare
/// colored-bg padding row just inside each border. The borders are part of
/// the hero block's reserved rows (the list makes room), not painted over
/// list content like `render_selected_block_borders` does.
pub(super) const HERO_BLOCK_EXTRA_ROWS: u16 = 4;
/// The shared display-flow accounting for an inline selected-detail block.
/// `offset` is the first display row in the viewport and
/// `detail_screen_row` is the detail block's row relative to that viewport.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct InlineDetailFlow {
    pub offset: usize,
    pub detail_screen_row: usize,
}

pub(super) fn inline_detail_flow(
    cursor_row: usize,
    detail_rows: u16,
    visible_rows: u16,
    stored_offset: usize,
) -> Option<InlineDetailFlow> {
    let detail_rows = detail_rows as usize;
    let visible_rows = visible_rows as usize;
    if detail_rows == 0 || detail_rows + 1 > visible_rows {
        return None;
    }

    let lower_bound = (cursor_row + detail_rows + 1)
        .saturating_sub(visible_rows)
        .min(cursor_row);
    let offset = stored_offset.clamp(lower_bound, cursor_row);
    Some(InlineDetailFlow {
        offset,
        detail_screen_row: cursor_row + 1 - offset,
    })
}

/// Paints the hero block's outer shell -- the colored bg (focused/unfocused
/// pattern) plus the `▁` top and `▔` bottom borders in SEEK_TRACK on the
/// block's outer-row one -- shared by the normal hero path and the empty
/// "placeholder panel" path (slice loading after a pill switch) so the block
/// is always drawn identically while it's reserved.
///
/// This is the `HeroShell` component's non-scrolled entry point: a thin
/// wrapper over `render_selected_block_background`/`render_selected_block_borders`
/// (the same scroll-aware pair used by album/queue/home-video lists) with the
/// hero's own fixed window (`offset = 0`, fully visible, padding rows
/// `[1, hero_rows - 2]`), so there is exactly one implementation of the ▁/▔
/// shell rather than two near-identical ones.
pub(super) fn selected_detail_shell(f: &mut Frame, hero_area: Rect, hero_rows: u16, focused: bool) {
    let bg = palette::resolve_surface_focus(focused);
    let visible = hero_rows as usize;
    let top_pad_abs = 1usize;
    let bottom_pad_abs = (hero_rows as usize).saturating_sub(2);
    super::render_selected_block_background(
        f,
        hero_area,
        0,
        visible,
        top_pad_abs,
        bottom_pad_abs,
        bg,
    );
    super::render_selected_block_borders(
        f,
        hero_area,
        0,
        visible,
        top_pad_abs,
        bottom_pad_abs,
        super::SelectedBlockBorderStyle::Framed,
    );
}

/// Paints the selected item's name on the hero's top row (two-column lists
/// only, when `show_title` is set at the call site): yellow, bold when
/// focused. Shared by the movie hero (`detail.rs`'s `render_compact_detail`,
/// via [`paint_hero_content`]) and the Series inline hero
/// (`detail_series_view.rs`'s `render_series_inline_detail`), which
/// otherwise duplicated this block with only the geometry differing.
/// Returns `row + 1` if the title was painted, else `row` unchanged, so
/// callers push subsequent content down by the result.
pub(super) fn render_hero_title_row(
    f: &mut Frame,
    x: u16,
    row: u16,
    max_y: u16,
    width: u16,
    name: &str,
    focused: bool,
) -> u16 {
    if row >= max_y {
        return row;
    }
    let title = trunc_str(name, width as usize);
    let title_style = if focused {
        Style::default()
            .fg(palette::YELLOW)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::YELLOW)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(title, title_style))),
        Rect {
            x,
            y: row,
            width,
            height: 1,
        },
    );
    row + 1
}

/// One line of the `Hero` component's overview/detail block. `Plain` uses
/// the block's default text colour (focus-derived); `Prefixed` renders a
/// bold-styled label span before the truncated value -- the movie hero's
/// "Director: <name>" line is the only user today.
pub(super) enum HeroLine {
    Plain(String),
    Prefixed { label: &'static str, value: String },
}

/// Where the `Hero` component's right-aligned image starts, relative to
/// `area`. The two legacy top placement content kinds place it differently: the
/// movie hero pins the image to `area`'s own top row, sharing that row with
/// the title when one is shown; the Series hero starts its image on the row
/// *after* the title, one row lower. This is an existing, preserved
/// difference between the two -- not something this component unifies.
pub(super) enum ImageTop {
    AreaTop,
    AfterTitle,
}

pub(super) struct HeroImage {
    pub actual_w: u16,
    pub height: u16,
    pub top: ImageTop,
}

/// Data the `Hero` component paints, already resolved by the screen: no
/// fetch, cache lookup, or other `App` state -- just the final strings and
/// dimensions to draw. The actual image bytes live behind
/// `App::cached_image_protocol_mut`, which this component has no access to;
/// callers render the image (or its placeholder block) into
/// `HeroPaintResult::img_rect` themselves, immediately after calling
/// [`paint_hero_content`].
pub(super) struct HeroContent<'a> {
    pub title: Option<&'a str>,
    pub meta_line: Option<&'a str>,
    /// Role colour for `meta_line` -- the one place today's two legacy top placement
    /// content kinds disagree (movie: `MUTED_GREEN`, series: `SUBTLE`), so
    /// it is the declaration's colour-variant field (design.md decision 6)
    /// rather than a value this component picks itself.
    pub meta_color: Color,
    pub show_playing: bool,
    /// The blank row after the meta line: the movie hero only reserves it
    /// when a meta line was actually shown; the Series hero reserves it
    /// unconditionally (a spacer before the overview even with no meta
    /// line). Preserved as a declared per-kind difference rather than
    /// unified, since unifying it would change one screen's row count.
    pub unconditional_spacer_after_meta: bool,
    pub lines: &'a [HeroLine],
    pub image: Option<HeroImage>,
}

pub(super) struct HeroPaintResult {
    /// First unpainted row after the title/meta/playing/overview block, for
    /// callers (the Series hero) that append more content below it.
    pub next_row: u16,
    pub img_rect: Option<Rect>,
}

/// Paints the `Hero` component's text content -- title row, meta line,
/// "Playing" indicator, and overview/detail lines wrapped around the
/// right-aligned image reservation -- into `area`. Extracted unchanged from
/// `detail.rs`'s `render_compact_detail` (the movie hero, legacy top placement's
/// source per design.md decision 4); `detail_series_view.rs`'s Series hero
/// shares the same shape and now calls this too.
pub(super) fn paint_hero_content(
    f: &mut Frame,
    area: Rect,
    content: &HeroContent,
    focused: bool,
) -> HeroPaintResult {
    if area.height == 0 || area.width < 3 {
        return HeroPaintResult {
            next_row: area.y,
            img_rect: None,
        };
    }

    let inner_x = area.x;
    let inner_w = area.width as usize;
    let inner_w16 = area.width;
    let mut row = area.y;
    let max_y = area.y + area.height;

    let text_color = if focused {
        palette::WHITE
    } else {
        palette::SUBTLE
    };

    if let Some(title) = content.title {
        row = render_hero_title_row(f, inner_x, row, max_y, inner_w16, title, focused);
    }

    let (img_actual_w, img_height, img_top_row) = match &content.image {
        Some(img) => {
            let top_row = match img.top {
                ImageTop::AreaTop => area.y.min(area.y + area.height.saturating_sub(1)),
                ImageTop::AfterTitle => row,
            };
            (img.actual_w, img.height, top_row)
        }
        None => (0, 0, area.y),
    };
    let img_x = area.x + area.width.saturating_sub(img_actual_w);
    let img_end_row = img_top_row + img_height;
    let img_rect = if img_height > 0 {
        Some(Rect {
            x: img_x,
            y: img_top_row,
            width: img_actual_w,
            height: img_height,
        })
    } else {
        None
    };

    let narrow_w = inner_w.saturating_sub(img_actual_w as usize);
    let narrow_w16 = inner_w16.saturating_sub(img_actual_w);
    let text_dims = |r: u16| -> (usize, u16) {
        if img_height > 0 && r >= img_top_row && r < img_end_row {
            (narrow_w, narrow_w16)
        } else {
            (inner_w, inner_w16)
        }
    };

    if let Some(meta) = content.meta_line {
        if row < max_y {
            let (tw, tw16) = text_dims(row);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(meta, tw),
                    Style::default().fg(content.meta_color),
                ))),
                Rect {
                    x: inner_x,
                    y: row,
                    width: tw16,
                    height: 1,
                },
            );
            row += 1;
        }
    }
    // Spacer row between metadata and description: reserved whenever a meta
    // line was shown, or unconditionally when the content declares it (see
    // `unconditional_spacer_after_meta`'s doc).
    if (content.meta_line.is_some() || content.unconditional_spacer_after_meta) && row < max_y {
        row += 1;
    }

    if content.show_playing && row < max_y {
        let (_tw, tw16) = text_dims(row);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Playing",
                Style::default()
                    .fg(palette::BG_GREEN)
                    .add_modifier(Modifier::BOLD),
            ))),
            Rect {
                x: inner_x,
                y: row,
                width: tw16,
                height: 1,
            },
        );
        row += 1;
    }

    for line in content.lines {
        if row >= max_y {
            break;
        }
        let (tw, tw16) = text_dims(row);
        match line {
            HeroLine::Prefixed { label, value } => {
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(*label, Style::default().fg(palette::MUTED_GREEN)),
                        Span::styled(trunc_str(value, tw), Style::default().fg(palette::TEXT)),
                    ])),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
            }
            HeroLine::Plain(text) => {
                if !text.is_empty() {
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            trunc_str(text, tw),
                            Style::default().fg(text_color),
                        ))),
                        Rect {
                            x: inner_x,
                            y: row,
                            width: tw16,
                            height: 1,
                        },
                    );
                }
            }
        }
        row += 1;
    }

    HeroPaintResult {
        next_row: row,
        img_rect,
    }
}

/// Renders Home's legacy top placement metadata shape -- wrapped yellow title,
/// optional green subtitle row, one meta line, a blank separator, then the
/// overview -- shared by the Keep Watching (Emby) hero and the generic
/// Audiobookshelf/Feeds hero, which otherwise duplicated this block
/// (including, at one point, an errant background box under the overview
/// that `paint_hero_content`'s movie/series heroes never had). No background
/// is painted here either: text sits directly on whatever the caller's shell
/// already painted, same as `paint_hero_content`.
///
/// Doesn't reuse `paint_hero_content` itself: that component's title is a
/// single truncated row and has no subtitle slot, while this shape wraps the
/// title across multiple lines and always reserves a show-name row below it.
///
/// `overview_lines` pairs each pre-wrapped line with whether it has wrapped
/// past a beside-the-text image and should render across `wide_area`'s full
/// width instead of `area`'s; callers with no such image pass `wide_area ==
/// area` (the `bool` is then irrelevant since both rects are identical).
///
/// `title_suffix` is drawn one space after the last title line, on the same
/// row (Emby's watch-state glyph — the only user today; other heroes pass
/// `None`).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_home_hero_meta_block(
    f: &mut Frame,
    area: Rect,
    wide_area: Rect,
    title_lines: &[String],
    subtitle: &str,
    title_suffix: Option<Span<'static>>,
    meta_rows: Vec<Vec<Span<'static>>>,
    overview_lines: &[(String, bool)],
    overview_pad: u16,
    focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut row = area.y;
    let max_y = area.y + area.height;

    for (idx, line) in title_lines.iter().enumerate() {
        if row >= max_y {
            break;
        }
        let mut spans = vec![Span::styled(
            line.clone(),
            Style::default()
                .fg(palette::YELLOW)
                .add_modifier(Modifier::BOLD),
        )];
        if idx + 1 == title_lines.len() {
            if let Some(suffix) = &title_suffix {
                spans.push(Span::raw(" "));
                spans.push(suffix.clone());
            }
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: 1,
            },
        );
        row += 1;
    }

    if row < max_y && !subtitle.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                trunc_str(subtitle, area.width as usize),
                Style::default().fg(palette::FOAM),
            )),
            Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: 1,
            },
        );
        row += 1;
    }

    // One reserved row per meta row. `meta_rows` is empty only for heroes
    // with nothing to show; render each non-empty row.
    for meta_spans in meta_rows.iter() {
        if row >= max_y {
            break;
        }
        if !meta_spans.is_empty() {
            f.render_widget(
                Paragraph::new(Line::from(meta_spans.clone())),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
        }
        row += 1;
    }

    row += 1; // blank separator row

    // Hero-on-left (the recessed-box path): the box's own top padding row
    // doubles as the separator above, leaving the metadata flush against the
    // box's top edge. Reserve one more row so the gap above the box is
    // visible. The narrow legacy top placement path (overview_pad == 0) renders no
    // box and keeps its single separator row.
    if overview_pad > 0 && !overview_lines.is_empty() {
        row += 1;
    }

    if !overview_lines.is_empty() && row < max_y {
        let ov_color = if focused {
            palette::WHITE
        } else {
            palette::MUTED
        };
        // hero-on-left: paint recessed box behind overview.
        let recessed = if overview_pad > 0 {
            let ov_height = overview_lines.len().min((max_y - row) as usize) as u16 + 2; // top and bottom padding rows
            let ov_area = Rect {
                x: area.x,
                y: row.saturating_sub(1), // start 1 row earlier for top padding
                width: area.width,
                height: ov_height,
            };
            Some(super::hero_left::hero_on_left_recessed_box(
                f,
                ov_area,
                overview_pad,
                1,
            ))
        } else {
            None
        };
        for (line, wide) in overview_lines {
            if row >= max_y {
                break;
            }
            let base = if *wide { wide_area } else { area };
            let text_r = match &recessed {
                Some((_, content)) => Rect {
                    x: content.x,
                    y: row,
                    width: content.width,
                    height: 1,
                },
                None => Rect {
                    x: base.x + overview_pad,
                    y: row,
                    width: base.width.saturating_sub(overview_pad * 2),
                    height: 1,
                },
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(ov_color),
                ))),
                text_r,
            );
            row += 1;
        }
    }
}

// ── Hero-on-left arrangement ────────────────────────────────────────────
// Geometry, borders, recessed content blocks, and wrapped-text rendering
// live in `hero_left.rs` to keep this file under the 800-line cap.

/// Renders the fuzzy-search input (query text plus a `[loading…]` suffix
/// while a search is in flight) into `area`. A single-row control that
/// resumes the pill bar row it replaces: same `PILL_ROW_BG` background and
/// leading `⌘` glyph as `render_pill_bar`'s prefix, so swapping between pills
/// and search doesn't shift the row's look, just its content.
pub(super) fn render_search_box(f: &mut Frame, area: Rect, query: &str, loading: bool) {
    use ratatui::widgets::Block;

    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(
        Block::default().style(Style::default().bg(palette::PILL_ROW_BG)),
        area,
    );
    let bg = Style::default().bg(palette::PILL_ROW_BG);
    let input_text = if loading {
        format!("{query}█ [loading…]")
    } else {
        format!("{query}█")
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" \u{2318} ", bg.fg(palette::FOAM)),
            Span::styled(
                "SEARCH: ",
                bg.fg(palette::FOAM).add_modifier(Modifier::BOLD),
            ),
            Span::styled(input_text, bg.fg(palette::SOFT_WHITE)),
        ])),
        area,
    );
}
