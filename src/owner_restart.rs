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
    Other,
}

pub(super) fn ask(
    owner_app_version: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Choice {
    let _ = writeln!(
        output,
        "Owner process version: {owner_app_version}\nThis terminal version: {}\nRestarting stops playback and closes any other mbv terminals.\n[R] Restart  [Q] Quit",
        env!("CARGO_PKG_VERSION")
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
    if error.is_owner_shutting_down()
        || (restart_requested && error.owner_build_mismatch().is_some())
    {
        FollowUp::WaitForOwnerExit
    } else if error.owner_build_mismatch().is_some() {
        FollowUp::Prompt
    } else {
        FollowUp::Other
    }
}

#[cfg(test)]
mod tests;
