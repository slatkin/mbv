//! The leaf adapters' mapping contracts: each gesture a surface owns resolves to
//! the `Msg` the matching TUI pointer path emits, over a mock hit map.

use super::{PaintedChoices, PlaybackLeaf, QueueLeaf, QueuePlaybackSurface, TransportAvailability};
use crate::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};
use crate::gui_core::GuiSurfaceLeaf;
use mbv_queue::QueueSlotId;
use mbv_surfaces::gesture::{ClickSelection, PointerGesture};
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::{HitMap, HitRegion};
use mbv_ui_model::media_list::{MediaKind, MediaListRow, MediaSemanticState};
use mbv_ui_msg::{Msg, PlaybackRequest, Position, ShellRequest};
use rstest::rstest;

/// A hit map holding `target` over one test rectangle.
fn regions_with(target: RegionTarget) -> HitMap<RegionTarget> {
    let mut regions = HitMap::new();
    regions.push(HitRegion::new(0.0, 0.0, 20.0, 20.0), target);
    regions
}

/// One item row for `slot`.
fn item_row(slot: u64) -> MediaListRow<QueueSlotId> {
    MediaListRow::Item {
        target: QueueSlotId::from_raw(slot),
        primary: format!("Item {slot}"),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// A Queue Surface holding the rows named by `slots`, in order.
fn queue_of(slots: &[u64]) -> QueueSurface {
    let mut surface = QueueSurface::new();
    surface
        .list_mut()
        .set_content(slots.iter().copied().map(item_row).collect());
    surface
}

/// A right-click anywhere on the panel opens the player menu at the
/// pointer's whole-pixel anchor (gui-player-overlay-menu).
#[test]
fn a_right_click_on_the_panel_opens_the_player_menu() {
    let mut surface = QueuePlaybackSurface::new();
    let regions = regions_with(RegionTarget::QueuePlayback(
        QueuePlaybackTargetId::VisualSlot,
    ));
    let painted = PaintedChoices::default();
    let mut leaf = PlaybackLeaf::new(
        &mut surface,
        &regions,
        TransportAvailability {
            stop: false,
            previous: false,
            next: false,
        },
        &painted,
    );
    assert_eq!(
        leaf.pointer(PointerGesture::RightClick(Position { x: 10.6, y: 20.9 })),
        Some(Msg::Playback(PlaybackRequest::OpenPlayerMenu {
            x: 10.6,
            y: 20.9
        }))
    );
}

/// A left press on the player menu's auto-hide entry runs the entry
/// through the select request, whose App-side effect applies the same
/// execution the menu's Enter applies and closes the menu with it
/// (gui-player-overlay-menu).
#[test]
fn a_press_on_the_player_menu_entry_selects_it() {
    let mut surface = QueuePlaybackSurface::new();
    let regions = regions_with(RegionTarget::QueuePlayback(
        QueuePlaybackTargetId::PlayerMenuEntry,
    ));
    let painted = PaintedChoices::default();
    let leaf = PlaybackLeaf::new(
        &mut surface,
        &regions,
        TransportAvailability {
            stop: false,
            previous: false,
            next: false,
        },
        &painted,
    );
    assert_eq!(
        leaf.press(Position { x: 10.0, y: 10.0 }),
        Some(Msg::Playback(PlaybackRequest::PlayerMenuSelect))
    );
}

/// Contract (spec `gui-client` "A queue row shows a Play control on
/// hover", design D6 of `gui-row-hover-actions`): a press on the row's
/// hover Play control selects the row and emits the double-click's
/// activation request for that slot, so the control and the double-click
/// run one path.
#[test]
fn a_press_on_a_row_play_control_activates_its_slot() {
    let mut surface = queue_of(&[1, 2, 3, 4]);
    let regions = regions_with(RegionTarget::Queue(QueueTargetId::RowPlay(4)));
    let mut leaf = QueueLeaf::new(&mut surface, &regions);

    assert_eq!(
        leaf.pointer(PointerGesture::Click {
            at: Position { x: 10.0, y: 10.0 },
            selection: ClickSelection::Plain,
        }),
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
            slot_id: Some(QueueSlotId::from_raw(4)),
        })))
    );
}

/// Contract (spec `gui-client` "A queue row shows a Play control on
/// hover", design D6 of `gui-row-hover-actions`): a double-click resolves
/// to an activation only on a row's own region, so the second press of a
/// pair on the row's chrome plays nothing a second time.
#[rstest]
#[case::row_play(QueueTargetId::RowPlay(1))]
#[case::filter(QueueTargetId::Filter)]
fn a_double_click_on_row_chrome_activates_nothing(#[case] target: QueueTargetId) {
    let mut surface = queue_of(&[1, 2]);
    let regions = regions_with(RegionTarget::Queue(target));
    let mut leaf = QueueLeaf::new(&mut surface, &regions);

    assert_eq!(
        leaf.pointer(PointerGesture::DoubleClick(Position { x: 10.0, y: 10.0 })),
        None
    );
}
