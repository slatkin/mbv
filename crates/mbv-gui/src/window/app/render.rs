//! The window shell's render helpers (gtds 12.4): the sub-trees of `App`'s
//! rsx that hang together — the Library panel's slot, the Queue column, and
//! the window-modal overlays. Each returns the `Element` its parent splices
//! inline, so the painted tree keeps the nodes, attributes, and order it had
//! when `App` built them itself.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::layout::LayoutSize;
use crate::window::arrangement::GuiArrangement;
use crate::window::components::confirm_overlay::ConfirmOverlay;
use crate::window::components::daemon_lost_overlay::DaemonLostOverlay;
use crate::window::components::library_panel::LibraryPanel;
use crate::window::components::panel::Panel;
use crate::window::components::playlists_panel::PlaylistsPanel;
use crate::window::resize::PanelWidths;

/// The Library panel's slot (design D8): the Playlists screen while it shows,
/// the active destination's Library panel while the window holds one, and
/// nothing otherwise. The Queue divider and the Queue column mount after it.
pub(super) fn library_column(
    visible: bool,
    playlists_open: bool,
    has_library_list: bool,
    arrangement: GuiArrangement,
    widths: PanelWidths,
) -> Element {
    if visible && playlists_open {
        // The Playlists screen takes the Library panel's slot, with the Queue
        // to its right.
        rsx! {
            PlaylistsPanel {
                arrangement,
                list_width: widths.list,
                library_column: widths.library_column,
            }
        }
    } else if visible && has_library_list {
        rsx! {
            LibraryPanel {
                arrangement,
                list_width: widths.list,
                library_column: widths.library_column,
            }
        }
    } else {
        rsx! {}
    }
}

/// The Queue column's mount (design D8): the Player panel over the measured
/// container, in the column class the arrangement and the collapsed state
/// pick, and the divider's resized width while two columns paint.
pub(super) fn queue_column(
    visible: bool,
    two_columns: bool,
    class: &'static str,
    width: f32,
    container: Signal<LayoutSize>,
    library_visible: bool,
) -> Element {
    if !visible {
        return rsx! {};
    }
    rsx! {
        div {
            class,
            "data-queue-column": "",
            style: if two_columns {
                format!("width: {width}px;")
            } else {
                String::new()
            },
            Panel {
                container: container.cloned(),
                library_visible,
            }
        }
    }
}

/// The window's window-modal overlays (D6): the raised `Confirm` and
/// `DaemonLost` frames, painted over every panel. The row menu, track popover,
/// and playback-target chooser paint inside their panels; the root mounts
/// nothing for them, as `App` does.
pub(super) fn overlay_layer(overlay: Option<&GuiOverlayFrame>) -> Element {
    match overlay {
        // The App's raised confirmation modal, window-modal over every panel
        // (add-gui-library-panel queue fixes): Enter and Esc answer in the
        // core's overlay key rules, and the buttons report their press
        // regions.
        Some(GuiOverlayFrame::Confirm { message, buttons }) => rsx! {
            ConfirmOverlay {
                message: message.clone(),
                buttons: buttons.clone(),
            }
        },
        // The App's raised daemon-lost recovery dialog, window-modal over
        // every panel: the dialog blocks every other input until one of its
        // choices runs, and the buttons report their press regions.
        Some(GuiOverlayFrame::DaemonLost {
            last_playing_title,
            daemon_log_path,
            restart_error,
            buttons,
        }) => rsx! {
            DaemonLostOverlay {
                last_playing_title: last_playing_title.clone(),
                daemon_log_path: daemon_log_path.clone(),
                restart_error: restart_error.clone(),
                buttons: buttons.clone(),
            }
        },
        _ => rsx! {},
    }
}
