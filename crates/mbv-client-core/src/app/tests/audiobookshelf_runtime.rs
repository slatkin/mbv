use crate::app::tests;
use mbv_core::service_runtime::ServiceState;

fn completion(
    generation: mbv_core::service_runtime::SetupGeneration,
    kind: crate::app::dispatch::session::service_startup::AudiobookshelfCompletionKind,
    result: Result<mbv_audiobookshelf::AudiobookshelfUser, mbv_audiobookshelf::AudiobookshelfError>,
) -> crate::app::dispatch::session::service_startup::AudiobookshelfCompletion {
    crate::app::dispatch::session::service_startup::AudiobookshelfCompletion {
        generation,
        kind,
        result,
    }
}

#[test]
fn stale_completion_after_removal_cannot_mutate_runtime() {
    let mut app = tests::make_app_stub();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.audiobookshelf_runtime.remove_setup();
    app.apply_audiobookshelf_completion(completion(
        generation,
        crate::app::dispatch::session::service_startup::AudiobookshelfCompletionKind::Test,
        Ok(mbv_audiobookshelf::AudiobookshelfUser {
            id: "user-id".into(),
            username: "reader".into(),
        }),
    ));
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NotConfigured
    );
    assert!(app.audiobookshelf_runtime.user.is_none());
}
