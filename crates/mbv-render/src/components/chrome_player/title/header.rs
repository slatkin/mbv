use super::super::super::marquee;
use super::super::title_part_fg;
use super::palette;
use crate::components::queue_playback::{header_status_word, status_color};
use mbv_queue::PlaybackTitleParts;
use mbv_ui_model::playback::NowPlayingTitleSite;
use mbv_ui_model::playback_target::NowPlayingStatus;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

/// The header row's playback-target icon: the connected-remote glyph while a
/// remote host owns playback, the offline glyph otherwise — the same
/// nerd-font/fallback pair the queue footer's remote pill painted. The icon
/// paints inside the header's target pill (yellow while local, green while
/// remote) in `TargetPillFg` ink.
#[must_use]
pub fn target_pill_icon(use_nerd_fonts: bool, host_is_remote: bool) -> &'static str {
    match (use_nerd_fonts, host_is_remote) {
        (false, _) => "\u{1F5A7}",
        (true, true) => "\u{f0119}",
        (true, false) => "\u{f0118}",
    }
}

/// The queue header row's facts: everything the header painter needs besides
/// the frame and the row. The panel owns the marquee state.
#[derive(Debug)]
pub struct HeaderTitle<'a> {
    /// The now-playing title, `None` when nothing plays. It rides this row
    /// only while the header is the title's home (the `Header` title site);
    /// the artwork site carries the title on the overlay.
    pub title: Option<&'a str>,
    pub parts: Option<&'a PlaybackTitleParts>,
    pub marquee_text: &'a mut String,
    pub marquee_started_at: &'a mut std::time::Instant,
    pub panel: palette::Surface,
    /// The playback-target icon glyph (`target_pill_icon`), painted in ink
    /// inside the target pill.
    pub icon: &'static str,
    pub title_site: NowPlayingTitleSite,
    /// The now-playing status, painted as its status word
    /// (`header_status_word`) in its semantic colour (`status_color`).
    pub status: NowPlayingStatus,
    /// The playback target's host label and whether it names a remote
    /// target. The remote flag picks the target pill's fill (yellow local,
    /// green remote) and its icon; while remote, the host name paints
    /// inside the pill. A local target paints the icon alone.
    pub host: &'a str,
    pub host_is_remote: bool,
}

/// The queue header row: the playback-target pill left-anchored — one fill
/// cell, the target icon in ink, and while a remote host owns playback one
/// space and the host name, then one more fill cell (yellow while local,
/// green while remote) — one space, and the bold status word:
/// ` 󰄙 host  PLAYING`. With the now-playing title riding (the `Header`
/// title site: no artwork slot or glyphs the overlay cannot render), a
/// two-part title keeps its context-left/title-right split and a lone
/// title's marquee takes the whole remaining zone. The artwork site carries
/// the title on the overlay, so that row paints the status part alone.
/// Fills `pill_area` with the pill's painted rect (clamped to the row), the
/// click target for the F3 sessions shortcut; empty when the row is
/// degenerate.
pub fn render_header_title(
    frame: &mut Frame,
    row: Rect,
    header: &mut HeaderTitle<'_>,
    pill_area: &mut Rect,
) {
    *pill_area = Rect::default();
    if row.width == 0 || row.height == 0 {
        return;
    }
    let panel_bg = palette::surface_colors(header.panel, false).fill;
    let pill_spans = target_pill_spans(header);
    let pill_w: usize = pill_spans.iter().map(|span| span.content.width()).sum();
    *pill_area = Rect {
        x: row.x,
        y: row.y,
        width: u16::try_from(pill_w).unwrap_or(u16::MAX).min(row.width),
        height: 1,
    };
    let mut spans = pill_spans;
    spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
    spans.push(Span::styled(
        header_status_word(header.status).to_string(),
        Style::default()
            .fg(status_color(header.status))
            .bg(panel_bg)
            .add_modifier(Modifier::BOLD),
    ));
    if header.title_site == NowPlayingTitleSite::Header
        && let Some(title_text) = header.title
    {
        append_title_spans(header, title_text, panel_bg, &mut spans, row.width);
    }
    // `Paragraph::style` fills the cells the spans leave uncovered.
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
        row,
    );
}

/// The target pill's spans: one fill cell, the target icon in
/// `TargetPillFg` ink, and while a remote host owns playback one space and
/// the host name, then one more fill cell — yellow `TargetPillLocal` while
/// local, green `TargetPillRemote` while remote.
fn target_pill_spans(header: &HeaderTitle<'_>) -> Vec<Span<'static>> {
    let pill_fill = if header.host_is_remote {
        palette::surface_colors(palette::Surface::TargetPillRemote, false).fill
    } else {
        palette::surface_colors(palette::Surface::TargetPillLocal, false).fill
    };
    let pill = Style::default()
        .fg(palette::Role::TargetPillFg.color())
        .bg(pill_fill);
    let mut spans = vec![
        Span::styled(" ", pill),
        Span::styled(header.icon.to_string(), pill),
    ];
    if header.host_is_remote && !header.host.is_empty() {
        spans.push(Span::styled(format!(" {} ", header.host), pill));
    }
    spans.push(Span::styled(" ", pill));
    spans
}

/// The title spans appended after the status part while the header is the
/// title's home: a two-part title clips its context (show) part left — the
/// show keeps priority and the title marquee keeps a one-cell floor — and a
/// lone title's marquee takes the whole remaining zone. One cell stays
/// reserved for the row's trailing indent.
fn append_title_spans(
    header: &mut HeaderTitle<'_>,
    title_text: &str,
    panel_bg: Color,
    spans: &mut Vec<Span<'static>>,
    row_width: u16,
) {
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    // One gap cell between the status part and the title zone, one trailing
    // indent cell at the row's end.
    let zone = (row_width as usize).saturating_sub(used + 2);
    spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
    if let Some((context, title)) = header
        .parts
        .and_then(|parts| parts.context.as_ref().map(|c| (c, &parts.title)))
    {
        let show = clip_to_width(&context.text, zone.saturating_sub(2));
        let show_w = show.width();
        let title_win = zone.saturating_sub(show_w + 1);
        spans.push(Span::styled(
            show.to_string(),
            Style::default()
                .fg(title_part_fg(context.role))
                .bg(panel_bg),
        ));
        let marquee = marquee_spans_at(
            &[(title_text, title_part_fg(title.role))],
            title_win,
            header.marquee_text,
            header.marquee_started_at,
        );
        let gap = zone.saturating_sub(show_w + title_win);
        if gap > 0 {
            spans.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
        }
        spans.extend(marquee);
    } else {
        let marquee = marquee_spans_at(
            &[(title_text, palette::Role::PlaybackContextFg.color())],
            zone,
            header.marquee_text,
            header.marquee_started_at,
        );
        spans.extend(marquee);
    }
    spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
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
