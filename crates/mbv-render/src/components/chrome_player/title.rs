use super::palette;

mod header;
mod queue_band;
mod title_row;
mod transport;

#[doc(inline)]
pub use header::{HeaderTitle, remote_state_icon, render_header_title};
pub(super) use queue_band::{QueueBand, blank_row, render_queue_band};
#[doc(inline)]
pub use title_row::{marquee_spans, render_title_row};
