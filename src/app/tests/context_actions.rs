use super::{make_app_stub, make_item};
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::ContextAction;

#[test]
fn watched_toggle_ignores_folders_and_audio_before_service_access() {
    let mut app = make_app_stub();
    let severity = app.status_severity;
    let mut folder = make_item("folder", "Folder");
    folder.is_folder = true;
    let audio = make_item("audio", "Audio");

    app.toggle_watched_item(0, folder);
    app.toggle_watched_item(0, audio);

    assert_eq!(app.status_severity, severity);
}

#[test]
fn context_mark_without_emby_reports_unavailable() {
    let mut app = make_app_stub();

    app.execute_context_action(Some(ContextAction::MarkPlayed("item".into())), None);

    assert_eq!(app.status_severity, ToastSeverity::Warning);
}
