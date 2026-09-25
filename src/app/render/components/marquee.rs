use ratatui::style::{Color, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// `key` is the stable state key. Must match the text the caller looked the
/// marquee state up with (the media-list painter keys on the row's full
/// marqueed title text), or the start time resets every frame and the marquee
/// never advances. `force_scroll` scrolls a title that already fits its
/// window instead of returning it static, for a list that reveals its row
/// titles only on the selected row.
///
/// Reused beyond the media-list painters by the Grouped Music tree adapter's
/// label renderer (the crate label-renderer seam permits it; design D8 of
/// `add-grouped-music-tree-browser`).
pub(in crate::app) fn marquee_spans(
    key: &str,
    parts: &[(&str, Color)],
    max_width: usize,
    marquee_text: &mut String,
    marquee_started_at: &mut std::time::Instant,
    force_scroll: bool,
) -> Vec<Span<'static>> {
    let total_width: usize = parts.iter().map(|(text, _)| text.width()).sum();
    // A zero budget (the row fully consumed by reserved slots) yields an
    // empty window — never the untruncated parts.
    if total_width <= max_width && !force_scroll {
        return parts
            .iter()
            .map(|(text, color)| Span::styled(text.to_string(), Style::default().fg(*color)))
            .collect();
    }
    if *marquee_text != key {
        marquee_text.clear();
        marquee_text.push_str(key);
        *marquee_started_at = std::time::Instant::now();
    }
    // A forced scroll of a title that already fits travels the title's own
    // width, so it leaves the window entirely and returns; an overflowing
    // title travels only its excess.
    let travel = if force_scroll && total_width <= max_width {
        total_width
    } else {
        total_width.saturating_sub(max_width)
    };
    colored_width_window(
        parts,
        marquee_col(travel, marquee_started_at.elapsed().as_millis()),
        max_width,
    )
}

fn marquee_col(overflow: usize, elapsed_ms: u128) -> usize {
    if overflow == 0 {
        return 0;
    }
    const STEP_MS: u128 = 150;
    const HOLD_MS: u128 = 600;
    let scroll_ms = overflow as u128 * STEP_MS;
    let cycle = 2 * HOLD_MS + 2 * scroll_ms;
    let t = elapsed_ms % cycle;
    if t < HOLD_MS {
        0
    } else if t < HOLD_MS + scroll_ms {
        ((t - HOLD_MS) / STEP_MS) as usize
    } else if t < 2 * HOLD_MS + scroll_ms {
        overflow
    } else {
        overflow - ((t - (2 * HOLD_MS + scroll_ms)) / STEP_MS) as usize
    }
}

fn colored_width_window(
    parts: &[(&str, Color)],
    start_col: usize,
    width: usize,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut col = 0usize;
    let mut taken = 0usize;
    let mut current: Option<(String, Color)> = None;
    'parts: for (text, color) in parts {
        for c in text.chars() {
            let char_width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if col + char_width <= start_col {
                col += char_width;
                continue;
            }
            if taken + char_width > width {
                break 'parts;
            }
            match &mut current {
                Some((value, current_color)) if current_color == color => value.push(c),
                _ => {
                    if let Some((value, current_color)) = current.take() {
                        spans.push(Span::styled(value, Style::default().fg(current_color)));
                    }
                    current = Some((c.to_string(), *color));
                }
            }
            taken += char_width;
            col += char_width;
        }
    }
    if let Some((value, color)) = current {
        spans.push(Span::styled(value, Style::default().fg(color)));
    }
    spans
}

#[cfg(test)]
mod tests {
    #[test]
    fn marquee_advances_five_columns_per_second() {
        assert_eq!(super::marquee_col(10, 600 + 150 * 5), 5);
    }
}
