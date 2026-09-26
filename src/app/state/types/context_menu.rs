use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{FeedEntry, QueueSlotId};

#[derive(Clone, Debug)]
pub(in crate::app) enum BulkRemoveTarget {
    ContinueWatching(Box<EmbyItem>),
    Queue(QueueSlotId),
}
use ratatui::layout::Rect;

use crate::app::components::media_list::SelectionOrigin;
use crate::app::components::msg::HomeRowTarget;

/// Values resolved when a context menu opens. The overlay never re-resolves
/// these values after focus changes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ContextActionSnapshot<T> {
    pub origin: SelectionOrigin,
    pub values: Vec<T>,
}

/// Destination-qualified targets supplied by row context-menu requests.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ContextMenuTargets {
    Home(Vec<HomeRowTarget>),
    Browser(Vec<String>),
    Emby(Vec<EmbyItem>),
    Queue(Vec<QueueSlotId>),
    Feeds(Vec<FeedEntry>),
}
use unicode_width::UnicodeWidthStr;

use crate::app::state::types::settings::PanelFocus;

/// How a context menu's position is anchored. A keyboard-opened menu keeps a
/// selected-item anchor resolved from each fresh frame's layout; a
/// mouse-opened menu keeps its click point and is independent of selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum ContextMenuAnchor {
    /// Keyboard opening: anchor to the focused panel's selected item.
    SelectedItem(PanelFocus),
    /// Mouse opening: anchor to the click point.
    Pointer { x: u16, y: u16 },
}

/// The containing panel a context menu is clamped inside. The library panel
/// and the queue panel are distinct surfaces with distinct geometry.
#[derive(Clone, Debug)]
pub(in crate::app) enum ContextAction {
    Play,
    PlaySelection(Vec<EmbyItem>),
    ShuffleSelection(Vec<EmbyItem>),
    EnqueueSelection(Vec<EmbyItem>),
    RemoveSelection(Vec<BulkRemoveTarget>),
    MarkPlayedSelection(Vec<String>),
    MarkUnplayedSelection(Vec<String>),
    /// Play the queue item at this explicit index (split-queue-cursor-
    /// ownership D2): the queue menu retains the index resolved when the
    /// menu opened (the right-clicked slot), so a follow update to
    /// `queue_cursor` cannot redirect playback to another row.
    PlayQueue(usize),
    PlayFolder(String),
    ShuffleFolder(String),
    Enqueue,
    EnqueueFolder(Box<EmbyItem>),
    MarkPlayed(String),
    MarkUnplayed(String),
    RemoveFromContinueWatching,
    RemoveFromQueue(usize),
    GoToLibrary(String, String), // (item_id, item_type)
    FeedsPlay(Vec<FeedEntry>),
    FeedsEnqueue(Vec<FeedEntry>),
    FeedsMarkPlayed(Vec<FeedEntry>),
    FeedsMarkUnplayed(Vec<FeedEntry>),
}

#[derive(Clone)]
pub(in crate::app) struct ContextMenuEntry {
    pub(in crate::app) label: &'static str,
    pub(in crate::app) action: Option<ContextAction>,
}

pub(in crate::app) fn is_bulk_action(action: Option<&ContextAction>) -> bool {
    matches!(
        action,
        Some(
            ContextAction::PlaySelection(_)
                | ContextAction::ShuffleSelection(_)
                | ContextAction::EnqueueSelection(_)
                | ContextAction::RemoveSelection(_)
                | ContextAction::MarkPlayedSelection(_)
                | ContextAction::MarkUnplayedSelection(_)
        )
    )
}

/// One multiselect row: `(name_lower, display_name, is_hidden)`.
pub(crate) type MultiSelectItem = (String, String, bool);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MultiSelectKind {
    HiddenLibraries,
    MyLanguages,
    FeedViewLibraries,
}

#[derive(Clone)]
pub(in crate::app) struct MultiSelectPopup {
    pub(in crate::app) kind: MultiSelectKind,
    pub(in crate::app) items: Vec<MultiSelectItem>,
    pub(in crate::app) cursor: usize,
}

