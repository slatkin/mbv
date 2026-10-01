use super::palette;

mod header;
mod queue_band;
mod title_row;
mod transport;

pub use header::{HeaderTitle, playback_state_icon, render_header_title};
pub(super) use queue_band::{QueueBand, blank_row, render_queue_band};
pub use title_row::{marquee_spans, render_title_row};
