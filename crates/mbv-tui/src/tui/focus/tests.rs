use super::*;
use mbv_ui_msg::{ModalId, OverlayId, PopupId};

/// The reconcile census covers every overlay identity (PR 984 review,
/// finding 1): an identity the census misses is still mounted and focused by
/// `push_focus_overlay`, but `reconcile_focus_stack_mounts` never adopts or
/// dismisses it, so a direct `TuiRealm` umount would leave the stack holding
/// a dead top overlay and the focus step would stop applying focus. Each
/// match names every variant of its enum with no wildcard arm, so a variant
/// added without a census row fails to compile here; its arm checks the
/// census, so the assertion then fails until the variant's row lands in
/// [`overlay_surface_ids`].
#[test]
fn the_reconcile_census_covers_every_overlay_identity() {
    fn overlay_covered(id: OverlayId, census: &[ComponentId]) -> bool {
        match id {
            OverlayId::Search
            | OverlayId::Settings
            | OverlayId::Sessions
            | OverlayId::Playlists
            | OverlayId::Help
            | OverlayId::ContextMenu => census.contains(&ComponentId::Overlay(id)),
        }
    }

    fn modal_covered(id: ModalId, census: &[ComponentId]) -> bool {
        match id {
            ModalId::Confirm | ModalId::DaemonLost | ModalId::SavePlaylist | ModalId::TwitchUrl => {
                census.contains(&ComponentId::Modal(id))
            }
        }
    }

    fn popup_covered(id: PopupId, census: &[ComponentId]) -> bool {
        match id {
            PopupId::Multiselect | PopupId::LibraryRoutes | PopupId::FeedManage => {
                census.contains(&ComponentId::Popup(id))
            }
        }
    }

    let census = overlay_surface_ids();
    for covered in [
        overlay_covered(OverlayId::Search, &census),
        overlay_covered(OverlayId::Settings, &census),
        overlay_covered(OverlayId::Sessions, &census),
        overlay_covered(OverlayId::Playlists, &census),
        overlay_covered(OverlayId::Help, &census),
        overlay_covered(OverlayId::ContextMenu, &census),
        modal_covered(ModalId::Confirm, &census),
        modal_covered(ModalId::DaemonLost, &census),
        modal_covered(ModalId::SavePlaylist, &census),
        modal_covered(ModalId::TwitchUrl, &census),
        popup_covered(PopupId::Multiselect, &census),
        popup_covered(PopupId::LibraryRoutes, &census),
        popup_covered(PopupId::FeedManage, &census),
    ] {
        assert!(
            covered,
            "an overlay identity is missing from the `overlay_surface_ids` census"
        );
    }
}
