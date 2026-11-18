//! The collection destinations' one selector frame (gui-collection-dropdown
//! design D6): the Music, Podcasts, Feeds, and Emby feed-group destinations
//! paint the same dropdown over it — All, every collection in catalog order
//! with its checked flag, the destination's noun, and the date order a
//! feed-based destination sorts by. A destination with no collections (Shows,
//! a plain Emby library, Audiobooks) carries no frame, so no dropdown paints
//! there.
//!
//! The frame is the core's own value type, [`GuiCollections`], which the
//! projection builds and the frame carries as it is; the paint and press
//! helpers ride an extension trait over it, because the core type sits below
//! this crate and cannot carry the window's command vocabulary.

use mbv::GuiDateOrder;

use crate::bridge::library::GuiLibraryCommand;

pub use mbv::{GuiCollectionEntry, GuiCollections};

/// The paint key the All row carries: no collection's stable Service id is
/// empty, so the row keeps All and the entries in one key space and a press
/// names one row's command (design D6).
pub const ALL_KEY: &str = "";

/// The frame-shaped helpers over the core's collection selector (design D5,
/// D6): the label the trigger reads, the wire command a row press sends, and
/// the date-order toggle's press and label.
pub trait GuiCollectionsFrame {
    /// The selector's label (design D6): `All`, the one checked collection's
    /// name, or the count with the destination's noun.
    fn label(&self) -> String;

    /// The wire command a press on the row painted with `key` sends (design
    /// D5): the All row selects All, and every other key toggles its
    /// collection.
    fn pressed(&self, key: &str) -> GuiLibraryCommand;

    /// The order a press on the date-order toggle asks for (design D4, D6):
    /// Newest switches to Oldest and Oldest to Newest. `None` on Music, whose
    /// frame carries no order and which paints no toggle.
    fn next_order(&self) -> Option<GuiDateOrder>;

    /// The date-order toggle's label (design D6): Newest or Oldest. `None` on
    /// Music, which paints no toggle.
    fn order_label(&self) -> Option<&'static str>;
}

impl GuiCollectionsFrame for GuiCollections {
    fn label(&self) -> String {
        if self.all {
            return "All".to_owned();
        }
        let checked: Vec<&mbv::GuiCollectionEntry> =
            self.entries.iter().filter(|entry| entry.checked).collect();
        match checked.as_slice() {
            [one] => one.name.clone(),
            [] => "All".to_owned(),
            many => format!("{} {}", many.len(), self.noun.word()),
        }
    }

    fn pressed(&self, key: &str) -> GuiLibraryCommand {
        if key == ALL_KEY {
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

    fn next_order(&self) -> Option<GuiDateOrder> {
        self.order.map(|order| match order {
            GuiDateOrder::Newest => GuiDateOrder::Oldest,
            GuiDateOrder::Oldest => GuiDateOrder::Newest,
        })
    }

    fn order_label(&self) -> Option<&'static str> {
        self.order.map(|order| match order {
            GuiDateOrder::Newest => "Newest",
            GuiDateOrder::Oldest => "Oldest",
        })
    }
}
