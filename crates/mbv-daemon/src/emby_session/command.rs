//! Transport commands onto the Emby Session (emby-session-owner-output design
//! D9, task 4.4): the wire commands, the guarded pause, and the next/previous
//! step. The transport, seek, volume, mute, and track commands act on whatever
//! the device plays, as the TUI's direct session commands do; only the
//! subtitle-preference intent has no device command and is refused.

use std::time::Instant;

use mbv_ctrl::{Direction, WireCommand};
use mbv_emby_model::seconds_to_ticks;

use super::EmbySessionOutput;
use super::status::EMBY_SESSION_VOLUME_MAX;
use super::worker::SessionApi;

/// Routes one wire command onto the Session (design D9). `SetSub(0)` (off)
/// becomes the Emby disabled index −1, a relative `Seek` starts from the D6
/// position estimate and clamps at 0, and `SetSubtitlePrefs` is refused with
/// the reason the caller reports. The next-up and skip-intro overlays are
/// mpv-script messages with no Session counterpart, a documented no-op.
pub(crate) fn route_wire_command(
    session: &mut EmbySessionOutput,
    cmd: &WireCommand,
) -> Result<(), String> {
    match cmd {
        WireCommand::TogglePause => {
            transport(session, "PlayPause");
            Ok(())
        }
        WireCommand::Seek(delta) => {
            let target = session
                .estimate_ticks(Instant::now())
                .saturating_add(seconds_to_ticks(*delta));
            seek(session, target);
            Ok(())
        }
        WireCommand::SeekAbsolute(seconds) => {
            seek(session, seconds_to_ticks(*seconds));
            Ok(())
        }
        WireCommand::SetVolume(volume) => {
            let volume = (*volume).clamp(0, EMBY_SESSION_VOLUME_MAX);
            submit(session, "set_volume", move |api, session_id| {
                api.set_volume(session_id, volume)
            });
            Ok(())
        }
        WireCommand::SetMute(muted) => {
            let muted = *muted;
            submit(session, "set_mute", move |api, session_id| {
                api.set_mute(session_id, muted)
            });
            Ok(())
        }
        WireCommand::SetAudio(index) => {
            let index = *index;
            submit(session, "set_audio_index", move |api, session_id| {
                api.set_audio_index(session_id, index)
            });
            Ok(())
        }
        WireCommand::SetSub(index) => {
            // The wire index 0 means off; Emby spells off as −1.
            let index = if *index == 0 { -1 } else { *index };
            submit(session, "set_subtitle_index", move |api, session_id| {
                api.set_subtitle_index(session_id, index)
            });
            Ok(())
        }
        WireCommand::SetSubtitlePrefs { .. } => {
            Err("subtitle preferences are not supported on an Emby Session output".to_string())
        }
        WireCommand::NextUpShow { .. }
        | WireCommand::NextUpDismiss
        | WireCommand::SkipIntroDismiss => {
            // Documented no-op: these drive the mpv next-up and skip-intro
            // overlays, which a Session has no counterpart for.
            Ok(())
        }
        WireCommand::JumpTo(_) => {
            unreachable!("inbound WireCommand::JumpTo is rejected before conversion (design D6)")
        }
    }
}

/// `PlaybackIntentAction::SetPaused` on a Session output (design D9): the
/// desired state translates to an explicit device pause or unpause, and no
/// command goes out when the last observation already reports that state. The
/// caller settles the guarded intent at once; the device's own pause change
/// reports through the poll (design D5), not through a run event.
pub(crate) fn set_paused(session: &mut EmbySessionOutput, paused: bool) {
    if session
        .last_observation
        .as_ref()
        .map(|observed| observed.is_paused)
        == Some(paused)
    {
        return;
    }
    transport(session, if paused { "Pause" } else { "Unpause" });
}

/// Routes a next or previous on content the Owner did not load (design D9):
/// the Session plays an item that is not the loaded one, so the device gets
/// its own next/previous command and the Owner loads nothing. Returns whether
/// this was the device's step; a step on the loaded item is the Owner's
/// relative step and returns false.
pub(crate) fn route_step(session: &mut EmbySessionOutput, direction: Direction) -> bool {
    if !plays_foreign_item(session) {
        return false;
    }
    transport(
        session,
        match direction {
            Direction::Next => "NextTrack",
            Direction::Previous => "PreviousTrack",
        },
    );
    true
}

/// Whether the Session plays an item the Owner did not load (design D6, D9):
/// the observation presents an item, and it is not the loaded item.
fn plays_foreign_item(session: &EmbySessionOutput) -> bool {
    if !has_presented_item(session) {
        return false;
    }
    !plays_loaded_item(session)
}

/// Whether the device currently presents the item the Owner loaded (design
/// D6, D9, D11): the observation's now-playing item is the loaded one. A
/// missing observation and an idle Session present nothing.
fn plays_loaded_item(session: &EmbySessionOutput) -> bool {
    let Some(item_id) = session
        .last_observation
        .as_ref()
        .and_then(|observed| observed.now_playing_item_id.as_deref())
    else {
        return false;
    };
    session
        .loaded
        .as_ref()
        .is_some_and(|loaded| loaded.item_id == item_id)
}

/// Whether the observation presents any item at all.
fn has_presented_item(session: &EmbySessionOutput) -> bool {
    session
        .last_observation
        .as_ref()
        .is_some_and(|observed| observed.now_playing_item_id.is_some())
}

/// Stops the item the Owner loaded on the Session (design D9, D11; task 6.2):
/// the device gets `transport("Stop")` only while it plays that item, because
/// content the Owner did not load is left playing. Returns whether a stop was
/// sent, so the caller can tell a stopped item from a left-running device.
pub(crate) fn stop_loaded_item(session: &EmbySessionOutput) -> bool {
    if !plays_loaded_item(session) {
        return false;
    }
    transport(session, "Stop");
    true
}

/// Submits one seek, clamped at the start of the item (design D9).
fn seek(session: &EmbySessionOutput, ticks: i64) {
    let ticks = ticks.max(0);
    submit(session, "seek", move |api, session_id| {
        api.seek(session_id, ticks)
    });
}

/// Submits one transport command on the output's worker.
fn transport(session: &EmbySessionOutput, command: &'static str) {
    submit(session, command, move |api, session_id| {
        api.transport(session_id, command)
    });
}

/// Submits one `/Sessions` command job (design D4, D9): the operation runs on
/// the worker thread that owns the API, so commands reach the device in the
/// order the user sent them. The transport path has no reply route, so a
/// refused command is logged; the owning poll reports a disconnected device.
fn submit(
    session: &EmbySessionOutput,
    command: &'static str,
    op: impl FnOnce(&dyn SessionApi, &str) -> Result<(), String> + Send + 'static,
) {
    let Some(session_id) = session.session_id.clone() else {
        tracing::warn!(
            name: "daemon.emby_session_command.no_session",
            target: "daemon",
            command,
            "the Emby Session output has no session id; the command is dropped"
        );
        return;
    };
    let job = Box::new(move |api: &dyn SessionApi| {
        if let Err(error) = op(api, &session_id) {
            tracing::warn!(
                name: "daemon.emby_session_command.failed",
                target: "daemon",
                command,
                error = %error,
                "the Emby Session refused the command"
            );
        }
    });
    if session.worker.send(job).is_err() {
        tracing::warn!(
            name: "daemon.emby_session_command.worker_gone",
            target: "daemon",
            command,
            "the Emby Session worker is not running; the command is dropped"
        );
    }
}
