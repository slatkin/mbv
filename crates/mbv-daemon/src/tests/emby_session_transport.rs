// emby-session-owner-output row 4.4: transport routing onto an Emby Session
// (design D9). The table keeps only the rows whose routed value differs from
// the wire value: the subtitle "off" index, a relative seek clamped at the
// start, next on content the Owner did not load, the guarded pause, and the
// one command the Session refuses.

use rstest::rstest;

use super::cast_output::connect_output_client_with_caps;
use super::cast_playback::drained_events;
use super::emby_session_output::{session_output, wait_for_session_jobs};
use super::r#loop::TestLoop;
use super::*;

use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};

/// One transport route the table drives.
enum Route {
    /// A wire command, as `CtrlCmd::PlayerCmd` carries it.
    Wire(WireCommand),
    /// `PlaybackIntentAction::Next`.
    Next,
    /// `PlaybackIntentAction::SetPaused { paused }`.
    SetPaused(bool),
}

/// What the routed command left behind: the Session calls it made and the
/// refusal it reported, if any.
struct Routed {
    calls: Vec<SessionCall>,
    rejection: Option<String>,
}

/// A Session whose device already plays content the Owner did not load, so
/// the next row takes the device's own next command.
fn foreign_session() -> mbv_emby::SessionInfo {
    let mut scripted = session("device-1", "session-1");
    scripted.now_playing = Some("Other".to_string());
    scripted.now_playing_item_id = Some("other-item".to_string());
    scripted
}

/// Drives one route through the owner's real command paths: the wire command
/// through `CtrlCmd::PlayerCmd`, the step and the guarded pause through
/// `CtrlCmd::PlaybackIntent`.
fn drive(
    t: &mut TestLoop,
    client_id: u64,
    reply_tx: CtrlSender,
    rx: &mpsc::Receiver<crate::ctrl::CtrlOutbound>,
    calls: &Arc<Mutex<Vec<SessionCall>>>,
    route: Route,
) -> Routed {
    let _ = match route {
        Route::Wire(command) => t.event_loop.handle_event(DaemonEvent::Ctrl(
            CtrlCmd::PlayerCmd(command),
            client_id,
            reply_tx,
        )),
        Route::Next => t.event_loop.handle_event(DaemonEvent::Ctrl(
            CtrlCmd::PlaybackIntent(PlaybackIntent {
                request_id: 1,
                generation: 1,
                action: PlaybackIntentAction::Next,
            }),
            client_id,
            reply_tx,
        )),
        Route::SetPaused(paused) => t.event_loop.handle_event(DaemonEvent::Ctrl(
            CtrlCmd::PlaybackIntent(PlaybackIntent {
                request_id: 2,
                generation: 2,
                action: PlaybackIntentAction::SetPaused { paused },
            }),
            client_id,
            reply_tx,
        )),
    };
    wait_for_session_jobs(&t.event_loop.owner.output);
    let rejection = drained_events(rx)
        .into_iter()
        .find_map(|event| match event {
            CtrlEvent::CommandRejected(reason) => Some(reason),
            _ => None,
        });
    Routed {
        calls: routed_calls(calls),
        rejection,
    }
}

/// The Session calls after the selection's device lookup.
fn routed_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<SessionCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| !matches!(call, SessionCall::GetSessions))
        .cloned()
        .collect()
}

/// Asserts the route's one outcome: the exact command, or the refusal.
fn assert_routed(expected: Result<SessionCall, &str>, routed: Routed) {
    match expected {
        Ok(call) => {
            assert_eq!(
                routed.calls,
                vec![call],
                "the Session receives the one routed command"
            );
            assert_eq!(routed.rejection, None, "the command is not refused");
        }
        Err(reason) => {
            assert!(
                routed.calls.is_empty(),
                "a refused command sends nothing: {:?}",
                routed.calls
            );
            let reported = routed.rejection.unwrap_or_default();
            assert!(
                reported.contains(reason),
                "the refusal names the reason: {reported}"
            );
        }
    }
}

/// Contract: row 4.4 / design D9 — each command reaches the Session as the
/// value the table names, and `SetSubtitlePrefs` is refused with a reason.
#[rstest]
#[case::subtitle_off_maps_to_the_disabled_index(
    Route::Wire(WireCommand::SetSub(0)),
    Ok(SessionCall::SetSubtitleIndex {
        session_id: "session-1".to_string(),
        index: -1,
    })
)]
#[case::relative_seek_past_the_start_clamps_at_zero(
    Route::Wire(WireCommand::Seek(-5.0)),
    Ok(SessionCall::Seek {
        session_id: "session-1".to_string(),
        ticks: 0,
    })
)]
#[case::next_on_foreign_content_is_the_device_next(
    Route::Next,
    Ok(SessionCall::Transport {
        session_id: "session-1".to_string(),
        command: "NextTrack".to_string(),
    })
)]
#[case::set_paused_true_sends_pause(
    Route::SetPaused(true),
    Ok(SessionCall::Transport {
        session_id: "session-1".to_string(),
        command: "Pause".to_string(),
    })
)]
#[case::subtitle_preferences_are_refused(
    Route::Wire(WireCommand::SetSubtitlePrefs {
        mode: "smart".to_string(),
        subtitle_lang: "en".to_string(),
        audio_lang: "en".to_string(),
    }),
    Err("subtitle preferences")
)]
fn transport_commands_route_per_the_session_table(
    #[case] route: Route,
    #[case] expected: Result<SessionCall, &'static str>,
) {
    let fake = FakeSessionApi::with_sessions(vec![foreign_session()]);
    let (mut t, calls) = session_output(fake);
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    let routed = drive(&mut t, client_id, reply_tx, &rx, &calls, route);

    assert_routed(expected, routed);
}
