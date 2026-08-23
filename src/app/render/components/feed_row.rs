use crate::app::palette;
use crate::app::render::screens::feeds_model::format_duration;
use crate::app::ui_util::trunc_str;
use mbv_core::playback_queue::FeedEntry;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Renders one feed entry into a `cell_w`-wide cell: selection marker
/// (painted separately, at the list's outer edge, by
/// `draw_column_selection_markers`), watched check, title, and duration.
/// The full metadata set (publish date, MIME type) lives in the hero for the
/// selected entry; list rows stay terse in both column counts, mirroring how
/// the movie/TV list rows stay terse while their hero holds the detail.
pub(in crate::app::render) fn render_feed_entry_cell(
    f: &mut Frame,
    entry: &FeedEntry,
    x: u16,
    y: u16,
    cell_w: u16,
    selected: bool,
    focused: bool,
    show_title: bool,
) {
    if cell_w == 0 {
        return;
    }
    let bg = if selected {
        palette::resolve_surface_focus(focused)
    } else {
        palette::SURFACE_BACKDROP
    };
    let fg = if selected {
        if focused {
            palette::TEXT_STRONG
        } else {
            palette::TEXT_SECONDARY
        }
    } else {
        palette::TEXT_PRIMARY
    };

    let duration = format_duration(entry.duration_ticks);
    let dur_str = if duration.is_empty() {
        String::new()
    } else {
        format!(" {duration}")
    };
    let dur_w = dur_str.chars().count();
    let prefix_w = 3usize; // leading space + watched check + space
    let title_w = (cell_w as usize).saturating_sub(prefix_w + dur_w);

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default().bg(bg))];
    spans.push(if entry.played {
        Span::styled("✓", Style::default().fg(palette::STATUS_AVAILABLE).bg(bg))
    } else {
        Span::styled(" ", Style::default().bg(bg))
    });
    let title = if show_title {
        trunc_str(&entry.title, title_w)
    } else {
        Default::default()
    };
    spans.push(Span::styled(
        format!(" {title}"),
        Style::default().fg(fg).bg(bg).add_modifier(if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        }),
    ));
    if !dur_str.is_empty() {
        spans.push(Span::styled(
            dur_str,
            Style::default().fg(palette::PLAYBACK_META_FG).bg(bg),
        ));
    }
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let pad = (cell_w as usize).saturating_sub(used);
    if pad > 0 {
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            x,
            y,
            width: cell_w,
            height: 1,
        },
    );
}
