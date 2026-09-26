use crate::app::components::ShellRequest;

#[test]
fn quit_request_sets_shell_quit_and_unknown_navigation_is_forwarded() {
    let mut model = crate::app::shell::Model::new(crate::app::tests::make_app_stub());
    let mut quit = false;

    assert!(model
        .handle_navigation_request(ShellRequest::Quit, &mut quit)
        .is_none());
    assert!(quit);

    let request = ShellRequest::LibraryPanelFocus;
    assert_eq!(
        model.handle_navigation_request(request.clone(), &mut quit),
        Some(request)
    );
}
