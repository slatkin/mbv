use crate::app::components::{Msg, ShellRequest};
use crate::app::render::make_movie_app;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn list_pane_resize_live_is_memory_only_and_end_saves_prefs() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({"sentinel": true}).to_string(),
    )
    .expect("write initial prefs");
    let before = std::fs::read(crate::config::prefs_path()).expect("read initial prefs");

    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 120;
    app.terminal_height = 30;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("test terminal");
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .expect("draw test frame");

    let content_width = harness
        .model()
        .library_panel_content_area()
        .expect("library content area")
        .width;
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::ResizeListPaneLive(42))),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness.model().app.list_pane_width,
        crate::app::state::list_pane_width::normalize_list_pane_width(Some(42), content_width),
        "Live stores the normalized in-memory split width"
    );
    assert_eq!(
        std::fs::read(crate::config::prefs_path()).expect("read live prefs"),
        before,
        "Live does not write preferences"
    );

    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::ResizeListPaneEnd(u16::MAX))),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness.model().app.list_pane_width,
        crate::app::state::list_pane_width::normalize_list_pane_width(
            Some(u16::MAX),
            content_width,
        ),
        "End stores the normalized (clamped) in-memory split width"
    );
    assert_ne!(
        std::fs::read(crate::config::prefs_path()).expect("read end prefs"),
        before,
        "End rewrites preferences"
    );
}
