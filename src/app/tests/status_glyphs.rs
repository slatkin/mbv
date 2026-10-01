use super::*;
use rstest::rstest;

// Nerd-font glyphs for the queue footer's remote pill and the autosave pill;
// without nerd fonts the plain glyph and the words stay as they were.

#[rstest]
#[case::local_daemon(RemoteSlotState::LocalDaemon, "\u{f0118}")]
#[case::attached_session(RemoteSlotState::AttachedSession, "\u{f0119}")]
#[case::direct_remote(RemoteSlotState::DirectRemote, "\u{f0119}")]
fn queue_footer_remote_icon_follows_the_connection_with_nerd_fonts(
    #[case] remote_state: RemoteSlotState,
    #[case] expected: &str,
) {
    let mut app = make_app_stub();
    app.use_nerd_fonts = true;

    assert_eq!(app.remote_icon_and_label(remote_state, "").0, expected);
}

#[test]
fn queue_footer_remote_icon_without_nerd_fonts_is_the_plain_glyph() {
    let app = make_app_stub();

    assert_eq!(
        app.remote_icon_and_label(RemoteSlotState::DirectRemote, "")
            .0,
        "\u{1F5A7}"
    );
}

fn pill_label(app: &App) -> String {
    app.autosave_status_spans().unwrap()[1].content.to_string()
}

fn app_with_dirty_queue(use_nerd_fonts: bool) -> App {
    let mut app = make_app_stub();
    app.use_nerd_fonts = use_nerd_fonts;
    app.queue_dirty = true;
    app
}

fn app_with_autosaving_playlist(use_nerd_fonts: bool) -> App {
    let mut app = make_app_stub();
    app.use_nerd_fonts = use_nerd_fonts;
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        });
    app.config.lock().unwrap().save_playlist_on_consume = true;
    app
}

#[rstest]
#[case::nerd_fonts(true, " \u{f0f42} ")]
#[case::plain(false, " UNSAVED ")]
fn unsaved_pill_label_follows_the_nerd_font_flag(#[case] nerd: bool, #[case] expected: &str) {
    assert_eq!(pill_label(&app_with_dirty_queue(nerd)), expected);
}

#[rstest]
#[case::nerd_fonts(true, " \u{f18ea} ")]
#[case::plain(false, " AUTOSAVE ")]
fn autosave_pill_label_follows_the_nerd_font_flag(#[case] nerd: bool, #[case] expected: &str) {
    assert_eq!(pill_label(&app_with_autosaving_playlist(nerd)), expected);
}
