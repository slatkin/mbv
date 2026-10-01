use std::io::Cursor;

use rstest::rstest;

use super::{Choice, FollowUp, ask, follow_up};
use crate::remote_player::RemotePlayerError;

fn mismatch_error(owner_app_version: &str) -> RemotePlayerError {
    RemotePlayerError::owner_build_mismatch_for_test(owner_app_version)
}

// Issue #559 (tasks row 3.1 contract: only an explicit r restarts). The
// Restart-input trio is the spec-enumerated set -- case-insensitivity and
// trimming are named behaviours, not thin duplicates.
#[rstest]
#[case::lowercase_restart(b"r\n", Choice::Restart)]
#[case::uppercase_restart(b"R\n", Choice::Restart)]
#[case::trimmed_restart(b" r\n", Choice::Restart)]
#[case::empty_line(b"\n", Choice::Quit)]
#[case::unknown_input(b"x\n", Choice::Quit)]
#[case::quit(b"q\n", Choice::Quit)]
#[case::eof(b"", Choice::Quit)]
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
