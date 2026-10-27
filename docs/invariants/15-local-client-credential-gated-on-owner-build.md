# Invariant 15 — Local Client credential is sent only after the Owner build gate

## The invariant

When a local Client attaches to the local Owner process, the ctrl handshake
must reject a mismatched Owner build *before* the Client transmits its hello
or control credential. The build comparison happens on the Owner's hello,
inside the Client's read of the handshake; nothing identifying or
authorizing leaves the Client until the build check has passed. The gate
compares two values: the application version and the build identity.

## Why it matters

The gate exists (issue #559, design D1; the build identity added by issue
#949 item 3) so a terminal discovering an outdated Owner never pays
credentials into a process it is about to ask the user to kill. A differing
version is the ordinary stale process; a same version with a different
build identity is a rebuild at another commit, which the version alone
cannot see. An Owner that predates the build-id field sends an empty
identity and is refused the same way. Nothing in the type system orders the
two handshake steps: moving the check after `send_client_hello` compiles
and passes every functional test. The property is upheld only by statement
order in the handshake path.

## How the code maintains it today

- `read_server_hello` in
  `crates/mbv-remote-player/src/connect/handshake.rs` compares
  `CtrlHello.app_version` and `CtrlHello.build_id` against this binary
  before `send_client_hello` runs. `PeerBuild::MustMatch` scopes the gate to
  local Owner attachment: the Client connect when the endpoint is `Local`,
  and both restricted-role handshakes — service-setup admin and owner
  action — because they connect only to `control_socket_path()` and pay
  the same Control credential. Explicit `unix://`/`tcp://` endpoints and
  packaged mbvd are `PeerBuild::Any` (design D2/D3) and compare nothing.
  Stopping a mismatched Owner never needs the handshake: `mbv -q` signals
  the lock-file PID, so the gate cannot deadlock recovery.
- `build_id` is `BUILD_ID` from `crates/mbv-ctrl/build.rs`: the git short
  SHA, plus `-dirty` when `git status --porcelain` was non-empty at build
  time, or `unknown` when git is unavailable (tarball builds). A local
  Owner built from a tarball reports `unknown` on both sides and still
  attaches; two different tarball builds are indistinguishable.
- That value is re-embedded when the build script reruns, and the script
  watches git paths only, each emitted only when it exists: `HEAD`, the
  branch ref (the per-worktree path and the common-dir
  `refs/heads/<branch>`), `packed-refs`, and the per-worktree HEAD reflog
  (`logs/HEAD`). A literal `.git/HEAD` is never named, because in a
  worktree `.git` is a file and Cargo reruns the script on every build
  when a named path is missing. The reflog is what keeps the identity
  fresh when the branch ref is packed: `git pack-refs` folds the loose ref
  into `packed-refs`, which a commit never rewrites, while every commit
  appends to the reflog.
- An empty Owner `build_id` never equals this binary's non-empty identity,
  so a pre-change Owner is refused as a build mismatch rather than accepted
  on the version alone.
- Regression tests in `crates/mbv-remote-player/src/connect/tests.rs` assert
  the peer receives an empty client hello (nothing transmitted) on a version
  mismatch and on a same-version build-id mismatch, including the empty
  Owner identity a pre-change Owner sends.
- `CTRL_PROTOCOL_VERSION` and `validate_peer` are independent of the build
  gate; a protocol-only difference is not a build mismatch.

## Where it currently fails

The identity is commit-level, not binary-level: two builds of the same
commit carry the same `build_id`, so a rebuild at the same commit is not
detected. `-dirty` narrows the gap only when the build script reruns — the
marker is computed at build-script time, so a tree edited after that run
keeps the earlier marker until a HEAD, ref, or reflog change reruns the
script.

No known violation of the ordering. It is conventional, not enforced: any
future handshake restructuring must keep the build check ahead of the first
credential write.
