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
/// nerd-font/fallback pair the queue footer's remote pill painted — always
/// yellow.
#[must_use]
pub fn remote_state_icon(use_nerd_fonts: bool, host_is_remote: bool) -> (&'static str, Color) {
    let icon = match (use_nerd_fonts, host_is_remote) {
        (false, _) => "\u{1F5A7}",
        (true, true) => "\u{f0119}",
        (true, false) => "\u{f0118}",
    };
    (icon, palette::Role::TextFocusAccent.color())
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
    /// The playback-target icon (`remote_state_icon`), yellow.
    pub icon: (&'static str, Color),
    pub title_site: NowPlayingTitleSite,
    /// The now-playing status, painted as its status word
    /// (`header_status_word`) in its semantic colour (`status_color`).
    pub status: NowPlayingStatus,
    /// The playback target's host label and whether it names a remote
    /// target; every header row paints it bracketed
    /// (`PlaybackHostRemoteFg` when remote, cream when local).
    pub host: &'a str,
    pub host_is_remote: bool,
}

/// The queue header row: the playback-target icon, the bracketed host, and
/// the bold status word left-anchored — `󰄘 [host] PLAYING` — with the
/// now-playing title riding the remaining space while the header is the
/// title's home (the `Header` site): a two-part title keeps its
/// context-left/title-right split, a lone title's marquee takes the whole
/// remaining zone. The artwork site carries the title on the overlay, so
/// that row paints the status part alone.
pub fn render_header_title(frame: &mut Frame, row: Rect, header: &mut HeaderTitle<'_>) {
    let panel_bg = palette::surface_colors(header.panel, false).fill;
    let mut spans = status_brand_spans(header, panel_bg);
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

/// The status part every header row paints: one leading indent cell, the
/// playback-target icon (yellow), one space, the bracketed host (foam
/// brackets, the host mauve when remote / cream when local), one space, and
/// the bold status word in its semantic colour.
fn status_brand_spans(header: &HeaderTitle<'_>, panel_bg: Color) -> Vec<Span<'static>> {
    let (icon, icon_fg) = header.icon;
    let bg = Style::default().bg(panel_bg);
    let mut spans = vec![
        Span::styled(" ", bg),
        Span::styled(icon.to_string(), Style::default().fg(icon_fg).bg(panel_bg)),
    ];
    if !header.host.is_empty() {
        let host_fg = if header.host_is_remote {
            palette::Role::PlaybackHostRemoteFg.color()
        } else {
            palette::Role::TextEmphasis.color()
        };
        let foam = Style::default()
            .fg(palette::Role::TextMetadata.color())
            .bg(panel_bg);
        spans.push(Span::styled(" ", bg));
        spans.push(Span::styled("[", foam));
        spans.push(Span::styled(
            header.host.to_string(),
            Style::default().fg(host_fg).bg(panel_bg),
        ));
        spans.push(Span::styled("]", foam));
    }
    spans.push(Span::styled(" ", bg));
    spans.push(Span::styled(
        header_status_word(header.status).to_string(),
        Style::default()
            .fg(status_color(header.status))
            .bg(panel_bg)
            .add_modifier(Modifier::BOLD),
    ));
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
