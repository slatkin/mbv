# Task Checklist: Compatible Ctrl Protocol Peers

Issue: #215

## Task 0: Run Pre-Edit Impact Checks

**Description:** Before editing any symbol, run the mandatory GitNexus impact
analysis for the symbols this plan expects to touch. Update the plan and warn
the user before implementation if any result is HIGH or CRITICAL risk.

**Acceptance criteria:**
- [ ] Impact is recorded for `CtrlHello::validate_peer`.
- [ ] Impact is recorded for `RemotePlayer::connect_endpoint` if it will change.
- [ ] Impact is recorded for `App::connect_to_session` or any extracted helper
  if app session connection behavior will change.

**Verification:**
- [ ] `impact({target: "validate_peer", file_path: "crates/mbv-core/src/ctrl.rs", direction: "upstream"})`
- [ ] `impact({target: "connect_endpoint", file_path: "crates/mbv-core/src/remote_player.rs", direction: "upstream"})`
- [ ] `impact({target: "connect_to_session", file_path: "src/app/mod.rs", direction: "upstream"})`

**Dependencies:** None

**Files likely touched:**
- None

**Estimated scope:** XS: read-only

## Task 1: Add Protocol Compatibility Profile

**Description:** Replace the exact-version-only peer check with a small explicit
compatibility profile in `crates/mbv-core/src/ctrl.rs`.

**Acceptance criteria:**
- [ ] Local protocol version remains accepted.
- [ ] Protocol v2 is accepted as known-compatible with local protocol v3.
- [ ] Unknown protocol versions are rejected with an actionable error.
- [ ] The compatibility profile records whether v3-only queue append is
  supported.

**Verification:**
- [ ] `cargo test -p mbv-core ctrl`

**Dependencies:** Task 0

**Files likely touched:**
- `crates/mbv-core/src/ctrl.rs`

**Estimated scope:** Small: 1 file

## Task 2: Preserve Capability Rejection Under Version Skew

**Description:** Add/adjust ctrl hello tests so compatible protocol skew still
requires every required capability.

**Acceptance criteria:**
- [ ] A protocol v2 hello with `queue-state`, `play-items-start-idx`, and
  `status-only` passes validation.
- [ ] A protocol v2 hello missing any required capability fails validation.
- [ ] The missing-capability error remains distinguishable from a version error.

**Verification:**
- [ ] `cargo test -p mbv-core ctrl`

**Dependencies:** Task 1

**Files likely touched:**
- `crates/mbv-core/src/ctrl.rs`

**Estimated scope:** Small: 1 file

## Task 3: Send Peer-Compatible Client Hello To V2 Daemon

**Description:** Update `RemotePlayer::connect_endpoint` so after a v3 client
accepts a compatible v2 daemon hello it sends a v2-compatible client hello
rather than always sending the local protocol version. This covers the reported
new-client-to-older-daemon direction only.

**Acceptance criteria:**
- [ ] A fake v2 daemon that rejects v3 client hellos can complete the handshake
  with the v3 client.
- [ ] Same-version v3 daemon handshakes still send the current v3 client hello.
- [ ] The negotiated compatibility profile is retained where outbound command
  serialization needs it.

**Verification:**
- [ ] Focused `RemotePlayer::connect_endpoint` fake-daemon test.
- [ ] `cargo test -p mbv-core remote_player`

**Dependencies:** Tasks 1-2

**Files likely touched:**
- `crates/mbv-core/src/remote_player.rs`
- `crates/mbv-core/src/ctrl.rs`

**Estimated scope:** Medium: 2 files

## Task 4: Make V2 Remote Append Safe

**Description:** Ensure a direct remote connected through the v2 compatibility
profile never receives the v3-only `QueueAppend` wire command or a
non-append-equivalent `ReplaceQueue` substitute.

**Acceptance criteria:**
- [ ] Existing v3 peers still use the current `QueueAppend` path.
- [ ] V2 peers reject the remote append action visibly.
- [ ] Tests prove a v2 peer does not receive `WireCommand::QueueAppend`.
- [ ] Tests prove a v2 peer does not receive `WireCommand::ReplaceQueue` for
  append.

