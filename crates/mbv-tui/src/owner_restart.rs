//! Owner build-mismatch handling at startup (issue #559): the terminal prompt,
//! and the wait for the old Owner process to exit.
//!
//! A local Client refuses an Owner built from a different version before
//! sending its control credential. What that refusal means — prompt, wait for
//! the old Owner, refuse, or report — is the core's pure decision
//! ([`mbv_client_core::dispatch::session::owner_restart::follow_up`]). This module
//! turns it into the terminal prompt and reads the answer.

use std::io::{BufRead, Write};

use mbv_remote_player::RemotePlayerError;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Restart,
    Quit,
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

#[cfg(test)]
mod tests;
