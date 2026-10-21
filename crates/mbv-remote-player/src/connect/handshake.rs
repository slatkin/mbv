//! Control-protocol handshake with a daemon: server hello validation, role
//! selection, client hello, and the initial state read. Split out of
//! `connect.rs` so each file stays under the size bar.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::time::{Duration, Instant};

use mbv_ctrl::{CtrlCmd, CtrlCompatibility, CtrlEvent, CtrlHello, DisconnectReason};
use mbv_net::stream::SocketStream;

// Hard wall-clock bound on the post-connect protocol handshake (hello
// exchange + initial state), independent of `endpoint::DAEMON_TCP_CONNECT_TIMEOUT`
// -- that constant only bounds the initial TCP-level connect, not the
// blocking `read_line` calls that follow it (issue #191 fix #5). A stalled
// daemon on localhost/LAN (user-configured, not a public/flaky server) is a
// rarer and more clearly-broken scenario than a slow Emby server, so this is
// tighter than `EmbyClient::AUTHENTICATE_HARD_BOUND`.
pub(super) const DAEMON_HANDSHAKE_HARD_BOUND: Duration = Duration::from_secs(5);

/// Whether the handshake must reject an Owner built from a different version.
///
/// The local Owner process is the same binary as this Client, so a differing
/// `app_version` means a stale process; explicit `unix://`/`tcp://` endpoints
/// and the packaged `mbvd` are independent builds and never compare.
#[derive(Clone, Copy)]
pub(super) enum PeerBuild {
    Any,
    MustMatch,
}

/// Which non-Client role a handshake plays, mirroring the daemon's
/// `CtrlConnectionRole` (`mbv-daemon/src/ctrl.rs`): a Client connects as a
/// full control peer; the two restricted roles each allow only their own
/// command (`ApplyServiceSetup` / `RunOwnerAction`) and are never counted as
/// a Client by the Owner (tray-pin-swap design D7).
#[derive(Clone, Copy)]
pub(super) enum HandshakeRole {
    Client,
    ServiceSetupAdmin,
    OwnerAction,
}

/// Test-only entry point to the handshake, on `stream`, with
/// [`PeerBuild::Any`]. Production goes through `connect_endpoint`, which
/// runs [`perform_handshake_with_role`] on a worker thread bounded by
/// `DAEMON_HANDSHAKE_HARD_BOUND` (issue #191 fix #5).
#[cfg(test)]
pub(super) fn perform_handshake<F>(
    stream: SocketStream,
    load_control_token: F,
) -> Result<(BufReader<SocketStream>, CtrlEvent, CtrlCompatibility), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    perform_handshake_with_role(
        stream,
        load_control_token,
        HandshakeRole::Client,
        PeerBuild::Any,
        false,
        None,
    )
}

pub(super) fn perform_service_setup_admin_handshake<F>(
    stream: SocketStream,
    load_control_token: F,
) -> Result<(BufReader<SocketStream>, CtrlEvent, CtrlCompatibility), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    perform_handshake_with_role(
        stream,
        load_control_token,
        HandshakeRole::ServiceSetupAdmin,
        PeerBuild::Any,
        false,
        None,
    )
}

pub(super) fn perform_handshake_with_role<F>(
    stream: SocketStream,
    load_control_token: F,
    role: HandshakeRole,
    peer_build: PeerBuild,
    pinned: bool,
    swap_token: Option<String>,
) -> Result<(BufReader<SocketStream>, CtrlEvent, CtrlCompatibility), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    let mut reader = BufReader::new(stream);
    let ctrl_compatibility = read_server_hello(&mut reader, peer_build)?;
    send_client_hello(
        &mut reader,
        &ctrl_compatibility,
        load_control_token,
        role,
        pinned,
        swap_token,
    )?;
    let state_event = read_initial_state(&mut reader)?;

    Ok((reader, state_event, ctrl_compatibility))
}

