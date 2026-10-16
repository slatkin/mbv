//! Routing transport commands onto the Cast receiver (cast-owner-output
//! design D4, row 5.3): the transport commands act on the receiver, track
//! selection has no cast primitive and is rejected, and the mpv-script
//! overlays are a documented no-op.

use mbv_cast::{CastError, CastTransport};

use super::CastOutput;
use super::PlaybackOutput;
use crate::ctrl::{CtrlSender, send_to};

/// The cast side of `CtrlCmd::PlayerCmd` (design D4, row 5.3). Local
/// forwards the command to the run unchanged; Cast routes through
/// `route_cast_command` with the ctrl reply path for the rejections.
pub fn route_player_cmd(ctx: &mut crate::CtrlContext<'_>, cmd: mbv_ctrl::WireCommand) {
    let PlaybackOutput::Cast(cast) = &mut ctx.owner.output else {
        ctx.player
            .send_command(mbv_ctrl::player::PlayerCommand::from(cmd));
        return;
    };
    route_cast_command(cast, &cmd, Some(ctx.reply_tx));
}

/// The transport side of a tray/MPRIS `TransportCommand::Player` (design D4,
/// row 5.3): Local forwards the command to the run unchanged. On a Cast
/// output a command with a wire form routes like the matching ctrl wire
/// command; a local-only command (slot jumps, run playlist submission, the
/// mpv next-up overlays) has no receiver counterpart and is a documented
/// no-op — the canonical queue paths are output-agnostic.
pub fn route_transport_command(
    player: &mbv_player::Player,
    output: &mut super::PlaybackOutput,
    command: mbv_ctrl::player::PlayerCommand,
) {
    let PlaybackOutput::Cast(cast) = output else {
        player.send_command(command);
        return;
    };
    match mbv_ctrl::WireCommand::try_from_player_command(command) {
        Ok(wire) => route_cast_command(cast, &wire, None),
        Err(command) => {
            tracing::debug!(
                name: "cast.transport_command.ignored",
                target: "cast",
                command = ?command,
                "local-only player command has no receiver counterpart"
            );
        }
    }
}

/// `PlaybackIntentAction::SetPaused` on a Cast output (row 5.3 correction:
/// spec "play, pause, next, previous, and stop from either Client act on
/// that receiver"): the desired state translates to an explicit receiver
/// pause or play — the intent carries the state, so no toggle is needed. The
/// caller settles the guarded intent; the receiver's own pause change
/// reports through the status poll (design D8), not through a run event.
pub fn set_receiver_paused(output: &mut super::PlaybackOutput, paused: bool) {
    let PlaybackOutput::Cast(cast) = output else {
        return;
    };
    // Already in the desired state: no command, the caller still settles the
    // intent (the local path settles a matching state without a command too).
    if matches!(
        cast.last_status.as_ref().map(|status| status.state),
        Some(mbv_cast::client::CastPlaybackState::Paused)
    ) == paused
    {
        return;
    }
    submit_command_job(cast, "set_paused", move |transport| {
        if paused {
            transport.pause()
        } else {
            transport.play()
        }
    });
}

/// Routes one wire command onto the Cast receiver (design D4, row 5.3). The
/// transport commands act on the receiver — `TogglePause` picks its direction
/// from the receiver's last reported state, a relative `Seek` starts from the
/// receiver's reported position, and the 0-100 wire volume scales to the
/// receiver's 0-1 level; track selection has no cast primitive and is
/// rejected with a reason when a reply path exists; the next-up and
/// skip-intro overlays are mpv script messages with no receiver counterpart,
/// a documented no-op. `JumpTo` never reaches here: the dispatcher rejects it
/// before conversion.
fn route_cast_command(
    cast: &mut CastOutput,
    cmd: &mbv_ctrl::WireCommand,
    reply: Option<&CtrlSender>,
) {
    match cmd {
        mbv_ctrl::WireCommand::TogglePause => {
            let paused = matches!(
                cast.last_status.as_ref().map(|status| status.state),
                Some(mbv_cast::client::CastPlaybackState::Paused)
            );
            submit_command_job(cast, "toggle_pause", move |transport| {
                if paused {
                    transport.play()
                } else {
                    transport.pause()
                }
            });
        }
        mbv_ctrl::WireCommand::Seek(delta) => {
            let base = f64::from(
                cast.last_status
                    .as_ref()
                    .and_then(|status| status.position_seconds)
                    .unwrap_or(0.0),
            );
            submit_seek(cast, (base + *delta).max(0.0));
        }
        mbv_ctrl::WireCommand::SeekAbsolute(seconds) => {
            submit_seek(cast, *seconds);
        }
        mbv_ctrl::WireCommand::SetVolume(volume) => {
            let volume = u8::try_from((*volume).clamp(0, 100)).unwrap_or(u8::MAX);
            let level = f32::from(volume) / 100.0;
            submit_command_job(cast, "set_volume", move |transport| {
                transport.set_volume(level)
            });
        }
        mbv_ctrl::WireCommand::SetMute(muted) => {
            let muted = *muted;
            submit_command_job(cast, "set_muted", move |transport| {
                transport.set_muted(muted)
            });
        }
        mbv_ctrl::WireCommand::SetAudio(_)
        | mbv_ctrl::WireCommand::SetSub(_)
        | mbv_ctrl::WireCommand::SetSubtitlePrefs { .. } => {
            refuse_track_command(reply, cmd);
        }
        mbv_ctrl::WireCommand::NextUpShow { .. }
        | mbv_ctrl::WireCommand::NextUpDismiss
        | mbv_ctrl::WireCommand::SkipIntroDismiss => {
            // Documented no-op: these drive the mpv next-up and skip-intro
            // overlays, which a receiver has no counterpart for.
        }
        mbv_ctrl::WireCommand::JumpTo(_) => {
            unreachable!("inbound WireCommand::JumpTo is rejected before conversion (design D6)")
        }
    }
}

