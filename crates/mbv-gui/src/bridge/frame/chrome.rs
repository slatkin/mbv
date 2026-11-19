//! The Queue footer and Status bar sections (design D4). Each type
//! mirrors one library projection, so the window paints what the Clients
//! paint from the same rules.

use mbv::{FooterPill, QueueFooterProjection, StatusBarProjection};
use mbv_ui_model::toast::{ToastCard, ToastId, ToastSeverity};

use super::theme::GuiRole;

/// The Queue footer's status pills, mirroring `QueueFooterProjection`. The
/// footer line carries no Player-owner label and no liveness dot
/// (gui-named-queues task 8.5, design D9), so neither crosses the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiQueueFooter {
    /// The playlist source pill.
    pub playlist: GuiFooterPill,
    /// The autosave indicator. Present only while save-on-consume is on for a
    /// saved playlist.
    pub autosave: Option<GuiFooterPill>,
    /// The dirty indicator. Present only while the queue differs from its saved
    /// playlist.
    pub dirty: Option<GuiFooterPill>,
}

impl From<&QueueFooterProjection> for GuiQueueFooter {
    fn from(projection: &QueueFooterProjection) -> Self {
        Self {
            playlist: GuiFooterPill::from(&projection.playlist),
            autosave: projection.autosave.as_ref().map(GuiFooterPill::from),
            dirty: projection.dirty.as_ref().map(GuiFooterPill::from),
        }
    }
}

/// One footer pill: its glyph-free text (design D8 — no emoji or Nerd Font
/// glyph reaches the window, which maps each pill's slot to its Lucide
/// icon), the foreground role it paints in, and whether the text is
/// emphasised. Mirrors `FooterPill`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiFooterPill {
    pub text: String,
    pub role: GuiRole,
    pub bold: bool,
}

impl From<&FooterPill> for GuiFooterPill {
    fn from(pill: &FooterPill) -> Self {
        Self {
            text: pill.label.clone(),
            role: GuiRole::from(pill.role),
            bold: pill.bold,
        }
    }
}

/// The Status bar's facts for one section value, mirroring `StatusBarProjection`.
/// The toasts are not among them (design D6 of `gui-floating-toasts`): they
/// ride their own section, [`GuiToasts`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiStatusBar {
    /// Whether prefix mode is armed. The armed indicator paints only while armed.
    pub prefix_armed: bool,
}

impl From<&StatusBarProjection> for GuiStatusBar {
    fn from(projection: &StatusBarProjection) -> Self {
        Self {
            prefix_armed: projection.prefix_armed,
        }
    }
}

/// The live toasts of one section value, oldest first: one card per toast the
/// Client core keeps live and renders in-app. Mirrors `App::toast_cards`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuiToasts(pub Vec<GuiToast>);

impl From<Vec<ToastCard>> for GuiToasts {
    fn from(cards: Vec<ToastCard>) -> Self {
        Self(cards.iter().map(GuiToast::from).collect())
    }
}

impl GuiToasts {
    /// The newest toast, `None` while none is live: the card of the last
    /// arrival. A status line that still paints a toast inline reads it here
    /// until it drops that paint (task row 2.4).
    #[must_use]
    pub fn newest(&self) -> Option<&GuiToast> {
        self.0.last()
    }
}

/// A live toast: its stable id, text, and severity. Mirrors `ToastCard`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiToast {
    /// The stable identity the card is keyed on.
    pub id: ToastId,
    /// The toast's text.
    pub text: String,
    /// The toast's severity class.
    pub severity: GuiToastSeverity,
}

impl From<&ToastCard> for GuiToast {
    fn from(card: &ToastCard) -> Self {
        Self {
            id: card.id,
            text: card.toast.text.clone(),
            severity: GuiToastSeverity::from(card.toast.severity),
        }
    }
}

/// The severity class of a toast, mirroring `ToastSeverity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiToastSeverity {
    Neutral,
    Success,
    Warning,
    Error,
}

impl From<ToastSeverity> for GuiToastSeverity {
    fn from(severity: ToastSeverity) -> Self {
        match severity {
            ToastSeverity::Neutral => Self::Neutral,
            ToastSeverity::Success => Self::Success,
            ToastSeverity::Warning => Self::Warning,
            ToastSeverity::Error => Self::Error,
        }
    }
}
