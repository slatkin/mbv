use super::super::{palette, App};
use crate::app::infra::visualizer_worker::StereoSample;
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
    let x = center_x as f32
        + if left < 0.0 {
            left * center_x as f32
        } else {
            left * positive_x as f32
        };
    let y = center_y as f32
        + if right < 0.0 {
            right * center_y as f32
        } else {
            right * positive_y as f32
        };
    Some((
        (x.round() as i32).clamp(0, width as i32 - 1) as u16,
        (y.round() as i32).clamp(0, height as i32 - 1) as u16,
    ))
}

fn point_color(sample: StereoSample) -> Color {
    match sample.left.abs().max(sample.right.abs()) * DISPLAY_GAIN {
        amplitude if amplitude < 0.25 => palette::ACCENT,
        amplitude if amplitude < 0.5 => palette::TEXT_METADATA,
        amplitude if amplitude < 0.75 => palette::TEXT_FOCUS_ACCENT,
        _ => palette::STATUS_ERROR,
    }
}
