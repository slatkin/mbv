# Invariant 15 — Local Client credential is sent only after the Owner build gate

## The invariant

When a local Client attaches to the local Owner process, the ctrl handshake
must reject a mismatched Owner build *before* the Client transmits its hello
or control credential. The version comparison happens on the Owner's hello,
inside the Client's read of the handshake; nothing identifying or
authorizing leaves the Client until the build check has passed.

## Why it matters

The gate exists (issue #559, design D1) so a terminal discovering an outdated
Owner never pays credentials into a process it is about to ask the user to
kill. Nothing in the type system orders the two handshake steps: moving the
check after `send_client_hello` compiles and passes every functional test.
The property is upheld only by statement order in the handshake path.

## How the code maintains it today

- `read_server_hello` in `crates/mbv-remote-player/src/connect.rs` performs
  the build comparison against `CtrlHello.app_version` before
  `send_client_hello` runs; `PeerBuild::MustMatch` scopes the gate to local
  Owner attachment (explicit `unix://`/`tcp://` endpoints and packaged mbvd
  are `PeerBuild::Any`, design D2/D3).
- Regression tests in `crates/mbv-remote-player/src/connect/tests.rs` assert
  the peer receives an empty client hello (nothing transmitted) on mismatch.
- `CTRL_PROTOCOL_VERSION` and `validate_peer` are independent of the build
  gate; a protocol-only difference is not a build mismatch.

## Where it currently fails

No known violation. The ordering is conventional, not enforced: any future
handshake restructuring must keep the build check ahead of the first
credential write.
