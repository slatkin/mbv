use super::*;
use crate::app::tests::make_app_stub;
use mbv_components::HelpComponent;

#[test]
fn root_ui_uses_native_lifo_focus_restoration() {
    let mut model = Model::new(make_app_stub());
    let help = ComponentId::Overlay(OverlayId::Help);
    let confirm = ComponentId::Modal(ModalId::Confirm);

    model
        .application
        .mount(help.clone(), Box::new(HelpComponent::new()), vec![])
        .unwrap();
    model.application.active(&help).unwrap();
    model
        .application
        .mount(
            confirm.clone(),
            Box::new(mbv_components::ConfirmComponent::new()),
            vec![],
        )
        .unwrap();
    model.application.active(&confirm).unwrap();

    model.application.umount(&confirm).unwrap();
    assert_eq!(model.application.focus(), Some(&help));
    model.application.umount(&help).unwrap();
    assert_eq!(model.application.focus(), Some(&ComponentId::UiRoot));
}
