use super::super::{palette, App};
use mbv_visualizer::StereoSample;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::Frame;
use std::collections::HashSet;

const SILENCE_THRESHOLD: f32 = 0.0001;
const DISPLAY_GAIN: f32 = 4.0;

impl App {
    pub(in crate::app::render) fn render_visualizer(&self, f: &mut Frame, area: Rect, bg: Color) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let bg_style = Style::default().bg(bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                    cell.set_symbol(" ").set_style(bg_style);
                }
            }
        }
        let inner = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(2),
        };
        if inner.width == 0 || inner.height == 0 || is_silent(&self.visualizer_window.samples) {
            return;
        }

        let mut seen = HashSet::new();
        for sample in &self.visualizer_window.samples {
            let Some((x, y)) = sample_to_cell(*sample, inner.width, inner.height) else {
                continue;
            };
            if !seen.insert((x, y)) {
                continue;
            }
            if let Some(cell) = f.buffer_mut().cell_mut((inner.x + x, inner.y + y)) {
                cell.set_symbol(&self.visualizer_glyph);
                cell.set_style(Style::default().fg(point_color(*sample)).bg(bg));
            }
        }
    }
}

fn is_silent(samples: &[StereoSample]) -> bool {
    samples.iter().all(|sample| {
        sample.left.abs() <= SILENCE_THRESHOLD && sample.right.abs() <= SILENCE_THRESHOLD
    })
}

fn sample_to_cell(sample: StereoSample, width: u16, height: u16) -> Option<(u16, u16)> {
    if width == 0 || height == 0 {
        return None;
    }
    let left = (sample.left * DISPLAY_GAIN).clamp(-1.0, 1.0);
    let right = (sample.right * DISPLAY_GAIN).clamp(-1.0, 1.0);
    let center_x = width / 2;
    let center_y = height / 2;
    let positive_x = width.saturating_sub(center_x.saturating_add(1));
    let positive_y = height.saturating_sub(center_y.saturating_add(1));
    let x = f32::from(center_x)
        + if left < 0.0 {
            left * f32::from(center_x)
        } else {
            left * f32::from(positive_x)
        };
    let y = f32::from(center_y)
        + if right < 0.0 {
            right * f32::from(center_y)
        } else {
            right * f32::from(positive_y)
        };
    Some((
        rounded_coordinate(x, width - 1),
        rounded_coordinate(y, height - 1),
    ))
}

fn rounded_coordinate(value: f32, max: u16) -> u16 {
    let rounded = value.round().clamp(0.0, f32::from(max));
    let (mut low, mut high) = (0, max);
    while low < high {
        let middle = low + (high - low) / 2;
        if f32::from(middle) < rounded {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

fn point_color(sample: StereoSample) -> Color {
    match sample.left.abs().max(sample.right.abs()) * DISPLAY_GAIN {
        amplitude if amplitude < 0.25 => palette::ACCENT,
        amplitude if amplitude < 0.5 => palette::TEXT_METADATA,
        amplitude if amplitude < 0.75 => palette::TEXT_FOCUS_ACCENT,
        _ => palette::STATUS_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::rounded_coordinate;

    #[test]
    fn rounded_coordinate_matches_round_then_clamp() {
        assert_eq!(rounded_coordinate(-1.0, 10), 0);
        assert_eq!(rounded_coordinate(2.5, 10), 3);
        assert_eq!(rounded_coordinate(99.0, 10), 10);
        assert_eq!(rounded_coordinate(f32::NAN, 10), 0);
    }
}
