# Proposal

## Why

A local Client refuses to attach to the stay-alive Owner when the Owner's build identity (git short SHA plus `-dirty`) is different from its own. Every commit and every build from a dirty tree changes that value, so a TUI-only or GUI-only change, which does not change what goes between Client and Owner, still stops the attach. You must restart the Owner, which stops playback, each time you test a rebuilt Client. Issue #949 item 3 added the build check after a stale Owner made an Emby Session selection do nothing. The real risk is a different wire or a different release, not a different commit. Since the wire schemas were committed (#968), the wire has changed in only 3 commits, but the build identity changed in hundreds.

## What Changes

- **A wire identity.** Each binary computes a short, deterministic digest of its ctrl wire shape: the `CtrlCmd`, `CtrlEvent`, and `CtrlHello` schemas that `schemars` generates, without documentation annotations. `CTRL_PROTOCOL_VERSION` is not part of it, because the protocol version keeps its own check. A doc-comment edit does not change it. A new variant, a new field, a renamed tag, or a changed type does change it.
- **The hello carries the wire identity.** `CtrlHello` gets an additive `wire_id` field with `#[serde(default)]`. The ctrl protocol version does not change.
- **The local attach gate compares version and wire identity, not build identity.** A local Client refuses the Owner when the application version is different or the wire identity is different. An Owner whose hello has no wire identity (an Owner from before this change) is refused. A different build identity alone no longer refuses. The refusal still happens before the Client sends its hello or control credential (invariant 15). The restart-or-quit prompt does not change, except that it shows the application version, build identity, and wire identity of both sides.
- **Settings shows the version information of both sides.** The Session section of Settings gets two read-only rows in the TUI and in the GUI. One row shows this Client's application version, build identity, and wire identity. The other row shows the same three values as the connected Owner reported them in its hello. The rows are always shown. Activating them does nothing and saves nothing. This is the only place where a build-identity difference is visible. There is no toast, status text, or prompt for it.
- **The refusal error is renamed.** `RemotePlayerErrorKind::OwnerBuildMismatch` becomes `OwnerMismatch`, because the refusal no longer depends on the build.
- **BREAKING (one time):** the first attach of a Client from this change to a running Owner from before this change shows the restart prompt, because that Owner sends no wire identity.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `local-daemon-single-instance`: the build-gate requirement is replaced by a version-and-wire gate and a separate mismatch-prompt requirement, and a new requirement shows the version information of both sides in Settings.
- `ctrl-protocol`: a new requirement defines the wire identity and its place in the hello.

## Impact

- `crates/mbv-ctrl/`: a new `wire_identity` module (the digest), a `BuildInfo` type (application version, build identity, wire identity), and the `wire_id` field on `CtrlHello`. The committed hello schema and the hello fixtures under `protocol/ctrl/v10/` change.
- `crates/mbv-remote-player/`: `read_server_hello` compares the version and the wire identity, the handshake keeps the Owner's `BuildInfo`, `RemotePlayer::owner_build()` returns it, and the error variant is renamed.
- `crates/mbv-ui-model/`: two new `SettingKey` rows in the Session section and a new read-only `SettingValueKind::Info`.
- `src/app/`: the Settings rows get their values from `App`, and activating an `Info` row is a no-op. `src/owner_restart.rs` keeps working through the renamed error.
- `crates/mbv-gui/`: the Settings frame carries the two rows with the new kind, and the Svelte section shows them read-only. This part depends on `add-gui-settings-screen`, which must be merged first.
- Docs: invariant 15, `CONTEXT.md` (*Build identity*, *Wire identity*), and `docs/architecture/gui-client.md` (the mismatch-state wording).
- No new dependency. The digest is a small FNV-1a function in `mbv-ctrl`.
