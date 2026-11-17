//! The collection destinations' one selector frame (gui-collection-dropdown
//! design D6): the Music, Podcasts, Feeds, and Emby feed-group destinations
//! paint the same dropdown over it — All, every collection in catalog order
//! with its checked flag, the destination's noun, and the date order a
//! feed-based destination sorts by. A destination with no collections (Shows,
//! a plain Emby library, Audiobooks) carries no frame, so no dropdown paints
//! there.
//!
//! The target, noun, and order are the core's own value types: they are plain
//! data the core command names unchanged, and the frame carries them as the
//! projection holds them.

use mbv::{GuiCollectionNoun, GuiCollectionTarget, GuiCollections, GuiDateOrder};

use crate::bridge::library::GuiLibraryCommand;

/// One destination's collection selector (design D6): the target its commands
/// name, the All state, every collection in catalog order, the destination's
/// noun, and the list order on a feed-based destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiCollectionsFrame {
    /// The destination the toggle, All, and date-order commands name.
    pub target: GuiCollectionTarget,
    /// Whether the selection is All.
    pub all: bool,
    /// The catalog's collections in its own order: each key, name, and
    /// checked flag.
    pub entries: Vec<GuiCollectionEntry>,
    /// The noun the trigger label and the Clear control read: Groups, Shows,
    /// Subscriptions, or Folders.
    pub noun: GuiCollectionNoun,
    /// The list's date order on a feed-based destination; `None` on Music,
    /// which sorts by name and paints no order toggle.
    pub order: Option<GuiDateOrder>,
}

/// One collection of the selector (design D6): the stable Service id the
/// toggle command names, the label the row paints, and whether the selection
/// checks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiCollectionEntry {
    /// The collection's stable Service id: the toggle command carries it.
    pub key: String,
    /// The collection's display name, the row's label.
    pub name: String,
    /// Whether the selection checks this collection.
    pub checked: bool,
}

impl GuiCollectionsFrame {
    /// The paint key the All row carries: no collection's stable Service id is
    /// empty, so the row keeps All and the entries in one key space and a
    /// press names one row's command (design D6).
    pub const ALL_KEY: &'static str = "";

    /// The selector's label (design D6): `All`, the one checked collection's
    /// name, or the count with the destination's noun.
    #[must_use]
    pub fn label(&self) -> String {
        if self.all {
            return "All".to_owned();
        }
        let checked: Vec<&GuiCollectionEntry> =
            self.entries.iter().filter(|entry| entry.checked).collect();
        match checked.as_slice() {
            [one] => one.name.clone(),
            [] => "All".to_owned(),
            many => format!("{} {}", many.len(), self.noun.word()),
        }
    }

    /// The wire command a press on the row painted with `key` sends (design
    /// D5): the All row selects All, and every other key toggles its
    /// collection.
    #[must_use]
    pub fn pressed(&self, key: &str) -> GuiLibraryCommand {
        if key == Self::ALL_KEY {
            GuiLibraryCommand::CollectionAll {
                target: self.target.clone(),
            }
        } else {
            GuiLibraryCommand::CollectionToggle {
                target: self.target.clone(),
                key: key.to_owned(),
            }
        }
    }

    /// The order a press on the date-order toggle asks for (design D4, D6):
    /// Newest switches to Oldest and Oldest to Newest. `None` on Music, whose
    /// frame carries no order and which paints no toggle.
    #[must_use]
    pub fn next_order(&self) -> Option<GuiDateOrder> {
        self.order.map(|order| match order {
            GuiDateOrder::Newest => GuiDateOrder::Oldest,
            GuiDateOrder::Oldest => GuiDateOrder::Newest,
        })
    }

    /// The date-order toggle's label (design D6): Newest or Oldest. `None` on
    /// Music, which paints no toggle.
    #[must_use]
    pub fn order_label(&self) -> Option<&'static str> {
        self.order.map(|order| match order {
            GuiDateOrder::Newest => "Newest",
            GuiDateOrder::Oldest => "Oldest",
        })
    }
}

impl From<&GuiCollections> for GuiCollectionsFrame {
    fn from(collections: &GuiCollections) -> Self {
        Self {
            target: collections.target.clone(),
            all: collections.all,
            entries: collections
                .entries
                .iter()
                .map(|entry| GuiCollectionEntry {
                    key: entry.key.clone(),
                    name: entry.name.clone(),
                    checked: entry.checked,
                })
                .collect(),
            noun: collections.noun,
            order: collections.order,
        }
    }
}
