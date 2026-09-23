use crate::app::components::library_panel::LibraryContentOwner;
use crate::app::shell::Model;
use crate::app::tests::make_app_stub;
use mbv_core::config::{HomeSelectorKey, SelectorIdentity};

#[test]
fn legacy_home_section_preference_falls_back_to_continue_watching() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "home_section": "abs:book-lib" }).to_string(),
    )
    .expect("write legacy prefs");

    let model = Model::new(make_app_stub());
    let (selector, _) = model.home_owner_shared().unwrap().launch_snapshot();
    assert_eq!(
        selector,
        Some(SelectorIdentity::Home {
            key: HomeSelectorKey::Continue,
        })
    );
}
