#[cfg(test)]
use super::list_rows::SELECTED_BLOCK_SIDE_PADDING;
#[cfg(test)]
use crate::app::palette;
#[cfg(test)]
use crate::app::ui_util::*;
#[cfg(test)]
use ratatui::layout::*;
#[cfg(test)]
use ratatui::style::*;
#[cfg(test)]
use ratatui::text::*;
#[cfg(test)]
use ratatui::widgets::*;
#[cfg(test)]
use ratatui::Frame;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Parses a leading `YYYY-MM-DD` date out of `date_str` (an Emby date field,
/// which may carry a `T...` time/offset suffix that's ignored here) and
/// returns its `(year, month name, day)`, or `None` if it doesn't parse.
fn parse_ymd(date_str: &str) -> Option<(&str, &'static str, u32)> {
    let date_part = date_str.split('T').next().unwrap_or(date_str);
    let parts: Vec<&str> = date_part.splitn(3, '-').collect();
    let [y, m, d] = parts.as_slice() else {
        return None;
    };
    let day: u32 = d.parse().ok()?;
    let month_idx: usize = m.parse::<usize>().ok()?.checked_sub(1)?;
    Some((y, MONTHS.get(month_idx)?, day))
}

/// Formats an Emby `PremiereDate` value (e.g. `2015-06-19T00:00:00.0000000Z`)
/// as a release date like "19 Jun 2015".
pub(in crate::app::render) fn format_release_date(premiere_date: &str) -> String {
    parse_ymd(premiere_date)
        .map(|(y, month, d)| format!("{d} {} {y}", &month[..3]))
        .unwrap_or_else(|| premiere_date.to_string())
}

#[cfg(test)]
pub(in crate::app::render) fn render_home_video_item(
    f: &mut Frame,
    item: &mbv_core::api::EmbyItem,
    row_y: u16,
    item_h: u16,
    content_area: Rect,
    text_w: usize,
    selected: bool,
    focused: bool,
) {
    let expanded = selected && item_h > 1;
    let title_y = row_y + if expanded { 1 } else { 0 };

    if expanded {
        // The expanded item's own fill spans its whole cell, its top and
        // bottom spacer rows included.
        super::widgets::fill_surface(
            f,
            Rect {
                x: content_area.x,
                y: row_y,
                width: text_w as u16,
                height: item_h,
            },
            palette::Surface::InlineHero,
            focused,
        );
    }

    // The selected row's background treatment alone marks selection.
    let text_inset = if selected {
        SELECTED_BLOCK_SIDE_PADDING
    } else {
        0
    };
    let tx = content_area.x + text_inset;
    let tw = (text_w as u16).saturating_sub(2 * text_inset);
    let title_color = if expanded {
        palette::TEXT_FOCUS_ACCENT
    } else if selected && focused {
        palette::ACCENT_ACTIVE
    } else if focused {
        // The unselected-unfocused title role (was the shared
        // `focused_or_subtle` helper, inlined when its last consumer became
        // test-only with the legacy painters' removal).
        palette::TEXT_EMPHASIS
    } else {
        palette::TEXT_SECONDARY
    };
    let title_style = if selected && focused {
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(title_color)
    };
    let title_trunc = trunc_str(&item.display_name(), tw as usize);
    f.render_widget(
        Paragraph::new(Span::styled(title_trunc, title_style)),
        Rect {
            x: tx,
            y: title_y,
            width: tw,
            height: 1,
        },
    );
}
