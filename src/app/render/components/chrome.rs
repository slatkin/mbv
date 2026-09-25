use crate::app::palette;
use crate::app::ui_util::*;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use tui_scrollbar::GlyphSet;

pub(in crate::app) fn thin_vertical_thumb(mut glyphs: GlyphSet) -> GlyphSet {
    glyphs.thumb_vertical_lower = ['▕'; 8];
    glyphs.thumb_vertical_upper = ['▕'; 8];
    glyphs
}

pub(in crate::app::render) const PLAY_ICON: &str = "\u{f04b}";
const PLAY_ICON_FALLBACK: &str = ">";

pub(in crate::app::render) fn play_icon(use_nerd_fonts: bool) -> &'static str {
    if use_nerd_fonts {
        PLAY_ICON
    } else {
        PLAY_ICON_FALLBACK
    }
}

pub(in crate::app::render) fn daemon_endpoint_label(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.eq_ignore_ascii_case("local") {
        return None;
    }
    if let Some(tcp) = endpoint.strip_prefix("tcp://") {
        return tcp
            .rsplit_once(':')
            .map(|(host, _port)| host)
            .filter(|host| !host.is_empty())
            .map(str::to_string);
    }
    if let Some(path) = endpoint.strip_prefix("unix://") {
        return std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
    }
    std::path::Path::new(endpoint)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

// --- Render-seam free functions (design D9, task 3.1) ---
//
// Extracted from `impl App` methods so Interactive Components can call them
// without `App` access. Output-preserving: the function bodies are the former
// `impl App` method bodies, only `Self::` → direct calls.
//
// `pub(in crate::app)` so the `render` module can re-export them for the
// Interactive Components in `crate::app::components` (design D5/D9).

pub(in crate::app) fn panel_shell_rect(full: Rect, width: u16) -> Rect {
    Rect {
        x: full.x,
        y: full.y + 2,
        width: width.min(full.width),
        height: full.height.saturating_sub(2),
    }
}

pub(in crate::app) fn left_panel_content_area(sidebar: Rect) -> Rect {
    Rect {
        x: sidebar.x + 2,
        y: sidebar.y + 3,
        width: sidebar.width.saturating_sub(4),
        height: sidebar.height.saturating_sub(5),
    }
}

pub(in crate::app) fn render_panel_shell_at(
    f: &mut Frame,
    sidebar: Rect,
    title: &str,
    hints: &str,
) -> Rect {
    f.render_widget(Clear, sidebar);
    let body_bg = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
    let band_bg = palette::surface_colors(palette::Surface::SidebarBand, false).fill;
    // Too short to fit a title row, a content row, and the 2-row footer;
    // bail out rather than let `footer_y = sidebar.y + sidebar.height - 2`
    // underflow below.
    if sidebar.height < 4 || sidebar.width == 0 {
        return left_panel_content_area(sidebar);
    }
    f.render_widget(
        Block::default().style(Style::default().bg(body_bg)),
        sidebar,
    );
    let inner_w = sidebar.width.saturating_sub(4);
    let ix = sidebar.x + 2;
    let header_style = Style::default()
        .fg(palette::TEXT_PRIMARY)
        .bg(band_bg)
        .add_modifier(Modifier::BOLD);
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" {title}"),
            header_style,
        )]))
        .style(Style::default().bg(band_bg)),
        Rect {
            x: sidebar.x + 2,
            y: sidebar.y + 1,
            width: sidebar.width.saturating_sub(4),
            height: 1,
        },
    );
    let footer_y = sidebar.y + sidebar.height - 2;
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            trunc_str(hints, inner_w as usize),
            Style::default().fg(palette::TEXT_PRIMARY),
        )]))
        .style(Style::default().bg(band_bg)),
        Rect {
            x: ix,
            y: footer_y,
            width: inner_w,
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Span::raw("")).style(Style::default().bg(body_bg)),
        Rect {
            x: sidebar.x,
            y: sidebar.y + sidebar.height - 1,
            width: sidebar.width,
            height: 1,
        },
    );
    left_panel_content_area(sidebar)
}

/// Overlay a thin scroll indicator on a sidebar's right border column when
/// its content doesn't fit `content.height`. Reuses the existing border
/// column instead of reserving a dedicated width for a scrollbar. Thumb-only
/// (`minimal()`): a box-drawing track line sits half a cell off the block
/// thumb and reads as a disjoint column, so the track stays invisible and
/// the grey thumb alone marks position — the same treatment as the main
/// lists' `render_right_scrollbar`.
pub(in crate::app) fn render_sidebar_scrollbar(
    f: &mut Frame,
    content: Rect,
    total: usize,
    scroll: usize,
) {
    super::widgets::render_scrollbar_with_viewport_at(
        f,
        content,
        total,
        content.height as usize,
        scroll,
        content.x.saturating_add(content.width),
        thin_vertical_thumb(GlyphSet::minimal()),
        palette::SIDEBAR_SCROLLBAR,
    );
}

/// Render one row in a sidebar panel list.
/// `content_spans` should not include the indicator — it is prepended automatically.
/// Returns the usable text width (content area minus indicator and space).
pub(in crate::app) fn panel_row_text_width(content_width: u16) -> usize {
    content_width.saturating_sub(1) as usize // indicator char
}

pub(in crate::app) fn render_panel_row(
    f: &mut Frame,
    x: u16,
    y: u16,
    width: u16,
    selected: bool,
    spans: Vec<Span>,
    bg: Option<Color>,
) {
    // A caller-painted background (zebra stripe, selected bar) fills the
    // whole row through the widget style; the selected bar carries no gutter
    // mark, so its indicator column stays blank to hold the text alignment.
    let (mark, mark_fg) = if selected && bg.is_some() {
        (" ", None)
    } else {
        (
            if selected { "\u{258c}" } else { " " },
            Some(palette::ACCENT),
        )
    };
    let mut mark_style = Style::default();
    if let Some(fg) = mark_fg {
        mark_style = mark_style.fg(fg);
    }
    let mut all = vec![Span::styled(mark, mark_style)];
    all.extend(spans);
    let mut row = Paragraph::new(Line::from(all));
    if let Some(bg) = bg {
        row = row.style(Style::default().bg(bg));
    }
    f.render_widget(
        row,
        Rect {
            x,
            y,
            width,
            height: 1,
        },
    );
}
