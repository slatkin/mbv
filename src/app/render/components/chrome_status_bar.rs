//! Status-bar layout and painter.
//!
//! The mounted `StatusBarPanel` Interactive Component
//! (`src/app/components/status_bar_panel.rs`) owns the status row's pill hit
//! regions, overflow drop-order and click/scroll resolution. This module owns
//! its visual model, layout, and the single painter. The Local/Remote queue-
//! scope pills are queue concern and paint in the `QueueColumn` footer
//! (`render_queue_status`), never here.

use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

fn status_width(spans: &[Span]) -> u16 {
    spans_width(spans)
}

fn append_status(spans: &mut Vec<Span<'static>>, status: Vec<Span<'static>>) {
    if !spans.is_empty() {
        spans.push(Span::raw(" "));
    }
    spans.extend(status);
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct VisualModeIndicator {
    /// Number of selected items.
    pub count: usize,
}
/// Plain-data paint model for one status row. The shell projects spans; the
/// mounted `StatusBarPanel` owns overflow, hit regions and Visual-mode clearing.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct StatusBarModel {
    /// Whether the remote/session pill participates (the base frame has
    /// always passed `false` here — the queue-scope pills below show the
    /// same info; the parameter is retained verbatim).
    pub show_session_pill: bool,
    /// Remote/session pill spans (empty unless `show_session_pill`).
    pub remote: Vec<Span<'static>>,
    /// Mute pill spans (absent when not muted).
    pub mute: Option<Vec<Span<'static>>>,
    /// Volume pill spans.
    pub volume: Vec<Span<'static>>,
    /// Fully built right segment (scope label, username, service glyphs).
    pub right: Vec<Span<'static>>,
    /// Visual-mode count indicator, when selected items exist.
    pub visual_mode: Option<VisualModeIndicator>,
    /// Prefix-armed pill spans (absent when not armed).
    pub prefix_armed: Option<Vec<Span<'static>>>,
}
/// The status row's pointer regions, retained by the mounted
/// `StatusBarPanel` after painting.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct StatusBarRegions {
    /// Volume pill: scroll-wheel adjusts the volume.
    pub volume: Option<Rect>,
    /// Mute pill: click toggles mute.
    pub mute: Option<Rect>,
    /// Remote/session pill region, when the session pill is enabled.
    pub remote: Option<Rect>,
    /// Visual-mode region; clicking it clears selection.
    pub visual_clear: Option<Rect>,
}

/// Paint the one-row status bar within `area` (the `RootFrame.status_bar`
/// placement) and return the painted pill regions.
///
/// Persistent bottom status bar. Left side: volume, connection,
/// and mute status groups. Right side: queue source/save-state/scope
/// detail and the service-state glyphs (Emby, Audiobookshelf,
/// stay-alive). The playlist status pill renders in the left queue panel
/// instead; the Local/Remote queue-scope pills paint in the `QueueColumn`
/// footer (`render_queue_status`), never here.
struct StatusBarLeftSegments {
    mute: Option<Vec<Span<'static>>>,
    volume: Vec<Span<'static>>,
    remote: Vec<Span<'static>>,
    armed: Option<Vec<Span<'static>>>,
    visual: Option<Vec<Span<'static>>>,
    volume_width: u16,
    remote_width: u16,
    visual_width: u16,
    fits: StatusBarFit,
    visibility: StatusBarVisibility,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent per-segment visibility results over distinct status-bar fit drop tiers; all four are simultaneously true at StatusBarFit::All (design analysis, issue #804)"
)]
#[derive(Clone, Copy)]
struct StatusBarVisibility {
    visual: bool,
    armed: bool,
    remote: bool,
    volume: bool,
}

struct StatusBarWidths {
    visual: u16,
    armed: u16,
    remote: u16,
    mute: u16,
    volume: u16,
}

