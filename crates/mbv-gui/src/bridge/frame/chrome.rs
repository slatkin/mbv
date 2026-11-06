//! The Queue footer and Status bar sections of a frame (design D4). Each type
//! mirrors one library projection, so the front end paints what the Clients
//! paint from the same rules.

use mbv::{FooterPill, QueueFooterProjection, StatusBarProjection};
use mbv_ui_model::toast::{StatusToast, ToastSeverity};
use serde::Serialize;

use super::theme::GuiRole;

/// The Queue footer's status pills, mirroring `QueueFooterProjection`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiQueueFooter {
    /// The owner indicator, always present.
    pub owner: GuiFooterPill,
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
            owner: GuiFooterPill::from(&projection.owner),
            playlist: GuiFooterPill::from(&projection.playlist),
            autosave: projection.autosave.as_ref().map(GuiFooterPill::from),
            dirty: projection.dirty.as_ref().map(GuiFooterPill::from),
        }
    }
}

/// One footer pill: its glyph-free text (design D8 — no emoji or Nerd Font
/// glyph reaches the front end, which maps each pill's slot to its Lucide
/// icon), the foreground role it paints in, and whether the text is
/// emphasised. Mirrors `FooterPill`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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

/// The Status bar's facts for one frame, mirroring `StatusBarProjection`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiStatusBar {
    /// Whether prefix mode is armed. The armed indicator paints only while armed.
    pub prefix_armed: bool,
    /// The live toast that the row presents, after delivery and expiry gating.
    pub toast: Option<GuiToast>,
}

impl From<&StatusBarProjection> for GuiStatusBar {
    fn from(projection: &StatusBarProjection) -> Self {
        Self {
            prefix_armed: projection.prefix_armed,
            toast: projection.toast.as_ref().map(GuiToast::from),
        }
    }
}

/// A live toast: its text and severity. Mirrors `StatusToast`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiToast {
    pub text: String,
    pub severity: GuiToastSeverity,
}

impl From<&StatusToast> for GuiToast {
    fn from(toast: &StatusToast) -> Self {
        Self {
            text: toast.text.clone(),
            severity: GuiToastSeverity::from(toast.severity),
        }
    }
}

/// The severity class of a toast, mirroring `ToastSeverity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
