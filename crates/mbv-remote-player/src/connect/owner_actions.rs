//! One-shot restricted-role requests to this machine's running Local Owner:
//! the service-setup signal and the owner action. Each call opens its own
//! connection, handshakes with its restricted role, sends one command, and
//! awaits one reply. Split out of `connect.rs` so each file stays under the
//! size bar.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use mbv_ctrl::{CtrlCmd, CtrlEvent, OwnerAction};
use mbv_net::stream::SocketStream;

use super::handshake::{
    HandshakeRole, PeerBuild, perform_service_setup_admin_handshake, read_initial_state,
    read_server_hello, send_client_hello,
};

/// Best-effort signal to a running same-user Local daemon to reread its own
/// owner-local Service storage. A single non-blocking connect attempt is made;
/// when no Local daemon is reachable the call returns `Ok(())` so a bare-mode
/// commit proceeds without a daemon. When a daemon is reachable, the
/// control-auth handshake runs, `ApplyServiceSetup` is sent, and the
/// applied/rejected acknowledgement is awaited. Any failure after the connect
/// reports a restart requirement; the caller's durable commit is untouched.
pub fn signal_local_daemon_service_setup(
    kind: mbv_queue::ServiceKind,
    revision: u64,
) -> Result<(), crate::RemotePlayerError> {
    let path = PathBuf::from(mbv_config::control_socket_path());
    let Ok(stream) = UnixStream::connect(&path) else {
        return Ok(());
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(6)))
        .map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (cannot read local daemon ctrl): {error}"
            ))
        })?;
    let (mut reader, _state, _compatibility) =
        perform_service_setup_admin_handshake(SocketStream::Unix(stream), || {
            Ok(mbv_config::load_or_create_control_credential()?)
        })
        .map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (local daemon handshake failed): {error}"
            ))
        })?;
    let request =
        serde_json::to_string(&CtrlCmd::ApplyServiceSetup { kind, revision }).map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (cannot serialize setup request): {error}"
            ))
        })?;
    writeln!(reader.get_mut(), "{request}")
        .and_then(|()| reader.get_mut().flush())
        .map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (cannot send setup request): {error}"
            ))
        })?;
    await_service_setup_acknowledgement(&mut reader)
}

fn await_service_setup_acknowledgement(
    reader: &mut BufReader<SocketStream>,
) -> Result<(), crate::RemotePlayerError> {
    for next in reader.lines() {
        let line = next.map_err(|_error| {
            crate::RemotePlayerError::restart_required(
                "restart required (setup acknowledgement unavailable)",
            )
        })?;
        let event = serde_json::from_str::<CtrlEvent>(&line).map_err(|_error| {
            crate::RemotePlayerError::restart_required(
                "restart required (invalid setup acknowledgement)",
            )
        })?;
        match event {
            CtrlEvent::ServiceSetupApplied { .. } => return Ok(()),
            CtrlEvent::ServiceSetupRejected { reason, .. } => {
                return Err(crate::RemotePlayerError::restart_required(format!(
                    "restart required (live setup rejected: {reason:?})"
                )));
            }
            _ => {}
        }
    }
    Err(crate::RemotePlayerError::restart_required(
        "restart required (setup acknowledgement unavailable)",
    ))
}

/// Runs one Owner action on this machine's running Local Owner process
/// (tray-pin-swap design D7): the same path the Tray uses. Connects to
/// `control_socket_path()`, handshakes with the [`HandshakeRole::OwnerAction`
/// role][HandshakeRole], sends `RunOwnerAction`, and awaits the reply under a
/// 6 s read timeout. No socket reports that no mbv runs; a server Hello
/// without the `owner-action` capability reports a restart requirement before
/// anything is sent, so an Owner that predates Owner actions receives no
/// action.
pub fn run_local_owner_action(action: OwnerAction) -> Result<(), crate::RemotePlayerError> {
    let path = PathBuf::from(mbv_config::control_socket_path());
    let Ok(stream) = UnixStream::connect(&path) else {
        return Err(crate::RemotePlayerError::connection("no running mbv"));
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(6)))
        .map_err(|error| {
            crate::RemotePlayerError::connection(format!(
                "cannot set the local daemon read timeout: {error}"
            ))
        })?;
    let mut reader = BufReader::new(SocketStream::Unix(stream));
    run_owner_action_handshake(&mut reader, action, || {
        Ok(mbv_config::load_or_create_control_credential()?)
    })?;
    send_owner_action(&mut reader, action)
}

/// The Owner-action handshake: reads the server Hello, refuses an Owner
/// without the `owner-action` capability before anything is sent, sends the
/// Client Hello with the [`HandshakeRole::OwnerAction`] role, and consumes
/// the initial state event the Owner sends on admission.
pub(super) fn run_owner_action_handshake<F>(
    reader: &mut BufReader<SocketStream>,
    action: OwnerAction,
    load_control_token: F,
) -> Result<(), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    let compatibility = read_server_hello(reader, PeerBuild::Any)?;
    if !compatibility.supports_owner_action {
        return Err(crate::RemotePlayerError::restart_required(format!(
            "the running mbv is too old for {}; restart it",
            action.cli_flag()
        )));
    }
    send_client_hello(
        reader,
        &compatibility,
        load_control_token,
        HandshakeRole::OwnerAction,
        false,
        None,
    )?;
    read_initial_state(reader).map(|_| ())
}

pub(super) fn send_owner_action(
    reader: &mut BufReader<SocketStream>,
    action: OwnerAction,
) -> Result<(), crate::RemotePlayerError> {
    let request = serde_json::to_string(&CtrlCmd::RunOwnerAction(action))
        .map_err(|error| crate::RemotePlayerError::protocol(error.to_string()))?;
    writeln!(reader.get_mut(), "{request}")
        .and_then(|()| reader.get_mut().flush())
        .map_err(|error| {
            crate::RemotePlayerError::connection(format!("cannot send the owner action: {error}"))
        })?;
    await_owner_action_reply(reader)
}

/// Awaits `OwnerActionAccepted` / `OwnerActionRefused` on the reply path,
/// under the 6 s read timeout `run_local_owner_action` set on the stream.
/// Other events (an Owner pushing state between the handshake and the reply)
/// are skipped, mirroring `await_service_setup_acknowledgement`.
fn await_owner_action_reply(
    reader: &mut BufReader<SocketStream>,
) -> Result<(), crate::RemotePlayerError> {
    for next in reader.lines() {
        let line = next.map_err(|_error| {
            crate::RemotePlayerError::connection("owner-action reply unavailable")
        })?;
        let event = serde_json::from_str::<CtrlEvent>(&line)
            .map_err(|_error| crate::RemotePlayerError::connection("invalid owner-action reply"))?;
        match event {
            CtrlEvent::OwnerActionAccepted => return Ok(()),
            CtrlEvent::OwnerActionRefused { reason } => {
                return Err(crate::RemotePlayerError::connection(format!(
                    "the Owner refused the action: {reason}"
                )));
            }
            _ => {}
        }
    }
    Err(crate::RemotePlayerError::connection(
        "owner-action reply unavailable",
    ))
}
