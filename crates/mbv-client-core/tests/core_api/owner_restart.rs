//! The attach-refusal follow-up contract (design D6, #559).

use mbv_client_core::dispatch::session::owner_restart::{FollowUp, follow_up};
use mbv_remote_player::RemotePlayerError;
use rstest::rstest;

fn mismatch_error(owner_app_version: &str) -> RemotePlayerError {
    RemotePlayerError::owner_mismatch_for_test(mbv_ctrl::BuildInfo {
        app_version: owner_app_version.to_string(),
        build_id: "owner-build-1".to_string(),
        wire_id: String::new(),
    })
}

// #559 (design D6): prompt once, then wait after requesting restart.
#[rstest]
#[case::before_restart(false, FollowUp::Prompt)]
#[case::after_restart(true, FollowUp::WaitForOwnerExit)]
fn mismatch_after_restart_waits_instead_of_reprompting(
    #[case] restart_requested: bool,
    #[case] expected: FollowUp,
) {
    assert_eq!(
        follow_up(&mismatch_error("1.0.0"), restart_requested),
        expected
    );
}
