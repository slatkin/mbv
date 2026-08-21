use ratatui::layout::Rect;

pub(in crate::app::render) mod hero_left;
pub(in crate::app::render) mod home;
pub(in crate::app::render) mod library;
pub(in crate::app::render) mod music;

/// Inset a `Rect` by symmetric horizontal/vertical padding: `pad_x` on each
/// side, `pad_y` top and bottom. Shared by every arrangement that pads a
/// panel into a content area.
pub(in crate::app::render) fn padded_rect(area: Rect, pad_x: u16, pad_y: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(pad_x),
        y: area.y.saturating_add(pad_y),
        width: area.width.saturating_sub(pad_x * 2),
        height: area.height.saturating_sub(pad_y * 2),
    }
}
