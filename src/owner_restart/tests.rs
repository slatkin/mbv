use std::io::Cursor;

use rstest::rstest;

use super::{Choice, FollowUp, ask, follow_up};
use crate::remote_player::RemotePlayerError;

#[rstest]
#[case::lowercase_restart(b"r\n", Choice::Restart)]
#[case::uppercase_restart(b"R\n", Choice::Restart)]
#[case::trimmed_restart(b" r\n", Choice::Restart)]
#[case::empty_line(b"\n", Choice::Quit)]
#[case::quit(b"q\n", Choice::Quit)]
#[case::unknown(b"x\n", Choice::Quit)]
#[case::eof(b"", Choice::Quit)]
fn only_explicit_restart_input_restarts(#[case] input: &[u8], #[case] expected: Choice) {
    assert_eq!(
        ask("1.0.0", &mut Cursor::new(input), &mut Vec::new()),
        expected
    );
}

#[test]
fn prompt_names_both_versions_and_restart_consequences() {
    let mut output = Vec::new();
    let _ = ask("1.0.0", &mut Cursor::new(b"q\n"), &mut output);
    let output = String::from_utf8(output).expect("prompt output should be UTF-8");

    assert!(output.contains("1.0.0"));
    assert!(output.contains(env!("CARGO_PKG_VERSION")));
    assert!(output.contains("Restarting stops playback and closes any other mbv terminals"));
}

// #559: prompt once with the Owner version, then wait after requesting restart.
#[rstest]
#[case::before_restart(false, FollowUp::Prompt("1.0.0"))]
#[case::after_restart(true, FollowUp::WaitForOwnerExit)]
fn mismatch_after_restart_waits_instead_of_reprompting(
    #[case] restart_requested: bool,
    #[case] expected: FollowUp<'_>,
) {
    let error = RemotePlayerError::owner_build_mismatch_error("1.0.0");

    assert_eq!(follow_up(&error, restart_requested), expected);
}
