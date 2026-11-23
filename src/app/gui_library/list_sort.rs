//! The GUI Library panel's one list sort (gui-flat-list-sort design D1, D2,
//! D3): the single-choice order every flat list carries, the target each
//! list's sort is keyed by, and the choices and default of each target. The
//! store sits beside them (`App::list_sorts`), keyed by the
//! destination's stable identity, because a sort is sticky per destination
//! and survives a restart through the launch file.

use std::cmp::Ordering;

use mbv_config::TabIdentity;

use super::collection::{GuiDateOrder, compare_gui_date_order};
use super::projection::sort_letter;
use super::tabs::GuiDestinationKind;
use crate::app::App;

/// The sorts of every target but Audiobooks (design D2), in paint order.
const THREE_SORTS: [GuiListSort; 3] =
    [GuiListSort::Title, GuiListSort::Newest, GuiListSort::Oldest];

/// The Audiobooks target's sorts (design D2), in paint order.
const FOUR_SORTS: [GuiListSort; 4] = [
    GuiListSort::Title,
    GuiListSort::Author,
    GuiListSort::Newest,
    GuiListSort::Oldest,
];

/// The single-choice order of a flat GUI Library panel list (design D1):
/// exactly one entry is active per destination. Title and Author are the name
/// sorts, whose lists show letter headings and the letter rail; Newest and
/// Oldest are the date sorts, which delegate to the date comparator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiListSort {
    /// The row's title, ascending.
    Title,
    /// A book's author sort key, then its title.
    Author,
    /// The most recently added row first.
    Newest,
    /// The earliest added row first.
    Oldest,
}

impl GuiListSort {
    /// The control's text (design D7): the core owns it, so the two layers
    /// cannot drift.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Author => "Author",
            Self::Newest => "Newest",
            Self::Oldest => "Oldest",
        }
    }

    /// Compare two row dates under this sort (design D1): Newest and Oldest
    /// delegate to [`compare_gui_date_order`], and the name sorts order no
    /// two dates apart, so they compare `Equal`.
    #[must_use]
    pub fn compare_date(self, a: Option<u64>, b: Option<u64>) -> Ordering {
        match self {
            Self::Title | Self::Author => Ordering::Equal,
            Self::Newest => compare_gui_date_order(a, b, GuiDateOrder::Newest),
            Self::Oldest => compare_gui_date_order(a, b, GuiDateOrder::Oldest),
        }
    }

    /// The rail letter of a row whose name key is `key` (design D6): the
    /// key's letter while the sort is a name sort, and none under a date
    /// sort, so the rail follows the one fact the core derives.
    #[must_use]
    pub(in crate::app::gui_library) fn rail_letter(self, key: &str) -> Option<char> {
        self.is_name_sort().then(|| sort_letter(key))
    }

    /// Whether the sort is a name sort (design D6): the list shows letter
    /// headings and the letter rail exactly while this is true.
    #[must_use]
    pub const fn is_name_sort(self) -> bool {
        matches!(self, Self::Title | Self::Author)
    }
}

/// The key a Title sort compares (design D6): the title or sort name
/// lowercased, so two keys that differ only in case tie.
#[must_use]
pub(in crate::app::gui_library) fn gui_title_sort_key(name: &str) -> String {
    name.to_lowercase()
}

/// The flat list one sort belongs to (design D2), as the commands and the
/// frames name it: the index is the library's index in the core's catalog,
/// Emby's `libs` or the Audiobookshelf libraries', as the destination kinds
/// key them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiSortTarget {
    /// A plain Emby library's list, or that library's feed-group list: the
    /// two modes share the choices and the identity, and only the default
    /// differs.
    Library(usize),
    /// The Audiobookshelf book library's Audiobooks list, at the
    /// Audiobookshelf library index.
    Audiobooks(usize),
    /// The Audiobookshelf podcast library's Podcasts list, at the
    /// Audiobookshelf library index.
    Podcasts(usize),
    /// The one Feeds tab's entry list.
    Feeds,
    /// The Shows destination's New view, at the Emby library index.
    ShowsNew(usize),
}

impl GuiSortTarget {
    /// The sorts the target's control offers, in paint order (design D2):
    /// Audiobooks offers Author too, and every other target offers the three.
    #[must_use]
    pub const fn choices(self) -> &'static [GuiListSort] {
        match self {
            Self::Library(_) | Self::Podcasts(_) | Self::Feeds | Self::ShowsNew(_) => &THREE_SORTS,
            Self::Audiobooks(_) => &FOUR_SORTS,
        }
    }

    /// The sort the target's list reads when the store holds none (design
    /// D2). `feed_group` says whether a `Library` target is its feed-group
    /// mode, which shares the plain mode's choices but not its default; the
    /// other targets read it as `false`.
    #[must_use]
    pub const fn default_sort(self, feed_group: bool) -> GuiListSort {
        match self {
            Self::Library(_) if feed_group => GuiListSort::Newest,
            Self::Library(_) => GuiListSort::Title,
            Self::Audiobooks(_) => GuiListSort::Author,
            Self::Podcasts(_) | Self::Feeds | Self::ShowsNew(_) => GuiListSort::Newest,
        }
    }

    /// The destination kind the target's identity resolves through (design
    /// D3): a `Library` target resolves as a plain Emby library, whose
    /// identity the feed-group mode shares, and `ShowsNew` resolves as the
    /// Shows destination, which has no other flat list.
    #[must_use]
    const fn destination_kind(self) -> GuiDestinationKind {
        match self {
            Self::Library(lib_idx) => GuiDestinationKind::Library { lib_idx },
            Self::Audiobooks(lib_idx) => GuiDestinationKind::Audiobooks { lib_idx },
            Self::Podcasts(lib_idx) => GuiDestinationKind::Podcasts { lib_idx },
            Self::Feeds => GuiDestinationKind::Feeds,
            Self::ShowsNew(lib_idx) => GuiDestinationKind::Shows { lib_idx },
        }
    }
}

