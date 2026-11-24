//! The tab bar's overflow window: which tabs fit the strip, and where the
//! chevrons scroll it. The strip holds one run of whole tabs. A tab that does
//! not fit is not painted; a chevron at the strip's edge steps the first
//! painted tab by one, as the TUI's tab bar scrolls its anchor
//! (`mbv-render`'s `visible_tab_range`). The widths are text measures, so the
//! math is pure and needs no layout pass.

use crate::bridge::frame::library::GuiLibraryTabFrame;
use crate::window::truncate::{TextStyle, text_width};

/// The width a chevron button takes in the strip, in CSS pixels.
pub const CHEVRON_WIDTH: f32 = 24.0;

/// The tab's horizontal padding (both sides), the current tab's underline
/// run padding, and the strip's gap, in CSS pixels.
const TAB_CHROME: f32 = 12.0 * 2.0 + 2.0 + 2.0;

/// The run of tabs the strip paints and the chevrons it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabWindow {
    /// The first painted tab's index.
    pub start: usize,
    /// One past the last painted tab's index.
    pub end: usize,
    /// Whether tabs before `start` are hidden, so the left chevron paints.
    pub left: bool,
    /// Whether tabs from `end` on are hidden, so the right chevron paints.
    pub right: bool,
}

/// The painted width of one tab: the mono face at the bar's one tab size,
/// uppercased as the strip paints it. Every tab measures the same way, so a
/// change of the current tab never moves the window.
#[must_use]
pub fn tab_width(tab: &GuiLibraryTabFrame) -> f32 {
    let style = TextStyle::mono(14.0, 600.0).tracking(0.08);
    text_width(&tab.label.to_uppercase(), &style) + TAB_CHROME
}

/// The widths of `tabs`, in order.
#[must_use]
pub fn tab_widths(tabs: &[GuiLibraryTabFrame]) -> Vec<f32> {
    tabs.iter().map(tab_width).collect()
}

/// The tabs that fit `avail` CSS pixels from `start`. A width of zero or less
/// is a strip the layout has not measured yet: every tab paints and the
/// strip clips. At least one tab always paints.
#[must_use]
pub fn tab_window(widths: &[f32], start: usize, avail: f32) -> TabWindow {
    let count = widths.len();
    if avail <= 0.0 {
        return TabWindow {
            start: 0,
            end: count,
            left: false,
            right: false,
        };
    }
    let start = start.min(count.saturating_sub(1));
    let left = start > 0;
    let fit = |room: f32| {
        let mut used = 0.0;
        let mut end = start;
        while end < count && (end == start || used + widths[end] <= room) {
            used += widths[end];
            end += 1;
        }
        end
    };
    let room = avail - if left { CHEVRON_WIDTH } else { 0.0 };
    let mut end = fit(room);
    let right = end < count;
    if right {
        end = fit(room - CHEVRON_WIDTH);
    }
    TabWindow {
        start,
        end,
        left,
        right,
    }
}

/// The first painted tab that keeps tab `current` painted: `start` itself
/// when the window from it already holds `current`.
#[must_use]
pub fn start_showing(widths: &[f32], current: usize, start: usize, avail: f32) -> usize {
    let mut start = start.min(current);
    while tab_window(widths, start, avail).end <= current && start < current {
        start += 1;
    }
    start
}

#[cfg(test)]
mod tests;