pub(super) fn read_server_hello(
    reader: &mut BufReader<SocketStream>,
    peer_build: PeerBuild,
) -> Result<CtrlCompatibility, crate::RemotePlayerError> {
    let mut first_line = String::new();
    reader.read_line(&mut first_line).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("failed to read daemon protocol hello: {e}"))
    })?;
    if first_line.trim().is_empty() {
        return Err(crate::RemotePlayerError::protocol(
            "daemon closed connection before protocol hello",
        ));
    }
    let hello = serde_json::from_str::<CtrlEvent>(first_line.trim_end()).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("invalid daemon protocol hello: {e}"))
    })?;
    let CtrlEvent::Hello(info) = hello else {
        return Err(crate::RemotePlayerError::protocol(
            "daemon did not send protocol hello",
        ));
    };
    // A local Owner is the same binary as this Client, so a differing
    // `app_version` means the user is attached to a stale process. Refuse
    // here, before `validate_peer` and therefore before `send_client_hello`,
    // so no control credential leaves this terminal. Both sides read the
    // workspace version (`version.workspace = true`), so this compares like
    // with like.
    if matches!(peer_build, PeerBuild::MustMatch) && info.app_version != env!("CARGO_PKG_VERSION") {
        return Err(crate::RemotePlayerError::owner_build_mismatch(
            info.app_version,
        ));
    }
    info.validate_peer()?;
    let mut compatibility = info.compatibility()?;
    compatibility.supports_lifecycle_shutdown = info.supports_lifecycle_shutdown();
    compatibility.supports_audio_only = info.supports_audio_only();
    compatibility.supports_control_auth = info.supports_control_auth();
    compatibility.supports_owner_queue_load = info.supports_owner_queue_load();
    compatibility.supports_answered_queue_ops = info.supports_answered_queue_ops();
    compatibility.supports_owner_action = info.supports_owner_action();
    compatibility.supports_queue_shuffle = info.supports_queue_shuffle();
    compatibility.supports_playback_output = info.supports_playback_output();
    compatibility.supports_emby_session_output = info.supports_emby_session_output();
    tracing::info!(name: "remote.daemon_protocol_validation.succeeded", target: "remote", protocol_version = info.protocol_version, app_version = %info.app_version, capabilities = ?info.capabilities, "daemon protocol validated");
    Ok(compatibility)
}

pub(super) fn send_client_hello<F>(
    reader: &mut BufReader<SocketStream>,
    compatibility: &CtrlCompatibility,
    load_control_token: F,
    role: HandshakeRole,
    pinned: bool,
    swap_token: Option<String>,
) -> Result<(), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    let control_token = compatibility
        .supports_control_auth
        .then(load_control_token)
        .transpose()?;
    // The Pin-swap capabilities and token belong to a Client connection only
    // (tray-pin-swap D2/D6); the restricted-role handshakes stay plain.
    let mut client_hello = match role {
        HandshakeRole::ServiceSetupAdmin => CtrlHello::current_service_setup_admin(control_token),
        HandshakeRole::OwnerAction => {
            CtrlHello::current_owner_action(control_token.ok_or_else(|| {
                crate::RemotePlayerError::protocol(
                    "owner-action handshake requires a Control credential",
                )
            })?)
        }
        HandshakeRole::Client => {
            if let Some(control_token) = control_token {
                CtrlHello::current_control_client(control_token).with_pin_swap(pinned, swap_token)
            } else {
                CtrlHello::current().with_pin_swap(pinned, swap_token)
            }
        }
    };
    // A Client connection negotiates `playback-output` (cast-owner-output
    // design D3): the claim is what tells the Owner to cold-seed this
    // connection with `CtrlEvent::PlaybackOutput` right after the initial
    // state and include it in every output broadcast. It also negotiates
    // `emby-session-output`: without that claim the Owner masks an Emby
    // Session output to `Local` for this Client (design D3), so a selected
    // Session appears to do nothing. Restricted-role hellos stay plain, so
    // the Owner never seeds them an unread line.
    if matches!(role, HandshakeRole::Client) {
        client_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_PLAYBACK_OUTPUT.to_string());
        client_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_EMBY_SESSION_OUTPUT.to_string());
    }
    client_hello.protocol_version = compatibility.client_protocol_version;
    let client_hello = serde_json::to_string(&CtrlCmd::Hello(client_hello))
        .map_err(|e| crate::RemotePlayerError::protocol(e.to_string()))?;
    // Write via the same handle the `BufReader` wraps (`get_mut()`) rather
    // than a second `try_clone()`'d handle -- the handshake is strictly
    // sequential (read hello -> write client hello -> read state) with no
    // concurrent access from another thread during this phase, so there's
    // nothing a second handle buys here beyond an extra fallible call.
    writeln!(reader.get_mut(), "{client_hello}").map_err(|e| {
        crate::RemotePlayerError::protocol(format!("failed to send daemon protocol hello: {e}"))
    })
}

pub(super) fn read_initial_state(
    reader: &mut BufReader<SocketStream>,
) -> Result<CtrlEvent, crate::RemotePlayerError> {
    let mut state_line = String::new();
    reader.read_line(&mut state_line).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("failed to read daemon initial state: {e}"))
    })?;
    if state_line.trim().is_empty() {
        return Err(crate::RemotePlayerError::protocol(
            "daemon closed connection before initial state",
        ));
    }
    let event = serde_json::from_str::<CtrlEvent>(state_line.trim_end()).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("invalid daemon initial state: {e}"))
    })?;
    match event {
        CtrlEvent::Disconnected {
            reason: DisconnectReason::ExclusiveOwner { pid },
        } => Err(crate::RemotePlayerError::exclusive_owner(pid)),
        CtrlEvent::Disconnected {
            reason: DisconnectReason::OwnerShuttingDown,
        } => Err(crate::RemotePlayerError::owner_shutting_down()),
        event => Ok(event),
    }
}

