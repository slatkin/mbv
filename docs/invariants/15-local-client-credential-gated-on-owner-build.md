# Invariant 15 — Local Client credential is sent only after the Owner version-and-wire gate

## The invariant

When a local Client attaches to the Owner process, the ctrl handshake must
reject a mismatched Owner *before* the Client transmits its hello or control
credential. The comparison happens on the Owner's hello, inside the Client's
read of the handshake. Nothing identifying or authorizing leaves the Client
until the comparison has passed. The gate compares two values: the
application version and the wire identity. The build identity never takes
part; it is for display only.

## Why it matters

The gate exists (issue #559; the Owner mismatch error and the wire identity
from the `owner-gate-on-wire-identity` change) so a terminal discovering an
outdated Owner never pays credentials into a process it is about to ask the
user to kill. The gate moved from the build identity to the wire identity
because the build identity changes with every commit and every dirty build: a
Client-only rebuild that left the ctrl wire unchanged was refused without
cause. A differing application version is the ordinary stale process. A
differing wire identity is an Owner whose ctrl messages this Client cannot
read. An Owner that predates the wire-identity field sends an empty value and
is refused the same way. Nothing in the type system orders the two handshake
steps: moving the check after `send_client_hello` compiles and passes every
functional test. The property is upheld only by statement order in the
handshake path.

## How the code maintains it today

- `read_server_hello` in
  `crates/mbv-remote-player/src/connect/handshake.rs` refuses under
  `PeerBuild::MustMatch` when `BuildInfo::current().differs_for_gate()`
  returns true for the Owner's `BuildInfo`, before `validate_peer` and
  `send_client_hello` run. `differs_for_gate` (`crates/mbv-ctrl/src/
  build_info.rs`) compares the application version and the wire identity and
  nothing else. The error is `RemotePlayerErrorKind::OwnerMismatch` (code
  `remote-player.owner_mismatch`). Its `Display` names the application
  version, build identity, and wire identity of the Owner and of this Client,
  with an empty value shown as unknown.
- `PeerBuild::MustMatch` scopes the gate to every handshake to
  `control_socket_path()`: the Client attach, and the two restricted roles —
  service-setup admin and owner action — because they pay the same Control
  credential. Explicit `unix://`/`tcp://` endpoints and packaged mbvd are
  `PeerBuild::Any` and compare nothing.
- An empty Owner `wire_id` never equals this binary's non-empty value, so a
  pre-change Owner is refused as a mismatch rather than accepted on the
  version alone.
- After a pass, the handshake carries the Owner's `BuildInfo` into
  `RemotePlayer::owner_build`. The F2 Settings Session version rows show it.
- Regression tests live in
  `crates/mbv-remote-player/src/connect/tests/owner_gate.rs`. Each refusal
  case — a different version, a different wire identity, an Owner without a
  wire identity — asserts that the peer receives an empty client hello
  (nothing transmitted). The accept case asserts that a different build
  identity with the same version and wire identity attaches, and that
  `owner_build()` reports the Owner's values.
- Stopping a mismatched Owner never needs the handshake: `mbv -q` signals
  the lock-file PID, so the gate cannot deadlock recovery.
- `CTRL_PROTOCOL_VERSION` and `validate_peer` are independent of the gate; a
  protocol-only difference is not an `OwnerMismatch`.

## Where it currently fails

No known violation of the ordering. It is conventional, not enforced: any
future handshake restructuring must keep the comparison ahead of the first
credential write.

A behavior-only Owner change stays outside the gate's reach. Such an Owner
passes the comparison and attaches without a prompt, so a test can run
against old Owner logic. The design accepts this trade-off. The F2 Settings
Session version rows show the build identity of both sides; that display is
the only signal of a behavior-only difference.
