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
/// paints in the target colour (yellow while local, green while remote).
#[must_use]
pub fn target_icon(use_nerd_fonts: bool, host_is_remote: bool) -> &'static str {
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
    /// The playback-target icon glyph (`target_icon`), painted in the target
    /// colour (yellow while local, green while remote).
    pub icon: &'static str,
    pub title_site: NowPlayingTitleSite,
    /// The now-playing status, painted as its status word
    /// (`header_status_word`) in its semantic colour (`status_color`).
    pub status: NowPlayingStatus,
    /// The playback target's host label and whether it names a remote
    /// target. The remote flag picks the target icon's colour (yellow local,
    /// green remote); while remote, the host name paints in ordinary text
    /// after one space. A local target paints the icon alone.
    pub host: &'a str,
    pub host_is_remote: bool,
}

/// The queue header row: one cell of interior padding each side, the
/// playback-target icon in the target colour (yellow while local, green
/// while remote) and, while a remote host owns playback, one space and the
/// host name — every element separated by one space — with the playback
/// status word right-aligned against the right padding. With the now-playing
/// title riding (the `Header` title site: no artwork slot or glyphs the
/// overlay cannot render), the title fills the zone between the target and
/// the right-aligned status, one space clear of each; a two-part title keeps
/// its context-left/title-right split and a lone title's marquee takes the
/// whole zone. The artwork site carries the title on the overlay, so that
/// row paints the target and the right-aligned status alone. Fills
/// `target_area` with the target indicator's painted rect (clamped to the
/// row), the click target for the F3 sessions shortcut; empty when the row
/// is degenerate.
pub fn render_header_title(
    frame: &mut Frame,
    row: Rect,
    header: &mut HeaderTitle<'_>,
    target_area: &mut Rect,
) {
    *target_area = Rect::default();
    if row.width == 0 || row.height == 0 {
        return;
    }
    let panel_bg = palette::surface_colors(header.panel, false).fill;
    let total = row.width as usize;
    // One cell of interior padding each side; the target indicator starts on
    // the first content cell.
    let content = total.saturating_sub(2);
    let mut spans = vec![Span::styled(" ", Style::default().bg(panel_bg))];
    let target = target_spans(header, panel_bg);
    let target_w: usize = target.iter().map(|span| span.content.width()).sum();
    *target_area = Rect {
        x: row.x.saturating_add(1),
        y: row.y,
        width: u16::try_from(target_w)
            .unwrap_or(u16::MAX)
            .min(u16::try_from(content).unwrap_or(u16::MAX)),
        height: 1,
    };
    spans.extend(target);
    let status = Span::styled(
        header_status_word(header.status).to_string(),
        Style::default()
            .fg(status_color(header.status))
            .bg(panel_bg)
            .add_modifier(Modifier::BOLD),
    );
    let status_w = status.content.width();
    if header.title_site == NowPlayingTitleSite::Header
        && let Some(title_text) = header.title
    {
        // The title zone is what remains between the target and the
        // right-aligned status: one space after the target, one before the
        // status.
        let zone = content.saturating_sub(target_w + status_w + 2);
        if zone > 0 {
            spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
            spans.extend(title_spans(header, title_text, panel_bg, zone));
        }
    }
    // Right-align the status word inside the right padding: the last status
    // cell is the row's second-to-last, and the final cell stays padding.
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    let status_start = total.saturating_sub(status_w + 1);
    let gap = status_start.saturating_sub(used);
    if gap > 0 {
        spans.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
    }
    spans.push(status);
    spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
    // `Paragraph::style` fills the cells the spans leave uncovered.
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
        row,
    );
}

/// The header's playback-target indicator: the target icon in the target
/// colour (yellow while local, green while remote), and while a remote host
/// owns playback one space and the host name in ordinary text.
fn target_spans(header: &HeaderTitle<'_>, panel_bg: Color) -> Vec<Span<'static>> {
    let icon_fg = if header.host_is_remote {
        palette::Role::PlaybackTargetRemoteFg.color()
    } else {
        palette::Role::PlaybackTargetLocalFg.color()
    };
    let mut spans = vec![Span::styled(
        header.icon.to_string(),
        Style::default().fg(icon_fg).bg(panel_bg),
    )];
    if header.host_is_remote && !header.host.is_empty() {
        spans.push(Span::styled(
            format!(" {}", header.host),
            Style::default()
                .fg(palette::Role::TextPrimary.color())
                .bg(panel_bg),
        ));
    }
    spans
}

/// The title spans for the header's `zone`-wide title zone: a two-part title
/// clips its context (show) part left — the show keeps priority and the
/// title marquee keeps a one-cell floor — and a lone title's marquee takes
/// the whole zone. The caller owns the gaps around the zone.
fn title_spans(
    header: &mut HeaderTitle<'_>,
    title_text: &str,
    panel_bg: Color,
    zone: usize,
) -> Vec<Span<'static>> {
    if let Some((context, title)) = header
        .parts
        .and_then(|parts| parts.context.as_ref().map(|c| (c, &parts.title)))
    {
        let show = clip_to_width(&context.text, zone.saturating_sub(2));
        let show_w = show.width();
        let title_win = zone.saturating_sub(show_w + 1);
        let mut spans = vec![Span::styled(
            show.to_string(),
            Style::default()
                .fg(title_part_fg(context.role))
                .bg(panel_bg),
        )];
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
        spans
    } else {
        marquee_spans_at(
            &[(title_text, palette::Role::PlaybackContextFg.color())],
            zone,
            header.marquee_text,
            header.marquee_started_at,
        )
    }
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