/// One flat list's sort control (design D7): the target a press's command
/// names, the active sort, and the sorts the control offers in paint order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiListSortFrame {
    /// The target a press names.
    pub target: GuiSortTarget,
    /// The sort the list is ordered by.
    pub active: GuiListSort,
    /// Every sort the control offers.
    pub choices: Vec<GuiListSort>,
}

impl App {
    /// Apply one sort command (design D5): a target that names no presented
    /// flat list, or a sort outside the target's choices, is logged and
    /// ignored; anything else sets the destination's store.
    pub(in crate::app::gui_library) fn apply_gui_list_sort(
        &mut self,
        target: GuiSortTarget,
        sort: GuiListSort,
    ) {
        if !self.gui_sort_target_present(target) {
            super::log_ignored_library_command(
                "listSort",
                "the target names no presented flat list",
            );
            return;
        }
        if !target.choices().contains(&sort) {
            super::log_ignored_library_command(
                "listSort",
                "the sort is not one of the target's choices",
            );
            return;
        }
        self.set_gui_list_sort(target, sort);
    }

    /// Whether `target` names a presented flat list (design D5): the
    /// destination's own presence rule, so a stale `lib_idx`, a `ShowsNew`
    /// whose library is not a TV library, and a Feeds tab with no
    /// subscription all reject the command.
    fn gui_sort_target_present(&self, target: GuiSortTarget) -> bool {
        match target {
            GuiSortTarget::Library(lib_idx) => self.gui_emby_library(lib_idx),
            GuiSortTarget::Audiobooks(lib_idx) => self.gui_abs_book_library(lib_idx),
            GuiSortTarget::Podcasts(lib_idx) => super::podcasts::podcast_library(self, lib_idx),
            GuiSortTarget::Feeds => self.gui_feeds_present(),
            GuiSortTarget::ShowsNew(lib_idx) => self.gui_tv_library(lib_idx),
        }
    }

    /// The sort `target`'s list is ordered by (design D2): the stored sort
    /// when the store holds one and the target offers it, else the target's
    /// default. A stored sort the target does not offer is ignored, because
    /// the control cannot paint it, and a target that resolves to no identity
    /// reads the default too.
    #[must_use]
    pub fn gui_list_sort(&self, target: GuiSortTarget) -> GuiListSort {
        let default = target.default_sort(self.gui_sort_target_is_feed_group(target));
        let Some(identity) = self.gui_sort_target_identity(target) else {
            return default;
        };
        match self.gui_list_sort_stored(&identity) {
            Some(sort) if target.choices().contains(&sort) => sort,
            _ => default,
        }
    }

    /// The sort control `target`'s list paints (design D7): the target, its
    /// active sort, and its choices.
    #[must_use]
    pub fn gui_list_sort_frame(&self, target: GuiSortTarget) -> GuiListSortFrame {
        GuiListSortFrame {
            target,
            active: self.gui_list_sort(target),
            choices: target.choices().to_vec(),
        }
    }

    /// Set `target`'s sort (design D3): the entry is replaced in place, or
    /// pushed when the destination has none, so the store keeps the order its
    /// destinations were first sorted in. A target that resolves to no
    /// identity stores nothing.
    pub fn set_gui_list_sort(&mut self, target: GuiSortTarget, sort: GuiListSort) {
        if let Some(identity) = self.gui_sort_target_identity(target) {
            self.install_gui_list_sort(identity, sort);
        }
    }

    /// Install one destination's sort by its stable identity, replacing the
    /// entry in place or pushing it (design D3, D10): the restore installs
    /// the saved sorts before any catalog lands, so it keys them by identity,
    /// and the command path resolves its target to that same identity.
    pub(in crate::app::gui_library) fn install_gui_list_sort(
        &mut self,
        identity: TabIdentity,
        sort: GuiListSort,
    ) {
        match self
            .gui
            .list_sorts
            .iter_mut()
            .find(|(stored, _)| stored == &identity)
        {
            Some(entry) => entry.1 = sort,
            None => self.gui.list_sorts.push((identity, sort)),
        }
    }

    /// The sort the store holds for `identity` (design D3, D10): the restore
    /// reads it to leave a destination the session already sorted alone, and
    /// to migrate a saved date order only where it holds none.
    #[must_use]
    pub(in crate::app::gui_library) fn gui_list_sort_stored(
        &self,
        identity: &TabIdentity,
    ) -> Option<GuiListSort> {
        self.gui
            .list_sorts
            .iter()
            .find(|(stored, _)| stored == identity)
            .map(|(_, sort)| *sort)
    }

    /// The stable identity `target`'s sort is keyed by (design D3): the
    /// destination identity the launch surface resolves, so a library keeps
    /// its sort across a restart and two libraries of one kind keep their
    /// own.
    fn gui_sort_target_identity(&self, target: GuiSortTarget) -> Option<TabIdentity> {
        super::launch::destination_identity(self, target.destination_kind())
    }

    /// Whether `target` is a `Library` target in the feed-group mode (design
    /// D2), whose default sort differs from the plain mode's.
    fn gui_sort_target_is_feed_group(&self, target: GuiSortTarget) -> bool {
        match target {
            GuiSortTarget::Library(lib_idx) => self.is_feed_home_video_library(lib_idx),
            GuiSortTarget::Audiobooks(_)
            | GuiSortTarget::Podcasts(_)
            | GuiSortTarget::Feeds
            | GuiSortTarget::ShowsNew(_) => false,
        }
    }
}
