use super::{App, LEFT_WIDTH_DEFAULT};

pub(crate) fn normalize_queue_column_width(width: u16, terminal_width: u16) -> u16 {
    width.clamp(
        LEFT_WIDTH_DEFAULT,
        LEFT_WIDTH_DEFAULT.max(terminal_width.saturating_mul(3) / 5),
    )
}

impl App {
    pub(super) fn normalize_queue_column_width(width: u16, terminal_width: u16) -> u16 {
        normalize_queue_column_width(width, terminal_width)
    }

    pub(super) fn clamp_queue_column_width(&mut self) -> bool {
        let normalized =
            Self::normalize_queue_column_width(self.queue_column_width, self.terminal_width);
        if normalized == self.queue_column_width {
            return false;
        }
        self.queue_column_width = normalized;
        true
    }
}
