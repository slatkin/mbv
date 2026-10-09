//! Launch-state contract tests for the TV owner: the TV content mode and
//! letter pill are session memory, so the snapshot records no selector and
//! no item, and restore applies nothing (change
//! `latest-pill-restart-default`; restart resolves the count-dependent
//! default through the load path).

use super::*;

fn launch_state_with_letter() -> mbv_config::TuiLaunchState {
    mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: Some(mbv_config::SelectorIdentity::Emby {
            key: mbv_config::EmbySelectorKey::Letter(
                mbv_config::EmbyLetterBucket::from_index(0).expect("bucket 0"),
            ),
        }),
        item: None,
    }
}

#[test]
fn launch_snapshot_records_no_selector_and_no_item() {
    let mut component = TvContent::new();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        None,
        true,
    ));

    assert_eq!(component.launch_snapshot(), (None, None));
}

#[test]
fn a_legacy_letter_selector_applies_nothing() {
    let mut component = TvContent::new();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        None,
        true,
    ));

    assert_eq!(component.launch_selector(&launch_state_with_letter()), None);
}