#[derive(Clone)]
pub(crate) enum LibraryRouteStage {
    /// (library_name_lower, display_name, current_device_or_none)
    PickLibrary {
        items: Vec<(String, String, Option<String>)>,
    },
    /// index 0 is always the synthetic "Local (no route)" entry.
    /// Each entry pairs a device's display name (UX only -- #256 never
    /// persists it) with its live-resolved endpoint (what actually gets
    /// written to config on commit). `None` means the device is visible
    /// in the live session list but session_direct_endpoint couldn't
    /// resolve it to a connectable address (e.g. no advertised
    /// direct-connect port, or an unparseable host) -- shown greyed out
    /// with a reason rather than silently omitted, and not committable.
    PickDevice {
        library_lower: String,
        devices: Vec<(String, Option<mbv_core::remote_player::DaemonEndpoint>)>,
    },
}

pub(crate) struct LibraryRoutePopup {
    pub(in crate::app) stage: LibraryRouteStage,
    pub(in crate::app) cursor: usize,
}

pub(in crate::app) struct ContextMenu {
    pub(in crate::app) anchor: ContextMenuAnchor,
    pub(in crate::app) entries: Vec<ContextMenuEntry>,
    pub(in crate::app) cursor: usize,
}

impl ContextMenu {
    pub(in crate::app) fn first_selectable(entries: &[ContextMenuEntry]) -> usize {
        entries
            .iter()
            .position(|entry| entry.action.is_some())
            .unwrap_or(0)
    }

    /// Rendered menu size: widest entry label + 4 (2 leading + 2 trailing
    /// spaces, matching `render_context_menu`'s `" {} "` item format), and
    /// `entries.len() + 2` rows (one blank top/bottom border row each).
    pub(in crate::app) fn rendered_size(entries: &[ContextMenuEntry]) -> (u16, u16) {
        let width = u16::try_from(
            entries
                .iter()
                .map(|entry| UnicodeWidthStr::width(entry.label))
                .max()
                .unwrap_or(4)
                + 4,
        )
        .unwrap_or(u16::MAX);
        let height = u16::try_from(entries.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2);
        (width.max(4), height.max(1))
    }

    /// Pure, saturating placement of a menu of the given rendered `size`
    /// against a containing `panel` rect, from either a selected-item
    /// `anchor` rect or a pointer `(x, y)`.
    ///
    /// For a selected item:
    /// 1. right-align the menu to the selected rect's right edge;
    /// 2. open from the selected rect's top when the full menu fits below;
    /// 3. otherwise align the menu bottom to the selected rect's bottom;
    /// 4. clamp the result inside the panel.
    ///
    /// For a pointer: place the menu at the click point, then clamp inside
    /// the panel. Exact anchor alignment wins when compatible with
    /// visibility; panel bounds win otherwise. When the menu exceeds a panel
    /// dimension, that panel edge wins and the terminal renderer may clip.
    pub(in crate::app) fn place(
        panel: Rect,
        size: (u16, u16),
        anchor: Option<&Rect>,
        pointer: Option<(u16, u16)>,
    ) -> (u16, u16) {
        let (width, height) = size;

        // Desired top-left corner. A selected item right-aligns the menu
        // (its right edge coincides with the item's right edge) and opens
        // downward when it fits, else upward. A pointer places the menu with
        // its top-left at the click point.
        let (want_x, want_y) = if let Some(anchor) = anchor {
            let x = anchor.right().saturating_sub(width);
            let fits_below = anchor.bottom() + height <= panel.bottom();
            let y = if fits_below {
                anchor.y
            } else {
                anchor.bottom().saturating_sub(height)
            };
            (x, y)
        } else if let Some((px, py)) = pointer {
            (px, py)
        } else {
            (panel.x, panel.y)
        };

        // Clamp inside the panel. Exact anchor alignment wins when it stays
        // visible; panel bounds win otherwise.
        let x = want_x.max(panel.x).min(panel.right().saturating_sub(width));
        let y = want_y
            .max(panel.y)
            .min(panel.bottom().saturating_sub(height));
        (x, y)
    }
}
