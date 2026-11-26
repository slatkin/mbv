//! The wire form of the GUI Hero's facts (gui-hero-design-polish design D1):
//! the pills and labelled rows the D1 builders split, and the movie cast
//! (design D3) with its photo reference. The window paints the label word
//! from the label enum and joins nothing it has not been given.

use crate::art::GuiArtRef;
use crate::gui_core::{
    GuiHeroCastMember, GuiHeroFactLabel, GuiHeroFactRow, GuiHeroFacts, GuiHeroPill,
};

use super::meta::{GuiMetaSegmentFrame, gui_meta_segments};

/// One Hero's facts (design D1): the header band's pills and the detail
/// block's labelled rows, each in paint order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHeroFactsFrame {
    /// One short fact per pill, in paint order (the header band's facts
    /// line).
    pub pills: Vec<GuiHeroPillFrame>,
    /// One labelled fact per row, in paint order (the detail block).
    pub rows: Vec<GuiHeroFactRowFrame>,
}

/// One short Hero fact (design D1): its typed segments, so the pill's text
/// and type colour stay as today, and the optional URL a link pill opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHeroPillFrame {
    /// The pill's typed runs of text.
    pub segments: Vec<GuiMetaSegmentFrame>,
    /// The URL a link pill opens (`None` on a plain fact pill).
    pub link: Option<String>,
}

/// One labelled Hero fact row (design D1): the window paints the label word
/// from the label enum, and the value through its typed segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHeroFactRowFrame {
    /// Which label word the row paints.
    pub label: GuiHeroFactLabelFrame,
    /// The row's typed runs of text.
    pub value: Vec<GuiMetaSegmentFrame>,
}

/// The label word of one Hero fact row (design D1), mirroring
/// `crate::gui_core::GuiHeroFactLabel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiHeroFactLabelFrame {
    /// The movie's directors, joined with `, `.
    Director,
    /// The item's release date.
    Released,
    /// The movie's studios, joined with `, `.
    Studio,
    /// The series' network: its first studio.
    Network,
    /// The audiobook's author.
    Author,
    /// The podcast episode's or the feed entry's publish date.
    Published,
}

/// One member of a movie's cast section (design D3), mirroring
/// `crate::gui_core::GuiHeroCastMember`: the person's name and role, the
/// initials the photo's round box falls back to, and the photo reference the
/// window paints — `None` when the person carries no photo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHeroCastMemberFrame {
    /// The person's name.
    pub name: String,
    /// The role painted beside the name: the person's role, else their kind.
    pub role: String,
    /// The initials the photo box paints when no photo shows: the first
    /// letter of the name's first two words, upper case.
    pub initials: String,
    /// The person's Primary photo as the window paints it. `None` with no id
    /// or no tag; `Some` holds the fetch's loading state while it runs.
    pub photo: Option<GuiArtRef>,
}

impl From<&GuiHeroFacts> for GuiHeroFactsFrame {
    fn from(facts: &GuiHeroFacts) -> Self {
        Self {
            pills: facts.pills.iter().map(GuiHeroPillFrame::from).collect(),
            rows: facts.rows.iter().map(GuiHeroFactRowFrame::from).collect(),
        }
    }
}

impl From<&GuiHeroPill> for GuiHeroPillFrame {
    fn from(pill: &GuiHeroPill) -> Self {
        Self {
            segments: gui_meta_segments(&pill.segments),
            link: pill.link.clone(),
        }
    }
}

impl From<&GuiHeroFactRow> for GuiHeroFactRowFrame {
    fn from(row: &GuiHeroFactRow) -> Self {
        Self {
            label: GuiHeroFactLabelFrame::from(row.label),
            value: gui_meta_segments(&row.value),
        }
    }
}

impl From<GuiHeroFactLabel> for GuiHeroFactLabelFrame {
    fn from(label: GuiHeroFactLabel) -> Self {
        match label {
            GuiHeroFactLabel::Director => Self::Director,
            GuiHeroFactLabel::Released => Self::Released,
            GuiHeroFactLabel::Studio => Self::Studio,
            GuiHeroFactLabel::Network => Self::Network,
            GuiHeroFactLabel::Author => Self::Author,
            GuiHeroFactLabel::Published => Self::Published,
        }
    }
}

impl From<&GuiHeroCastMember> for GuiHeroCastMemberFrame {
    fn from(member: &GuiHeroCastMember) -> Self {
        Self {
            name: member.name.clone(),
            role: member.role.clone(),
            initials: member.initials.clone(),
            photo: member.photo.as_ref().map(GuiArtRef::from),
        }
    }
}
