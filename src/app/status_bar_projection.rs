//! The Status bar projection every Client consumes (`add-gui-front-end` design
//! D5, row 6.5): the prefix-armed display and the live toast the status row
//! presents, computed once from the `App` so both Clients read the same
//! delivery and expiry rules. The Status bar's Surface state adopts it each
//! tick.

use mbv_ui_model::toast::StatusToast;

use crate::app::App;

/// The Status bar's facts for one tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusBarProjection {
    /// Whether prefix mode is armed. The armed indicator paints only while
    /// armed.
    pub prefix_armed: bool,
    /// The live toast the row presents, after delivery and expiry gating
    /// (`toast-notification-semantics`); `None` presents none.
    pub toast: Option<StatusToast>,
}

impl App {
    /// Project the Status bar's facts (design D5). Read-only.
    #[must_use]
    pub fn status_bar_projection(&self) -> StatusBarProjection {
        StatusBarProjection {
            prefix_armed: self.prefix_armed,
            toast: self.toast_presentation(),
        }
    }
}
