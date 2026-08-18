//! The `Hero` component (design.md "Component catalogue"): the reserved
//! panel that shows the selected item's artwork, metadata and overview
//! above (hero-on-top) or beside (hero-on-left) its list.
//!
//! This file holds the hero-on-top geometry and outer shell, extracted from
//! movies/TV (`list.rs`'s former `top_hero_layout` path, hero-on-top's
//! source per design.md decision 4): the fixed-height block reservation
//! (`top_hero_layout`) and its `▁`/`▔` bordered shell (`hero_block_shell`),
//! shared today by every hero-on-top content kind (movie, series, album)
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
use textwrap::wrap;

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
/// Blank row separating the hero block from the list below it.
const HERO_SEPARATOR_ROWS: u16 = 1;

pub(super) struct TopHeroLayout {
    pub hero_area: Rect,
    pub pills_area: Rect,
    pub list_area: Rect,
    pub hero_rows: u16,
}

pub(super) fn top_hero_layout(
    content_area: Rect,
    desired_hero_rows: u16,
    show_pills: bool,
) -> TopHeroLayout {
    let pills_reserved = if show_pills {
        2.min(content_area.height)
    } else {
        0
    };
    let separator_reserve = if show_pills { 0 } else { HERO_SEPARATOR_ROWS };
    let hero_rows = match desired_hero_rows.min(
        content_area
            .height
            .saturating_sub(1 + separator_reserve + pills_reserved),
    ) {
        r if r < HERO_BLOCK_EXTRA_ROWS => 0,
        r => r,
    };
    let separator_rows = if hero_rows > 0 { separator_reserve } else { 0 };
    let hero_shift = if hero_rows > 0 && content_area.y > 0 {
        1
    } else {
        0
    };
    let hero_area = Rect {
        y: content_area.y.saturating_sub(hero_shift),
        height: hero_rows,
        ..content_area
    };
    let pills_area = Rect {
        y: content_area.y.saturating_sub(hero_shift) + hero_rows + separator_rows,
        height: if show_pills { 1 } else { 0 },
        ..content_area
    };
    let list_area = Rect {
        y: content_area.y.saturating_sub(hero_shift) + hero_rows + separator_rows + pills_reserved,
        height: (content_area.height + hero_shift)
            .saturating_sub(hero_rows + separator_rows + pills_reserved),
        ..content_area
    };
    TopHeroLayout {
        hero_area,
        pills_area,
        list_area,
        hero_rows,
    }
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
pub(super) fn hero_block_shell(f: &mut Frame, hero_area: Rect, hero_rows: u16, focused: bool) {
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
        super::SelectedBlockBorderStyle::HeroOnTop,
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
/// `area`. The two hero-on-top content kinds place it differently: the
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
    /// Role colour for `meta_line` -- the one place today's two hero-on-top
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
/// `detail.rs`'s `render_compact_detail` (the movie hero, hero-on-top's
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

/// Renders Home's hero-on-top metadata shape -- wrapped yellow title,
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
#[allow(clippy::too_many_arguments)]
pub(super) fn render_home_hero_meta_block(
    f: &mut Frame,
    area: Rect,
    wide_area: Rect,
    title_lines: &[String],
    subtitle: &str,
    meta_spans: Vec<Span<'static>>,
    overview_lines: &[(String, bool)],
    overview_pad: u16,
    focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut row = area.y;
    let max_y = area.y + area.height;

    for line in title_lines {
        if row >= max_y {
            break;
        }
        f.render_widget(
            Paragraph::new(Span::styled(
                line.clone(),
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD),
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

    // Always reserves one row, even with nothing to show, so the overview
    // below lands on the same row whether or not this item has meta text.
    if row < max_y {
        if !meta_spans.is_empty() {
            f.render_widget(
                Paragraph::new(Line::from(meta_spans)),
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

    if !overview_lines.is_empty() && row < max_y {
        let ov_color = if focused {
            palette::WHITE
        } else {
            palette::MUTED
        };
        for (line, wide) in overview_lines {
            if row >= max_y {
                break;
            }
            let r = if *wide {
                Rect {
                    x: wide_area.x,
                    y: row,
                    width: wide_area.width,
                    height: 1,
                }
            } else {
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                }
            };
            let text_r = Rect {
                x: r.x + overview_pad,
                width: r.width.saturating_sub(overview_pad * 2),
                ..r
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

/// Minimum outer content-area height for the hero-on-left arrangement's
/// two-pane split; below this the caller falls back to the shared
/// hero-on-top narrow renderer (design.md decision 5's height floor). Moved
/// unchanged from grouped Music's former `MIN_WIDE_AREA_HEIGHT`.
pub(super) const HERO_ON_LEFT_MIN_AREA_HEIGHT: u16 = 6;
/// Minimum width either hero-on-left pane may shrink to (decision 5's
/// minimum pane width). Moved unchanged from grouped Music's former
/// `MIN_PANE_WIDTH`.
const HERO_ON_LEFT_MIN_PANE_WIDTH: u16 = 40;
/// Empty columns separating the hero-on-left arrangement's two panes.
const HERO_ON_LEFT_PANE_GAP: u16 = 2;
/// Height of the pill row at the top of the hero-on-left arrangement's right
/// (list) pane.
const HERO_ON_LEFT_PILLS_ROW_HEIGHT: u16 = 1;
/// Blank rows below the pill row before the list starts.
const HERO_ON_LEFT_PILLS_GAP_ROWS: u16 = 1;

/// Returns `(left_pane, right_pane)` for the hero-on-left arrangement's
/// horizontal split: a `HERO_ON_LEFT_PANE_GAP`-column gutter between a
/// ~40%-width left (hero) pane and the remaining right (list) pane, each
/// floored at `HERO_ON_LEFT_MIN_PANE_WIDTH`. Extracted unchanged from grouped
/// Music's former `wide_music_panes`.
pub(super) fn hero_on_left_panes(content_area: Rect) -> (Rect, Rect) {
    let left_w = ((content_area.width as u32 * 2 / 5) as u16)
        .max(HERO_ON_LEFT_MIN_PANE_WIDTH)
        .min(
            content_area
                .width
                .saturating_sub(HERO_ON_LEFT_MIN_PANE_WIDTH),
        );
    let right_w = content_area
        .width
        .saturating_sub(left_w)
        .saturating_sub(HERO_ON_LEFT_PANE_GAP);
    (
        Rect {
            x: content_area.x,
            y: content_area.y,
            width: left_w,
            height: content_area.height,
        },
        Rect {
            x: content_area.x + left_w + HERO_ON_LEFT_PANE_GAP,
            y: content_area.y,
            width: right_w,
            height: content_area.height,
        },
    )
}

/// The hero-on-left arrangement's right (list) pane geometry: a one-row pill
/// bar flush with the pane's top, then the list panel below it (decision
/// 6's "pill row at top of list pane"). `right_panel` is the pane's full
/// rect (its `y`/`height` anchor the pill row and the panel's bottom);
/// `right_area` is the vertically-inset pane used for the pill row's
/// x/width. `bottom_pad` is the caller's own trailing padding reserve
/// (grouped Music's `PANE_PAD_Y`), kept as a parameter rather than a second
/// constant here so the two files do not each own a copy of the same value.
/// Extracted unchanged from grouped Music's former `render_wide_music_group`.
pub(super) struct HeroOnLeftRightPane {
    pub pills_area: Rect,
    pub list_panel: Rect,
}

pub(super) fn hero_on_left_right_pane(
    right_panel: Rect,
    right_area: Rect,
    bottom_pad: u16,
) -> HeroOnLeftRightPane {
    let pills_area = Rect {
        x: right_area.x,
        y: right_panel.y,
        width: right_area.width,
        height: HERO_ON_LEFT_PILLS_ROW_HEIGHT,
    };
    let browser_y = right_panel.y + HERO_ON_LEFT_PILLS_ROW_HEIGHT + HERO_ON_LEFT_PILLS_GAP_ROWS;
    let list_panel = Rect {
        x: right_area.x,
        y: browser_y,
        width: right_area.width,
        height: right_panel.height.saturating_sub(
            HERO_ON_LEFT_PILLS_ROW_HEIGHT + HERO_ON_LEFT_PILLS_GAP_ROWS + bottom_pad,
        ),
    };
    HeroOnLeftRightPane {
        pills_area,
        list_panel,
    }
}

/// Paints the hero-on-left arrangement's right (list) pane border: hero-on-left's
/// declared variant of the shared [`render_selected_block_borders`] primitive
/// (design.md decision 6) -- a `▔` top row and a `▁` bottom row, with a
/// focus-resolved background, one row inside `list_panel`'s own top/bottom
/// edge. The variant is a separate match arm in `render_selected_block_borders`
/// (`SelectedBlockBorderStyle::HeroOnLeft`), so a hero-on-left-only change
/// here can never reach the `HeroOnTop` arm hero-on-top's `hero_block_shell`
/// uses. hero-on-left's own fixed window mirrors `hero_block_shell`'s
/// (`offset = 0`, fully visible, padding rows `[1, height - 2]`); this is
/// hero-on-left's thin shell entry point, the same role `hero_block_shell`
/// plays for hero-on-top.
pub(super) fn hero_on_left_list_panel_border(f: &mut Frame, list_panel: Rect, focused: bool) {
    if list_panel.height == 0 {
        return;
    }
    super::render_selected_block_borders(
        f,
        list_panel,
        0,
        list_panel.height as usize,
        1,
        (list_panel.height as usize).saturating_sub(2),
        super::SelectedBlockBorderStyle::HeroOnLeft { focused },
    );
}

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

/// One line of the `Hero` component's hero-on-left text block. Unlike
/// hero-on-top's single-row, truncated [`HeroLine`], hero-on-left text wraps
/// across as many rows as it needs (design.md decision 2's "Consequence":
/// text wrapping moves into `Hero`, screens hand over unwrapped strings).
/// Style is screen-chosen (e.g. focus-derived bold), matching how
/// `HeroContent::meta_color` lets a hero-on-top screen pick its own colour.
pub(super) struct WrappedHeroLine<'a> {
    pub text: &'a str,
    pub style: Style,
}

/// Paints `lines` wrapped to `area`'s width, top to bottom, stopping at
/// `area`'s bottom edge; empty line text is skipped. Returns the first
/// unpainted row. Extracted unchanged from grouped Music's former
/// `render_wide_left_hero`/`render_wrapped_text`.
pub(super) fn paint_hero_on_left_text(f: &mut Frame, area: Rect, lines: &[WrappedHeroLine]) -> u16 {
    if area.height == 0 || area.width < 3 {
        return area.y;
    }
    let mut row = area.y;
    let wrap_width = (area.width as usize).saturating_sub(1);
    for line in lines {
        if line.text.is_empty() {
            continue;
        }
        for wrapped in wrap(line.text, wrap_width.max(1)) {
            if row >= area.bottom() {
                return row;
            }
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(wrapped.into_owned(), line.style))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }
    }
    row
}