**Verification:**
- [ ] Focused command-serialization or app queue-sync test for v2.
- [ ] Existing direct remote queue edit tests still pass.

**Dependencies:** Task 3

**Files likely touched:**
- `crates/mbv-core/src/remote_player.rs`
- `src/app/mod.rs` or `src/app/actions.rs` if translation/rejection happens at
  the app queue-sync seam

**Estimated scope:** Medium: 2-3 files

## Task 5: Add Direct Upgrade Test Seam If Needed

**Description:** Add the smallest seam needed to test direct-upgrade success and
failure handling without real mpv, a live Emby server, or brittle socket
timeouts.

**Acceptance criteria:**
- [ ] Production `connect_to_session` behavior remains unchanged except through
  the seam.
- [ ] Tests can inject a direct connection failure string.
- [ ] Tests can preserve the existing success-path coverage through
  `switch_to_direct_remote` or an equivalent helper.

**Verification:**
- [ ] Focused app unit test compiles without real network setup.

**Dependencies:** None, but do after Task 0 impact checks

**Files likely touched:**
- `src/app/mod.rs`

**Estimated scope:** Small: 1 file

## Task 6: Surface Direct Upgrade Failure

**Description:** Update `App::connect_to_session` so a failed direct daemon
upgrade leaves a user-visible final status instead of only logging or being
overwritten by attached-session fallback success.

**Acceptance criteria:**
- [ ] When an mbv direct endpoint is attempted and fails, the app sets a visible
  final status containing the direct-control failure reason.
- [ ] Successful direct upgrades still call `switch_to_direct_remote` and return
  before attached-session fallback.
- [ ] Attached-session fallback behavior matches the reviewed product decision.
- [ ] If fallback proceeds, the final status is a combined message such as
  `Direct mbv control failed: {reason}; using attached session {name}`, not
  `Connected to {name}`.

**Verification:**
- [ ] Focused app test for direct-upgrade failure status.
- [ ] Existing direct remote/session tests still pass.

**Dependencies:** Task 5 and product decision on fallback policy/status wording

**Files likely touched:**
- `src/app/mod.rs`

**Estimated scope:** Small: 1 file

## Task 7: Add App Regression Coverage

**Description:** Add tests around the session connection seam so direct-upgrade
failure remains visible and direct remote mode remains the success path.

**Acceptance criteria:**
- [ ] Failure path test asserts the final `app.status` after attached-session
  fallback setup.
- [ ] Existing success-path tests continue proving direct remote mode creates the
  remote queue conditions that render Local/Remote scopes.
- [ ] Tests do not require real mpv, real network, or a live Emby server.

**Verification:**
- [ ] `cargo test --lib direct_remote session_direct_endpoint status_bar`

**Dependencies:** Tasks 5-6

**Files likely touched:**
- `src/app/mod.rs`

**Estimated scope:** Small: 1 file

## Task 8: Update Domain Documentation If Needed

**Description:** Review `CONTEXT.md` and ADRs after implementation. Update only
if the implementation changes domain vocabulary, compatibility policy, or the
capability negotiation model.

**Acceptance criteria:**
- [ ] No stale claim remains that ctrl negotiation rejects solely on exact
  protocol version.
- [ ] Capability negotiation remains described accurately as connection-time
  validation unless implementation changes that.
- [ ] ADR 0003 and ADR 0007 remain consistent with the implementation.

**Verification:**
- [ ] Manual doc review.

**Dependencies:** Tasks 1-7

**Files likely touched:**
- `CONTEXT.md` if needed
- `docs/adr/` only if needed

**Estimated scope:** Small: 0-2 files

## Task 9: Final Verification

**Description:** Run the full repo verification path and GitNexus change
detection before handoff or commit.

**Acceptance criteria:**
- [ ] Formatting, linting, and tests pass.
- [ ] GitNexus reports only expected changed symbols/flows.
- [ ] Spec and plan remain aligned with final implementation choices, including
  handshake profile, v2 append behavior, and fallback status policy.

**Verification:**
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `detect_changes({scope: "compare", base_ref: "main"})`

**Dependencies:** Tasks 1-8

**Files likely touched:**
- No new source files expected

**Estimated scope:** Small: verification only
