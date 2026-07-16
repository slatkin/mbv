# Spec: Compatible Ctrl Protocol Peers

Issue: #215

## Assumptions

- This is a narrow ctrl-handshake and direct-remote-upgrade fix, not a redesign
  of the daemon/TUI control seam.
- Ctrl protocol versions 2 and 3 are compatible only through an explicit
  compatibility profile. This spec covers the reported direction only: a v3
  client directly controlling a compatible v2 daemon. It does not make old v2
  clients compatible with a v3 daemon that speaks first.
- Capability negotiation remains connection-time validation. This change does
  not add general per-command capability gating unless implementation evidence
  shows it is required.
- User-visible direct-upgrade failure can use the existing status/toast path;
  it does not need a new modal, settings surface, or retry UI.
- The existing exclusive ctrl connection and authority rules in ADR 0003 and
  ADR 0007 remain unchanged.

## Objective

Allow a local mbv client using ctrl protocol v3 to directly control a compatible
older mbv daemon using ctrl protocol v2 when the daemon advertises the required
queue-control capabilities.

Today `CtrlHello::validate_peer` rejects a peer solely because
`peer_protocol_version != local_protocol_version`. In the reported case that
made `App::connect_to_session` log a direct daemon upgrade failure, then fall
back to attached Emby session control. The user saw one attached-session queue
instead of the expected Local/Remote queue scopes.

Success means protocol version skew only blocks direct control when the version
pair is known incompatible, the peer lacks required capabilities, or the client
cannot safely send commands under that peer's compatibility profile.
Compatible skew in that direction should complete the daemon hello plus
peer-compatible client hello exchange, enter direct remote mode, create
`remote_player_tab`, and let the existing Local/Remote queue-scope UI render.

## Tech Stack

- Rust 2021 workspace.
- `mbv-core` owns ctrl protocol types and handshake validation.
- `mbv` TUI owns session upgrade, direct remote mode, status/toast display, and
  Local/Remote queue rendering.
- Tests are ordinary Rust unit tests in the affected modules.

## Commands

- Build: `cargo build --workspace`
- Format check: `cargo fmt --all -- --check`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Focused ctrl tests: `cargo test -p mbv-core ctrl`
- Focused app/session tests: `cargo test --lib direct_remote session_direct_endpoint status_bar`
- Full test suite: `cargo test`
- Release, if requested later: `scripts/release.sh X.Y.Z "one-line summary"`

## Project Structure

- `crates/mbv-core/src/ctrl.rs` -> ctrl protocol version, capabilities,
  `CtrlHello`, wire command adapter, and protocol regression tests.
- `crates/mbv-core/src/remote_player.rs` -> direct ctrl connection setup that
  validates the daemon hello, sends the client hello, stores negotiated peer
  compatibility details, and serializes outbound commands.
- `src/app/mod.rs` -> Emby session connection, direct daemon upgrade,
  attached-session fallback, direct remote mode, status messages, and related
  app tests.
- `src/app/render/playlist.rs` and `src/app/render/power/queue.rs` ->
  Local/Remote queue-scope rendering that should work unchanged once direct
  remote mode is reached.
- `CONTEXT.md` and `docs/adr/` -> domain vocabulary and durable control-seam
  decisions. Update only if implementation changes the domain model or a
  settled decision.

## Code Style

Keep compatibility policy explicit and small. Prefer a named compatibility
profile over embedding version arithmetic inside capability validation.

```rust
impl CtrlHello {
    pub fn validate_peer(&self) -> Result<(), String> {
        CtrlCompatibility::for_peer(self.protocol_version)?;
        validate_required_capabilities(&self.capabilities)?;
        Ok(())
    }
}

struct CtrlCompatibility {
    peer_protocol_version: u32,
    supports_queue_append: bool,
}

impl CtrlCompatibility {
    fn for_peer(peer: u32) -> Result<Self, String> {
        match peer {
            CTRL_PROTOCOL_VERSION => Ok(Self {
                peer_protocol_version: peer,
                supports_queue_append: true,
            }),
            2 => Ok(Self {
                peer_protocol_version: peer,
                supports_queue_append: false,
            }),
            _ => Err(format!(
                "incompatible daemon protocol version: peer={peer} local={CTRL_PROTOCOL_VERSION}"
            )),
        }
    }
}
```

The exact implementation does not need to match this snippet, but it should keep
the compatibility rule centralized, testable, and easy to extend when a future
protocol version is actually breaking.

