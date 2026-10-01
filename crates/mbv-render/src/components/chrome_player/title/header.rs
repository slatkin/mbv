use super::super::super::chrome::{pause_icon, play_icon};
use super::super::super::marquee;
use super::super::title_part_fg;
use super::palette;
use super::transport::width_u16;
use mbv_queue::PlaybackTitleParts;
use mbv_ui_model::playback::NowPlayingTitleSite;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

/// The header row's state icon: the play glyph while playing, the pause
/// glyph while paused (same nerd-font/fallback pairs as the transport
/// controls), aqua for play and yellow for pause.
#[must_use]
pub fn playback_state_icon(use_nerd_fonts: bool, paused: bool) -> (&'static str, Color) {
    if paused {
        (pause_icon(use_nerd_fonts), palette::TEXT_FOCUS_ACCENT)
    } else {
        (play_icon(use_nerd_fonts), palette::ACCENT)
    }
}

/// The header row's now-playing title facts: everything the header painter
/// needs besides the frame and the row. The panel owns the marquee state
/// and the icon (`playback_state_icon`).
#[derive(Debug)]
pub struct HeaderTitle<'a> {
    pub title: &'a str,
    pub parts: Option<&'a PlaybackTitleParts>,
    pub marquee_text: &'a mut String,
    pub marquee_started_at: &'a mut std::time::Instant,
    pub panel: palette::Surface,
    pub icon: (&'static str, Color),
    pub title_site: NowPlayingTitleSite,
    /// The playback target's host label and whether it names a remote
    /// target; the artwork-site brand row paints it after `on`
    /// (`PLAYBACK_HOST_REMOTE_FG` when remote, cream when local).
    pub host: &'a str,
    pub host_is_remote: bool,
}

/// The header row's now-playing title (moved up from the band's title row):
/// the state icon first, one space, then a two-part title — the
/// context part (the show) left-aligned and clipped without scrolling, the
/// title part with the marquee window of the remaining space — or a lone
/// yellow title. Painted only while a target plays; idle keeps the
/// status/host header row.
pub fn render_header_title(frame: &mut Frame, row: Rect, header: &mut HeaderTitle<'_>) {
    let panel_bg = palette::surface_colors(header.panel, false).fill;
    if header.title_site == NowPlayingTitleSite::Artwork {
        frame.render_widget(
            Paragraph::new(Line::from(artwork_brand_spans(
                header,
                panel_bg,
                row.width as usize,
            )))
            .style(Style::default().bg(panel_bg)),
            row,
        );
        return;
    }
    match header
        .parts
        .and_then(|parts| parts.context.as_ref().map(|c| (c, &parts.title)))
    {
        Some((context, title)) => render_queue_header_combined_title(
            frame,
            row,
            header,
            panel_bg,
            (&context.text, title_part_fg(context.role)),
            (&title.text, title_part_fg(title.role)),
        ),
        None => render_queue_header_title_only(frame, row, header, panel_bg),
    }
}

/// The artwork title site's header row: ` [mbv]` left-anchored and
/// `PLAYING:<host>` right-anchored, one trailing space outside the right
/// edge. The icon is dropped (the word carries the state), the brackets and
/// the colon are cream, `mbv` is foam, `PLAYING` is aqua and bold, and the
/// host is cream when local / `PLAYBACK_HOST_REMOTE_FG` when remote.
fn artwork_brand_spans(
    header: &HeaderTitle<'_>,
    panel_bg: Color,
    row_width: usize,
) -> Vec<Span<'static>> {
    let cream = Style::default().fg(palette::TEXT_EMPHASIS).bg(panel_bg);
    let mut spans = vec![
        Span::styled(" ", Style::default().bg(panel_bg)),
        Span::styled("[", cream),
        Span::styled(
            "mbv",
            Style::default().fg(palette::TEXT_METADATA).bg(panel_bg),
        ),
        Span::styled("]", cream),
    ];
    let mut right = vec![Span::styled(
        "PLAYING",
        Style::default()
            .fg(palette::ACCENT)
            .bg(panel_bg)
            .add_modifier(Modifier::BOLD),
    )];
    if !header.host.is_empty() {
        let host_fg = if header.host_is_remote {
            palette::PLAYBACK_HOST_REMOTE_FG
        } else {
            palette::TEXT_EMPHASIS
        };
        right.push(Span::styled(":", cream));
        right.push(Span::styled(
            header.host.to_string(),
            Style::default().fg(host_fg).bg(panel_bg),
        ));
    }
    let left_w: usize = spans.iter().map(|span| span.content.width()).sum();
    let right_w: usize = right.iter().map(|span| span.content.width()).sum();
    let gap = row_width.saturating_sub(left_w + right_w + 1);
    if gap > 0 {
        spans.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
    }
    spans.extend(right);
    spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
    spans
}

