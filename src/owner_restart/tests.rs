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

// Issue #559: the prompt owns the user-visible mismatch text -- both
// versions, the `mbv -q` guidance, and exactly the two choices. Behavioural
// assertions, not the production format string, so wording reflows do not
// break the test while missing content still does.
#[test]
fn prompt_names_both_versions_and_restart_consequences() {
    let mut output = Vec::new();
    let _ = ask(
        &mismatch_error("1.0.0"),
        &mut Cursor::new(b"q\n"),
        &mut output,
    );
    let output = String::from_utf8(output).expect("prompt output should be UTF-8");

    assert!(output.contains("1.0.0"), "owner version missing: {output}");
    assert!(
        output.contains(env!("CARGO_PKG_VERSION")),
        "this terminal's version missing: {output}"
    );
    assert!(
        output.contains("`mbv -q`"),
        "removal guidance missing: {output}"
    );
    assert!(
        output.contains("[R] Restart"),
        "restart choice missing: {output}"
    );
    assert!(output.contains("[Q] Quit"), "quit choice missing: {output}");
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