#[derive(Clone, Copy)]
enum StatusBarFit {
    All,
    WithoutMute,
    WithoutVolume,
    WithoutRemote,
    None,
}

fn status_bar_fit(widths: &StatusBarWidths, available: u16) -> StatusBarFit {
    let joined_width = |widths: &[u16]| -> u16 {
        let mut total = 0u16;
        for (count, width) in widths.iter().copied().filter(|w| *w > 0).enumerate() {
            total = total.saturating_add(width);
            if count > 0 {
                total = total.saturating_add(1);
            }
        }
        total
    };
    let all = joined_width(&[
        widths.visual,
        widths.armed,
        widths.remote,
        widths.mute,
        widths.volume,
    ]) <= available;
    let without_mute = !all
        && joined_width(&[widths.visual, widths.armed, widths.remote, widths.volume]) <= available;
    let without_volume = !all
        && !without_mute
        && joined_width(&[widths.visual, widths.armed, widths.remote, widths.mute]) <= available;
    if all {
        StatusBarFit::All
    } else if without_mute {
        StatusBarFit::WithoutMute
    } else if without_volume {
        StatusBarFit::WithoutVolume
    } else if joined_width(&[widths.visual, widths.armed, widths.mute, widths.volume]) <= available
    {
        StatusBarFit::WithoutRemote
    } else {
        StatusBarFit::None
    }
}

fn status_bar_left_segments(model: &StatusBarModel, available: u16) -> StatusBarLeftSegments {
    let mute = model.mute.clone();
    let volume = model.volume.clone();
    let remote = if model.show_session_pill {
        model.remote.clone()
    } else {
        Vec::new()
    };
    let armed = model.prefix_armed.clone();

    // Preserve the existing left-segment overflow order: mute drops
    // first, then the volume pill, then remote. The armed pill sits with
    // the visual-mode indicator at the top persistence tier: it drops only
    // when nothing else is left (it names the active routing mode).
    let remote_width = status_width(&remote);
    let armed_width = armed.as_ref().map_or(0, |spans| status_width(spans));
    let visual = model.visual_mode.as_ref().map(|indicator| {
        vec![Span::styled(
            format!("-- VISUAL ({}) --", indicator.count),
            Style::default()
                .fg(palette::TEXT_FOCUS_ACCENT)
                .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill),
        )]
    });
    let visual_width = visual.as_ref().map_or(0, |spans| status_width(spans));
    let mute_width: u16 = mute.as_ref().map_or(0, |spans| status_width(spans));
    let volume_width = status_width(&volume);
    let fits = status_bar_fit(
        &StatusBarWidths {
            visual: visual_width,
            armed: armed_width,
            remote: remote_width,
            mute: mute_width,
            volume: volume_width,
        },
        available,
    );
    let any_fit = !matches!(fits, StatusBarFit::None);
    let visibility = StatusBarVisibility {
        visual: visual_width > 0 && any_fit,
        armed: armed_width > 0 && any_fit,
        remote: remote_width > 0
            && matches!(
                fits,
                StatusBarFit::All | StatusBarFit::WithoutMute | StatusBarFit::WithoutVolume
            ),
        volume: matches!(
            fits,
            StatusBarFit::All | StatusBarFit::WithoutMute | StatusBarFit::WithoutRemote
        ),
    };
    StatusBarLeftSegments {
        mute,
        volume,
        remote,
        armed,
        visual,
        volume_width,
        remote_width,
        visual_width,
        fits,
        visibility,
    }
}

