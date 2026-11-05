//! Public-API tests for the core focus stack (task 3.2): push, dismiss-restore
//! independent of mount order, and Panel focus moves.

use mbv_surfaces::FocusStack;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{ComponentId, ModalId, OverlayId};

fn search_overlay() -> ComponentId {
    ComponentId::Overlay(OverlayId::Search)
}

#[test]
fn push_overlay_mounts_and_focuses_it() {
    let mut stack = FocusStack::new();
    stack.mount(ComponentId::Queue);
    stack.set_panel_focus(PanelFocus::Queue);

    let id = search_overlay();
    stack.push_overlay(id.clone());

    assert!(stack.is_mounted(&id));
    assert_eq!(stack.focused(), Some(id));
}

#[rstest::rstest]
#[case(ComponentId::Queue, ComponentId::Library)]
#[case(ComponentId::Library, ComponentId::Queue)]
fn dismiss_restores_panel_focus_independent_of_mount_order(
    #[case] first_mounted: ComponentId,
    #[case] second_mounted: ComponentId,
) {
    let mut stack = FocusStack::new();
    stack.mount(first_mounted);
    stack.mount(second_mounted);
    stack.set_panel_focus(PanelFocus::Queue);

    let id = ComponentId::Modal(ModalId::Confirm);
    stack.push_overlay(id.clone());
    assert_eq!(stack.focused(), Some(id.clone()));

    assert!(stack.dismiss(&id));
    assert_eq!(stack.focused(), Some(ComponentId::Queue));
}

#[test]
fn dismissing_the_top_overlay_restores_the_one_beneath_it() {
    let mut stack = FocusStack::new();
    stack.mount(ComponentId::Library);

    let bottom = search_overlay();
    let top = ComponentId::Modal(ModalId::Confirm);
    stack.push_overlay(bottom.clone());
    stack.push_overlay(top.clone());
    assert_eq!(stack.focused(), Some(top.clone()));

    stack.dismiss(&top);
    assert_eq!(stack.focused(), Some(bottom.clone()));

    stack.dismiss(&bottom);
    assert_eq!(stack.focused(), Some(ComponentId::Library));
}

#[test]
fn panel_focus_moves_move_the_focused_surface() {
    let mut stack = FocusStack::new();
    stack.mount(ComponentId::Library);
    stack.mount(ComponentId::Queue);
    assert_eq!(stack.focused(), Some(ComponentId::Library));

    stack.set_panel_focus(PanelFocus::Queue);
    assert_eq!(stack.focused(), Some(ComponentId::Queue));

    stack.set_panel_focus(PanelFocus::Library);
    assert_eq!(stack.focused(), Some(ComponentId::Library));
}

#[test]
fn unmounting_an_overlay_drops_it_from_the_stack() {
    let mut stack = FocusStack::new();
    stack.mount(ComponentId::Queue);
    stack.set_panel_focus(PanelFocus::Queue);

    let id = search_overlay();
    stack.push_overlay(id.clone());
    assert!(stack.umount(&id));

    assert!(!stack.is_mounted(&id));
    assert_eq!(stack.overlay_stack(), []);
    assert_eq!(stack.focused(), Some(ComponentId::Queue));
}