/// Submits one seek to the receiver (design D4): the requested seconds are
/// narrowed to the receiver protocol's f32, and an out-of-range value drops
/// the seek. The seek arms the settle marker only while an item is loaded:
/// a seek with nothing loaded has no slot whose settle could mean anything
/// (design D9). The bound is the next poll id, so only a status whose poll
/// was issued after the seek settles it — a poll still in flight at arm time
/// carries a pre-seek snapshot.
fn submit_seek(cast: &mut CastOutput, requested_seconds: f64) {
    let Some(target) = cast_seconds_f32(requested_seconds) else {
        tracing::warn!(
            name: "cast.seek.invalid",
            target: "cast",
            seconds = requested_seconds,
            "the seek position is outside the receiver's range; the seek is dropped"
        );
        return;
    };
    if cast.loaded.is_some() {
        cast.pending_seek = Some(cast.next_poll_id);
    }
    submit_command_job(cast, "seek", move |transport| transport.seek(target));
}

/// Refuses one audio or subtitle track-selection command (design D4): a
/// receiver has no track primitives. The refusal reaches the requesting
/// Client when a reply path exists; the transport path has none, so the
/// refusal is logged and a track command is never silently swallowed.
fn refuse_track_command(reply: Option<&CtrlSender>, cmd: &mbv_ctrl::WireCommand) {
    if let Some(reply) = reply {
        send_to(
            reply,
            &mbv_ctrl::CtrlEvent::CommandRejected(
                "audio and subtitle track selection is not supported on a cast receiver"
                    .to_string(),
            ),
        );
    } else {
        tracing::debug!(
            name: "cast.track_command.refused",
            target: "cast",
            command = ?cmd,
            "track selection has no receiver counterpart"
        );
    }
}

/// Narrows a wire seconds value to the receiver protocol's f32 (`rust_cast`'s
/// seek is f32; no lossless f64-to-f32 conversion exists). A value outside
/// the f32 range — or not a number — is no valid seek position: the caller
/// drops the seek instead of issuing one.
fn cast_seconds_f32(value: f64) -> Option<f32> {
    num_traits::cast(value)
}

/// Submits one receiver-command job (design D4, row 5.3): the operation runs
/// on the Cast worker against the transport it owns. A failure is logged and
/// sent back to the owner as `CastOwnerEvent::CommandFailed`, which names it
/// to every Client (spec "Command is not supported by the receiver"). A dead
/// connection is reported by the next status poll, which disconnects and
/// reconnects (design D8).
pub(super) fn submit_command_job(
    cast: &CastOutput,
    command: &'static str,
    op: impl FnOnce(&mut dyn CastTransport) -> Result<(), CastError> + Send + 'static,
) {
    let events = cast.events.clone();
    let _ = cast
        .worker
        .send(Box::new(move |transport: &mut dyn CastTransport| {
            if let Err(error) = op(transport) {
                tracing::warn!(
                    name: "cast.command.failed",
                    target: "cast",
                    command,
                    error = %error,
                    "the receiver refused the command"
                );
                let _ = events.send(crate::DaemonEvent::Cast(
                    super::CastOwnerEvent::CommandFailed {
                        error: error.to_string(),
                    },
                ));
            }
        }));
}
