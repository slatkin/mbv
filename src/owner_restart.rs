//! Owner build-mismatch handling at startup (issue #559): prompt, then wait
//! for the old Owner process to exit.
//!
//! A local Client refuses an Owner built from a different version before
//! sending its control credential. This module turns that refusal into the
//! terminal prompt and decides what to do next. [`follow_up`] is pure over
//! `(error, restart_requested)` so the "do not prompt again while the old
//! Owner winds down" rule is unit-testable without `process::exit`
//! (design D6).

use std::io::{BufRead, Write};

use crate::remote_player::RemotePlayerError;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Restart,
    Quit,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum FollowUp {
    Prompt,
    WaitForOwnerExit,
    RefuseSecondTerminal,
    Other,
}

/// Prints the mismatch and reads one line, restarting only on `r`/`R`
/// (trimmed); anything else, an empty line, or EOF quits.
pub(super) fn ask(
    error: &RemotePlayerError,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Choice {
    let _ = writeln!(
        output,
        "{error}\nRestarting stops playback and closes any other mbv terminals.\n[R] Restart  [Q] Quit"
    );
    let _ = output.flush();

    let mut response = String::new();
    if input.read_line(&mut response).is_ok() && matches!(response.trim(), "r" | "R") {
        Choice::Restart
    } else {
        Choice::Quit
    }
}

pub(super) fn follow_up(error: &RemotePlayerError, restart_requested: bool) -> FollowUp {
    if error.is_owner_shutting_down() {
        return FollowUp::WaitForOwnerExit;
    }
    if error.mismatched_owner_version().is_some() {
        return if restart_requested {
            FollowUp::WaitForOwnerExit
        } else {
            FollowUp::Prompt
        };
    }
    if error.is_exclusive_owner() {
        return FollowUp::RefuseSecondTerminal;
    }
    FollowUp::Other
}

#[cfg(test)]
mod tests;