## Testing Strategy

- Add `mbv-core` tests proving `CtrlHello::validate_peer` accepts protocol v2
  when all required capabilities are present.
- Add a `RemotePlayer::connect_endpoint` test with a fake v2 daemon that rejects
  a v3 client hello. The v3 client must complete the handshake by sending a
  peer-compatible client hello, then read initial state successfully.
- Keep or replace the current incompatible-version test so a truly incompatible
  peer still fails with a clear version error.
- Keep the missing-capability test, including for older peers, so version
  compatibility never bypasses capability checks.
- Add coverage for a v2 peer receiving an append-equivalent operation. The
  client must not send a v3-only `QueueAppend` wire command to a v2 peer. A
  full `ReplaceQueue` is not append-equivalent because it can restart/reload
  playback, so v2 remote append should be rejected visibly.
- Add app-level regression coverage at the existing direct-upgrade seam if it
  can be tested without real mpv or a live Emby server:
  - compatible older hello/remote setup reaches direct remote mode
  - failed direct upgrade produces a final user-visible status rather than only
    a log or a status that is immediately overwritten by fallback success
  - attached-session fallback does not hide a failed mbv direct upgrade silently
- Prefer focused tests for handshake and mode transition over broad render
  snapshots. Existing render tests already prove Local/Remote scope pills appear
  whenever `has_direct_remote_queue()` is true.

## Boundaries

- Always: preserve required capability checks for `queue-state`,
  `play-items-start-idx`, and `status-only`.
- Always: keep ctrl connection exclusivity and ctrl-vs-Emby authority behavior
  consistent with ADR 0003 and ADR 0007.
- Always: make direct-upgrade failure visible to the user if direct upgrade was
  attempted and rejected.
- Always: complete the new-client-to-older-daemon hello compatibility story.
  Accepting the daemon hello is insufficient if the client hello remains
  incompatible with that daemon.
- Always: do not send v3-only wire commands, including `QueueAppend`, to a v2
  peer unless that peer explicitly advertises support through a compatibility
  profile or capability. Do not translate append to `ReplaceQueue`.
- Always: run GitNexus impact analysis before editing affected symbols during
  implementation.
- Ask first: declaring any protocol version range incompatible beyond the
  currently known v2/v3 compatibility.
- Ask first: adding a dependency, changing the wire representation, or changing
  the daemon authority model.
- Ask first: replacing attached-session fallback entirely instead of surfacing
  the failed direct-upgrade status.
- Never: accept a peer that lacks required capabilities.
- Never: infer broad command-level support from protocol version alone. A
  version-specific compatibility profile may encode known wire differences only
  when tests prove the behavior is safe.
- Never: change `WireCommand` serde tags as an incidental part of this work.

## Success Criteria

- A protocol v3 client accepts a protocol v2 daemon when the daemon advertises
  `queue-state`, `play-items-start-idx`, and `status-only`.
- The same protocol v3 client sends a v2-compatible client hello when connected
  to a v2 daemon, so an unchanged v2 daemon can complete the handshake.
- The client does not send v3-only `QueueAppend` to a v2 daemon; remote queue
  append behavior is rejected visibly instead of being translated to
  `ReplaceQueue`.
- The same connection path enters direct remote mode, so `remote_player_tab`
  exists, `has_direct_remote_queue()` can become true, and Local/Remote queue
  scopes render as they do for same-version direct control.
- Missing required capabilities still reject direct control with an actionable
  error.
- Unknown or known-incompatible protocol versions still reject direct control
  with an actionable error.
- When direct upgrade is attempted but cannot proceed, the UI surfaces that
  status instead of only logging and silently presenting a normal attached
  session.
- Regression tests cover new-v3-client-to-v2-daemon version-skew compatibility,
  missing-capability rejection, incompatible-version rejection, v2-safe append
  behavior, and the user-visible failure status.

## Open Questions

- Should protocol version 1 be explicitly known-incompatible, or should the
  implementation reject every version except the local version and the known
  compatible v2?
- What exact status wording should the UI show after a direct-upgrade failure?
  Candidate when fallback proceeds: `Direct mbv control failed: {reason}; using
  attached session {name}`.
- After a direct-upgrade failure, should attached-session fallback still proceed
  with a visible warning, or should mbv refuse the fallback for sessions that
  advertised an mbv direct endpoint?
- For v2 peers, remote append is rejected visibly until a true append-compatible
  capability exists.