fn render_status_bar_left(
    f: &mut Frame,
    area: Rect,
    bar_style: Style,
    segments: StatusBarLeftSegments,
) -> (StatusBarRegions, u16) {
    let mut regions = StatusBarRegions::default();
    let mut spans: Vec<Span> = Vec::new();
    if segments.visibility.visual {
        let visual_x = area.x + status_width(&spans);
        append_status(&mut spans, segments.visual.unwrap_or_default());
        regions.visual_clear = Some(Rect {
            x: visual_x,
            y: area.y,
            width: segments.visual_width,
            height: 1,
        });
    }
    if segments.visibility.armed {
        append_status(&mut spans, segments.armed.unwrap_or_default());
    }
    if segments.visibility.volume {
        let vol_x = area.x + status_width(&spans);
        append_status(&mut spans, segments.volume);
        regions.volume = Some(Rect {
            x: vol_x,
            y: area.y,
            width: segments.volume_width,
            height: 1,
        });
    }
    let remote_x = segments
        .visibility
        .remote
        .then(|| area.x + status_width(&spans) + u16::from(!spans.is_empty()));
    if segments.visibility.remote {
        append_status(&mut spans, segments.remote);
        regions.remote = remote_x.map(|x| Rect {
            x,
            y: area.y,
            width: segments.remote_width,
            height: 1,
        });
    }
    if matches!(segments.fits, StatusBarFit::All | StatusBarFit::WithoutMute) {
        if let Some(mute) = segments.mute {
            let mute_x = area.x + status_width(&spans);
            let mute_width = status_width(&mute);
            append_status(&mut spans, mute);
            regions.mute = Some(Rect {
                x: mute_x,
                y: area.y,
                width: mute_width,
                height: 1,
            });
        }
    }

    // `left_content_w` tracks how far the left segment actually extends after
    // the above priority drop, so the right-segment overlap check can compare
    // against the real left edge instead of a hardcoded constant.
    let label_w = spans_width(&spans);
    if !spans.is_empty() {
        let label_rect = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(spans)).style(bar_style),
            label_rect,
        );
    }
    (regions, label_w)
}

/// Total rendered width of a span run, saturating instead of truncating
/// per-span or overflowing the `u16` sum.
fn spans_width(spans: &[Span<'_>]) -> u16 {
    spans
        .iter()
        .map(|s| u16::try_from(s.content.width()).unwrap_or(u16::MAX))
        .fold(0, u16::saturating_add)
}

fn render_status_bar_right(
    f: &mut Frame,
    area: Rect,
    bar_style: Style,
    left_content_width: u16,
    right_spans: &[Span<'static>],
) {
    if !right_spans.is_empty() {
        let right_w = spans_width(right_spans);
        // Compare against `left_content_width` (pill + session label, from Task 2),
        // not a hardcoded pill-only width -- otherwise this check passes while
        // the right segment still overlaps a rendered session label (e.g.
        // " ATTACHED" / " REMOTE ALIVE") on narrow terminals.
        let left_end = area.x + left_content_width;
        let right_x = area.x + area.width.saturating_sub(right_w);
        if right_w > 0 && right_x > left_end {
            let right_rect = Rect {
                x: right_x,
                y: area.y,
                width: right_w,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(right_spans.to_vec())).style(bar_style),
                right_rect,
            );
        }
        // else: terminal too narrow -- the right segment drops silently
        // rather than overlapping the pill or the session label.
        // (Design doc's open question on narrow-terminal truncation: right
        // segment yields first.)
    }
}

pub(in crate::app) fn render_status_bar(
    f: &mut Frame,
    area: Rect,
    model: &StatusBarModel,
) -> StatusBarRegions {
    // Keep the row itself darker so the pills read as segments sitting on top of it.
    let bar_style =
        Style::default().bg(palette::surface_colors(palette::Surface::StatusBar, false).fill);
    // `Clear` blanks every cell's symbol first (task 12.2): a bare
    // `Block::style` only recolors a cell, it never overwrites a stale
    // glyph left by whatever painted this placement before the status bar
    // owned it.
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(bar_style), area);

    let segments = status_bar_left_segments(model, area.width);
    let (regions, left_content_width) = render_status_bar_left(f, area, bar_style, segments);
    render_status_bar_right(f, area, bar_style, left_content_width, &model.right);
    regions
}
