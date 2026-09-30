use std::io::Cursor;

use rstest::rstest;

use super::{Choice, FollowUp, ask, follow_up};
use crate::remote_player::RemotePlayerError;

fn mismatch_error(owner_app_version: &str) -> RemotePlayerError {
    RemotePlayerError::owner_build_mismatch_for_test(owner_app_version)
}

#[rstest]
#[case::lowercase_restart(b"r\n", Choice::Restart)]
#[case::uppercase_restart(b"R\n", Choice::Restart)]
#[case::trimmed_restart(b" r\n", Choice::Restart)]
#[case::not_restart(b"q\n", Choice::Quit)]
fn only_explicit_restart_input_restarts(#[case] input: &[u8], #[case] expected: Choice) {
    assert_eq!(
        ask(
            &mismatch_error("1.0.0"),
            &mut Cursor::new(input),
            &mut Vec::new()
        ),
        expected
    );
}

/// Issue #559: the prompt owns the user-visible mismatch text -- both
/// versions, the restart consequence, and exactly the two choices.
#[test]
fn prompt_names_both_versions_and_restart_consequences() {
    let mut output = Vec::new();
    let _ = ask(
        &mismatch_error("1.0.0"),
        &mut Cursor::new(b"q\n"),
        &mut output,
    );
    let output = String::from_utf8(output).expect("prompt output should be UTF-8");

    assert_eq!(
        output,
        format!(
            "Owner process is running version 1.0.0, but this terminal is version {}; stop it with `mbv -q`, then relaunch mbv\n\
             Restarting stops playback and closes any other mbv terminals.\n\
             [R] Restart  [Q] Quit\n",
            env!("CARGO_PKG_VERSION")
        )
    );
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
