//! The Status bar's Surface state (add-gui-front-end design D5, row 4.6):
//! the prefix-armed display and the toast presentation the status row owns,
//! front-end-neutral. The TUI and GUI Clients hold this state and paint from
//! it; neither reimplements a rule.

use crate::HitMap;
use mbv_ui_model::toast::StatusToast;

/// The semantic target of one painted Status bar region (design D6): what a
/// pointer press or wheel step on the region resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarHitTarget {
    /// The volume pill: a wheel step adjusts the volume.
    VolumePill,
    /// The mute pill: a press toggles mute.
    MutePill,
    /// The visual-mode indicator: a press clears the multi-selection.
    VisualClear,
}

/// One frame's Status bar interaction report (design D6): the painted regions
/// a pointer event resolves against. A Client derives it from its own painted
/// geometry and hands it to the Client core; the core reads it and never
/// computes geometry itself. The Status bar has no shared list, so the report
/// carries no capacity.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBarFrameReport {
    /// The frame's painted pill regions and their semantic targets.
    pub hit_map: HitMap<StatusBarHitTarget>,
}

/// The Status bar's Surface state: the prefix-armed display state and the
/// live toast presentation. The armed flag is the display decision's whole
/// input (the armed indicator paints only while armed); the toast carries the
/// presented text and severity, after the projecting Client's delivery and
/// expiry gating.
#[derive(Debug, Default)]
pub struct StatusBarSurface {
    prefix_armed: bool,
    toast: Option<StatusToast>,
}

impl StatusBarSurface {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Project the prefix-armed display state.
    pub fn set_prefix_armed(&mut self, armed: bool) {
        self.prefix_armed = armed;
    }

    /// Whether the status row presents the armed indicator.
    #[must_use]
    pub const fn prefix_armed(&self) -> bool {
        self.prefix_armed
    }

    /// Project the live toast presentation; `None` presents no toast.
    pub fn set_toast(&mut self, toast: Option<StatusToast>) {
        self.toast = toast;
    }

    /// The live toast presentation the painters style, if one is presented.
    #[must_use]
    pub const fn toast(&self) -> Option<&StatusToast> {
        self.toast.as_ref()
    }
}
