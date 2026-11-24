//! The pure half of the Owner build-mismatch handling (issue #559): what an
//! attach refusal means for a Client whose Owner was built from a different
//! version.
//!
//! The terminal prompt that calls this lives with the TUI Client's startup
//! (`crate::owner_restart`). [`follow_up`] is pure over
//! `(error, restart_requested)` so the "do not prompt again while the old
//! Owner winds down" rule is unit-testable without `process::exit`
//! (design D6).

use mbv_remote_player::RemotePlayerError;

/// What a Client does about an attach refusal.
#[derive(Debug, PartialEq, Eq)]
pub enum FollowUp {
    /// Ask the user, then restart the Owner or quit.
    Prompt,
    /// The old Owner is shutting down. Wait for it to exit, then attach.
    WaitForOwnerExit,
    /// Another Client already holds the Owner. Refuse.
    RefuseSecondTerminal,
    /// Anything else. Report the error.
    Other,
}

pub fn follow_up(error: &RemotePlayerError, restart_requested: bool) -> FollowUp {
    if error.is_owner_shutting_down() {
        return FollowUp::WaitForOwnerExit;
    }
    if error.is_owner_mismatch() {
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