/// The ` <icon> ` prefix every header title row starts with.
fn icon_prefix(icon: (&'static str, Color), panel_bg: Color) -> [Span<'static>; 3] {
    [
        Span::styled(" ", Style::default().bg(panel_bg)),
        Span::styled(icon.0, Style::default().fg(icon.1).bg(panel_bg)),
        Span::styled(" ", Style::default().bg(panel_bg)),
    ]
}

/// The queue header row's lone title, no time: ` <icon> <title> ` with the
/// marquee window sized to the row minus its indent cells. Only called when
/// no context part projects (a two-part title takes the combined painter);
/// the lone title always paints yellow.
fn render_queue_header_title_only(
    frame: &mut Frame,
    row: Rect,
    header: &mut HeaderTitle<'_>,
    panel_bg: Color,
) {
    let text = header.parts.map_or(header.title, |parts| &parts.title.text);
    let icon_w = width_u16(header.icon.0.width());
    let mut spans = icon_prefix(header.icon, panel_bg).to_vec();
    spans.extend(marquee_spans_at(
        &[(text, palette::PLAYBACK_CONTEXT_FG)],
        (row.width.saturating_sub(2 + icon_w + 1)) as usize,
        header.marquee_text,
        header.marquee_started_at,
    ));
    // `Paragraph::style` fills the cells the spans leave uncovered.
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
        row,
    );
}

/// A two-part title on the queue header row: ` <icon> <show> ... <title> ` — the context part
/// left-aligned and clipped without scrolling (no marquee), the title part
/// right-aligned with the marquee window of the remaining space. The show
/// keeps priority: it clips only past `content - 2`, so the title always
/// keeps at least a one-cell marquee floor and no part is ever dropped.
fn render_queue_header_combined_title(
    frame: &mut Frame,
    row: Rect,
    header: &mut HeaderTitle<'_>,
    panel_bg: Color,
    (show_text, show_fg): (&str, Color),
    (title_text, title_fg): (&str, Color),
) {
    let icon_w = usize::from(width_u16(header.icon.0.width()));
    let content = row.width.saturating_sub(2) as usize;
    let show = clip_to_width(show_text, content.saturating_sub(icon_w + 1 + 2));
    let sw = width_u16(show.width());
    let title_win = content.saturating_sub(icon_w + 1 + usize::from(sw) + 1);
    let mut spans = icon_prefix(header.icon, panel_bg).to_vec();
    spans.push(Span::styled(
        show,
        Style::default().fg(show_fg).bg(panel_bg),
    ));
    let marquee = marquee_spans_at(
        &[(title_text, title_fg)],
        title_win,
        header.marquee_text,
        header.marquee_started_at,
    );
    let used = spans
        .iter()
        .chain(&marquee)
        .map(|span| span.content.width())
        .sum::<usize>();
    // Right-anchor the title: every spare cell lands between the show and the
    // title; the trailing indent is kept outside the gap math.
    let gap = (row.width as usize).saturating_sub(used + 1);
    if gap > 0 {
        spans.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
    }
    spans.extend(marquee);
    spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
        row,
    );
}

/// `text` cut char by char from the end until it fits `max` cells.
pub(super) fn clip_to_width(text: &str, max: usize) -> &str {
    let mut clipped = text;
    while clipped.width() > max
        && let Some(last) = clipped.chars().last()
    {
        clipped = &clipped[..clipped.len() - last.len_utf8()];
    }
    clipped
}

/// Marquee spans for one part set: the single adapter into the shared
/// marquee machinery, used by the header painter and `marquee_spans`.
pub(super) fn marquee_spans_at(
    parts: &[(&str, Color)],
    max_width: usize,
    marquee_text: &mut String,
    marquee_started_at: &mut std::time::Instant,
) -> Vec<Span<'static>> {
    let key: String = parts.iter().map(|(text, _)| *text).collect();
    marquee::marquee_spans(
        &key,
        parts,
        max_width,
        marquee_text,
        marquee_started_at,
        false,
    )
}