/// How long the handshake will wait, past the initial state, for the
/// Owner's cold-seed playback-output event. An Owner that advertises
/// `playback-output` writes it immediately after the initial state, before
/// its command loop (`core_ctrl_spawn` order: state first, then the seed),
/// so under normal conditions the line is already in the socket buffer.
/// The timeout exists only so an absent or late seed can never hang the
/// connect: if it fires, the seed (or any line arriving late) simply
/// becomes the reader thread's first event instead.
pub(super) const PLAYBACK_OUTPUT_SEED_READ_TIMEOUT: Duration = Duration::from_millis(500);

/// Read the Owner's cold-seed playback-output event, tolerantly. Runs
/// inside the handshake's hard bound, under its own short deadline: a
/// timeout, an EOF, or an unparseable line all yield no event without
/// failing the connect — an absent or late seed simply becomes the reader
/// thread's first event instead, which is the pre-fix behavior and costs
/// nothing but an event beat. A line that IS read is returned whole for
/// the caller to apply, so a consumed line is never silently lost.
///
/// A partial line is never lost. The wait accumulates the bytes it reads
/// and stops consuming exactly at the first `'\n'`; when the deadline
/// expires before the line completes, whatever was consumed comes back in
/// [`SeedLineRead::consumed`] and the caller replays it to the reader
/// thread ahead of the live stream — so the reader thread still reads the
/// whole line exactly as the daemon sent it, one event beat late. (A
/// plain `read_line` under a read timeout would consume the prefix of a
/// line split across two arrivals and lose it when the second half times
/// out: `BufRead::read_line` consumes each buffered chunk as it goes, and
/// the partial `String` is discarded on the error.) The read timeout is
/// always cleared again before the reader thread takes the stream over,
/// so no lingering timeout can ever end the event loop early.
pub(super) fn read_playback_output_seed(
    reader: &mut BufReader<SocketStream>,
    timeout: Duration,
) -> SeedLineRead {
    if reader.get_mut().set_read_timeout(Some(timeout)).is_err() {
        // Can't bound the wait on this stream: don't risk hanging the
        // connect. The seed stays the reader thread's problem.
        return SeedLineRead::default();
    }
    let seed = wait_for_seed_line(reader, timeout);
    // Always clear the timeout before the reader thread takes over: a
    // lingering one would end the event loop on its first quiet moment.
    let _ = reader.get_mut().set_read_timeout(None);
    seed
}

/// What the tolerant seed wait took from the stream. `event` is set when a
/// full line arrived in time; `consumed` holds bytes taken from the stream
/// that the caller must replay to the reader thread (empty otherwise — on
/// success every consumed byte belongs to the returned line).
#[derive(Default)]
pub(super) struct SeedLineRead {
    pub(super) event: Option<CtrlEvent>,
    pub(super) consumed: Vec<u8>,
}

/// Waits, up to `timeout`, for a
/// `'\n'`-terminated line, consuming as it goes and stopping exactly at
/// the newline. See [`read_playback_output_seed`] for the contract.
fn wait_for_seed_line(reader: &mut BufReader<SocketStream>, timeout: Duration) -> SeedLineRead {
    let deadline = Instant::now() + timeout;
    // Bytes consumed so far. On the happy path they all belong to the
    // returned line; on a give-up they are replayed to the reader thread.
    let mut taken: Vec<u8> = Vec::new();
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return SeedLineRead {
                event: None,
                consumed: taken,
            };
        };
        let _ = reader.get_mut().set_read_timeout(Some(remaining));
        match reader.fill_buf() {
            // EOF: hand back what was taken (replayed), then the reader
            // thread observes the ending itself.
            Ok([]) => {
                return SeedLineRead {
                    event: None,
                    consumed: taken,
                };
            }
            Ok(chunk) => {
                if let Some(end) = chunk.iter().position(|&byte| byte == b'\n') {
                    // The line completes inside this chunk: take exactly up
                    // to and including the newline, leave the rest buffered
                    // for the reader thread, and the line is whole in
                    // `taken`.
                    taken.extend_from_slice(&chunk[..=end]);
                    reader.consume(end + 1);
                    return SeedLineRead {
                        event: parse_seed_line(&taken),
                        consumed: Vec::new(),
                    };
                }
                // No newline in this chunk: consume all of it and keep
                // waiting — the next `fill_buf` goes to the socket under
                // the remaining timeout.
                taken.extend_from_slice(chunk);
                let taken_so_far = chunk.len();
                reader.consume(taken_so_far);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            // Timed out or the stream errored: nothing more arrived in
            // time; what was taken is replayed, nothing is lost.
            Err(_) => {
                return SeedLineRead {
                    event: None,
                    consumed: taken,
                };
            }
        }
    }
}

/// Parses one whole seed line (newline included in `line`).
fn parse_seed_line(line: &[u8]) -> Option<CtrlEvent> {
    serde_json::from_slice::<CtrlEvent>(line.trim_ascii_end()).ok()
}
