# Implementation Plan: Compatible Ctrl Protocol Peers

Issue: #215
Spec: `docs/superpowers/specs/2026-07-16-compatible-ctrl-protocol-peers.md`

## Overview

Relax ctrl hello validation so a protocol v3 client can directly control a
known-compatible protocol v2 daemon when required capabilities are present, while
preserving strict rejection for missing capabilities and unknown incompatible
versions. This supports the reported direction only: a v3 client controlling a
compatible v2 daemon. It does not make old v2 clients compatible with a v3
daemon that sends a v3 hello first. The compatibility work must cover the full
daemon hello plus peer-compatible client hello exchange in the supported
direction and avoid sending v3-only wire commands to v2 peers. Then make
direct-upgrade failure visible in the TUI instead of only logging before
attached-session fallback.

## Architecture Decisions

- Model protocol compatibility as an explicit profile, not as arithmetic such as
  "peer version <= local version". For this issue, the profiles are local v3 and
  known-compatible v2.
- Store or carry the negotiated compatibility profile after reading the daemon
  hello. That profile must control the client hello version and any command
  restrictions needed for v2 peers.
- When connected to a v2 peer, send a v2-compatible client hello. Accepting the
  daemon's v2 hello is not sufficient; an unchanged v2 daemon will reject a v3
  client hello.
- Keep capability validation separate from protocol-version compatibility so
  version skew never bypasses `queue-state`, `play-items-start-idx`, or
  `status-only`.
- Keep capability negotiation connection-time only. Do not introduce
  command-level capability dispatch in this issue.
- Treat `QueueAppend` as a v3-only wire command until proven otherwise. For v2
  peers, reject remote append visibly. Do not translate append synchronization
  to `ReplaceQueue`, because full queue replacement is not append-equivalent
  and can restart/reload playback.
- Keep attached-session fallback unless product review decides otherwise, but
  surface direct-upgrade failure through a final combined status message when an
  mbv direct endpoint was advertised and attempted, e.g. `Direct mbv control
  failed: {reason}; using attached session {name}`.
- Do not change ctrl connection exclusivity, authority, `WireCommand` tags, or
  Local/Remote queue rendering. The queue UI should work through the existing
  `remote_player_tab` / `has_direct_remote_queue()` path once direct mode is
  reached.

## Dependency Graph

```text
Pre-edit GitNexus impact checks
    |
    v
Ctrl protocol compatibility profile
    |
    v
CtrlHello::validate_peer tests and implementation
    |
    v
RemotePlayer::connect_endpoint sends peer-compatible client hello
    |
    v
RemotePlayer/app command path rejects v3-only QueueAppend for v2 peers
    |
    v
App::connect_to_session enters direct remote mode when direct upgrade succeeds
    |
    v
App::connect_to_session shows visible status when direct upgrade fails
    |
    v
Docs and final regression verification
```

## Blast Radius

- `CtrlHello::validate_peer`: LOW risk in GitNexus impact analysis; no indexed
  upstream callers were reported, but it is called from the direct ctrl handshake
  path and covered by module tests.
- `App::connect_to_session`: LOW risk in GitNexus impact analysis; two direct
  callers, one affected input/mouse process. Scope changes tightly to direct
  upgrade failure handling.
- `RemotePlayer::connect_endpoint`: called by `App::connect_to_session`, tests,
  and startup direct endpoint code. Prefer testing through existing seams rather
  than changing this function unless necessary.

## Task List

### Phase 0: Pre-Edit Checks

- [ ] Task 0: Run mandatory GitNexus impact checks for every symbol that will
  be edited and update this plan if any risk is HIGH or CRITICAL.

### Checkpoint: Impact

- [ ] Blast radius is recorded for `CtrlHello::validate_peer`,
  `RemotePlayer::connect_endpoint`, and any app helper that will be changed.

### Phase 1: Ctrl Handshake Compatibility

- [ ] Task 1: Add explicit protocol compatibility policy.
- [ ] Task 2: Update ctrl hello validation tests for compatible v2 and rejected
  unknown versions.
- [ ] Task 3: Make `RemotePlayer::connect_endpoint` complete a fake v2 daemon
  handshake by sending a peer-compatible client hello.

### Checkpoint: Handshake

- [ ] `cargo test -p mbv-core ctrl`
- [ ] Focused `RemotePlayer::connect_endpoint` fake-daemon handshake test passes.
- [ ] Version-skew acceptance still requires all required capabilities.

### Phase 2: V2-Safe Queue Command Behavior

- [ ] Task 4: Implement v2-safe append behavior by rejecting remote append
  visibly.

### Checkpoint: Queue Commands

- [ ] A v2 peer does not receive `WireCommand::QueueAppend`.
- [ ] A v2 peer does not receive `WireCommand::ReplaceQueue` as an append
  substitute.
- [ ] Existing v3 peers still use the current efficient append path.

### Phase 3: Direct Upgrade User Feedback

- [ ] Task 5: Add a small test seam for direct-upgrade connection results if
  needed.
- [ ] Task 6: Surface direct-upgrade failure in `App::connect_to_session` with a
  final status that survives attached-session fallback.
- [ ] Task 7: Add app-level regression coverage for visible failure status.

### Checkpoint: App Behavior

- [ ] Focused app tests pass.
- [ ] Direct upgrade success path still returns before attached-session fallback.
- [ ] Direct upgrade failure path still has the reviewed fallback behavior.

### Phase 4: Documentation and Final Verification

- [ ] Task 8: Update domain docs only if implementation changes the capability
  negotiation model or user-visible control-state vocabulary.
- [ ] Task 9: Run final verification and GitNexus change detection.

### Checkpoint: Complete

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `detect_changes({scope: "compare", base_ref: "main"})` reports expected
  affected symbols and flows.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| One-sided validation still fails against an unchanged v2 daemon. | High | Add a fake v2 daemon handshake test that rejects v3 client hellos, then make `connect_endpoint` send a peer-compatible hello. |
| v2 and v3 are not actually compatible for `QueueAppend`. | High | Treat `QueueAppend` as v3-only; visibly reject the operation for v2 instead of translating to `ReplaceQueue`. |
| User-visible warning plus attached-session fallback could be overwritten by the fallback success message. | High | Emit one final combined status after fallback setup and assert the final `app.status` in tests. |
| App tests may need real network/remote setup to exercise `App::connect_to_session`. | Medium | Add a narrow test seam for direct connect results rather than depending on socket failures or timeouts. |
| Changing `connect_to_session` can affect mouse-driven session selection. | Medium | Keep changes scoped to the `Err(e)` branch after direct endpoint attempt; verify focused input/session tests. |
| Protocol compatibility policy could become stale when a future breaking version ships. | Medium | Centralize policy in a helper with tests that force an explicit compatibility decision for future changes. |

## Parallelization Opportunities

- Handshake validation and fake-daemon handshake tests are sequential because the
  client hello behavior depends on the compatibility profile.
- V2-safe append behavior can be explored after the compatibility profile exists
  but before app failure-status work.
- App failure-status tests can be drafted after the expected status wording and
  fallback policy are decided.
- Documentation review can run in parallel with app tests only after the
  implementation approach is stable.

## Open Questions

- Should direct-upgrade failure fall back to attached-session mode with a visible
  warning, or refuse fallback when an mbv direct endpoint was advertised?
- What exact status string should be shown on direct-upgrade failure?
- Should protocol v1 be named explicitly as incompatible, or is "only local v3
  and known-compatible v2 are accepted" enough?
- For v2 peers, remote append is rejected visibly until a true append-compatible
  capability exists.
