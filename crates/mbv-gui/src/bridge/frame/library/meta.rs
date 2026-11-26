//! The wire form of a typed text segment (design D14). The window paints
//! each segment in its kind's colour and joins nothing it has not been
//! given.

use mbv_ui_model::meta::{MetaKind, MetaSegment};

/// One typed segment of a meta row: `{ text, kind }` on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiMetaSegmentFrame {
    /// The segment's text.
    pub text: String,
    /// The data type the text carries, which picks its colour.
    pub kind: GuiMetaKindFrame,
}

/// The data type of a meta segment, mirroring `mbv_ui_model::meta::MetaKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiMetaKindFrame {
    Plain,
    Time,
    Date,
    Count,
    Label,
    Secondary,
    Role,
}

impl From<MetaKind> for GuiMetaKindFrame {
    fn from(kind: MetaKind) -> Self {
        match kind {
            MetaKind::Plain => Self::Plain,
            MetaKind::Time => Self::Time,
            MetaKind::Date => Self::Date,
            MetaKind::Count => Self::Count,
            MetaKind::Label => Self::Label,
            MetaKind::Secondary => Self::Secondary,
            MetaKind::Role => Self::Role,
        }
    }
}

impl From<&MetaSegment> for GuiMetaSegmentFrame {
    fn from(segment: &MetaSegment) -> Self {
        Self {
            text: segment.text.clone(),
            kind: segment.kind.into(),
        }
    }
}

/// The wire segments of one typed text run, in paint order: the pills and
/// the fact rows' values (`gui-hero-design-polish` design D1).
#[must_use]
pub fn gui_meta_segments(segments: &[MetaSegment]) -> Vec<GuiMetaSegmentFrame> {
    segments.iter().map(GuiMetaSegmentFrame::from).collect()
}
