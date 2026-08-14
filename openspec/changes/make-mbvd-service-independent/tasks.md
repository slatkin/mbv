## 1. Daemon Role And Optional Service Context

- [x] 1.1 Add an explicit Local-versus-Packaged daemon role and a common startup context containing ordinary configuration plus optional owner-local Emby runtime state, preserving the existing Local-daemon startup and Control-credential behavior.
- [x] 1.2 Change daemon construction so queue authority, Player, ctrl listeners, Feed support, and shutdown initialize without an `EmbyClient` or Remote Service credential.
- [x] 1.3 Load packaged owner Emby setup and secret through current Service storage without authenticating it as a prerequisite for process startup.

## 2. Isolate Emby Runtime Behavior

- [x] 2.1 Create optional Emby runtime ownership for API lookup, source/reporting context, WebSocket sender/receiver, capability registration, and remote commands.
- [x] 2.2 Add a persisted `EmbySetup.revision: u64` for inter-process commit identity and preserve `service_runtime::SetupGeneration` as the separate in-memory stale-work guard; start and service Emby event sources, keepalives, and capability refresh only while their runtime generation is installed.
- [x] 2.3 Gate Emby QueueItem admission and asynchronous resolution on installed owner context while preserving queue, ctrl, and Feed behavior when absent or unavailable.
- [x] 2.4 Keep optional shared-data startup and authentication unchanged, and ensure its unavailable/Emby-absent path cannot disable daemon playback or ctrl.

## 3. Ctrl Version 9 And Packaged Trust Boundary

- [x] 3.1 Reconcile the pre-existing source v7 versus archived v8 protocol-contract drift, set the shipped `CTRL_PROTOCOL_VERSION` to 9, and remove the legacy `auth_token` field, client construction path, packaged validator call, and Emby-token compatibility diagnostic.
- [x] 3.2 Make v9 peers reject every prior protocol version immediately after the daemon hello so no client Service credential is serialized or transmitted.
- [x] 3.3 Apply role-specific hello policy: Local daemon continues advertising and validating its Control credential, while packaged `mbvd` accepts protocol-compatible Unix/TCP clients without application credentials.
- [x] 3.4 Replace the capability-gated legacy Emby-token migration contract, retain audio-only capability additions as additive within a protocol version, and document why v9 is required for the removed hello field and changed framing.
- [x] 3.5 Preserve transport- and role-scoped lifecycle restrictions so packaged TCP clients cannot invoke local shutdown or owner-administration commands.

## 4. Transactional mbvd Connect Emby

- [x] 4.1 Add exactly `mbvd --connect emby` parsing: require an interactive terminal, reject conflicting daemon/export/quit selectors and unsupported Services, provide specified exit codes, and accept no secrets through argv, environment, or files.
- [x] 4.2 Authenticate the candidate to obtain validated Emby setup identity and token while keeping username/password transient; emit classified diagnostics without passwords, tokens, usernames, user IDs, or raw remote response bodies.
- [x] 4.3 Serialize owner administration and commit candidate setup/secret through the existing rollback-safe Emby persistence transaction.
- [x] 4.4 Classify same server by normalized `EmbySetup.server_url`; increment the persisted revision on every successful commit; preserve state for repair and snapshot, clear, commit, and restore Emby-owned state transactionally for replacement.

## 5. Running-Owner Reconciliation

- [x] 5.1 Add packaged-Unix-only `CtrlCmd::ApplyServiceSetup { kind, revision }` and its applied/rejected response with the four specified rejection reasons; prohibit it on TCP and Local-daemon ctrl and never serialize setup, identity hash, or credentials.
- [x] 5.2 Make a running packaged daemon reread and revision-match committed owner storage, then install or replace its optional Emby API, Player, WebSocket, and capability context coherently.
- [x] 5.3 Preserve an active same-server Emby run and unrelated playback; for different-server replacement, purge every old Emby Bound item and stop/finalize an active old Emby run within the specified five-second bound before activating the replacement.
- [x] 5.4 Report specified exit outcomes: committed for next startup when stopped, committed and applied live, or exit `3` with restart required when acknowledgement is unavailable/rejected.

## 6. Verification And Operator Guidance

- [x] 6.1 Extend process-boundary tests to prove packaged `mbvd` starts and serves ctrl/Feed queues with zero Services and remains operational when configured Emby is unavailable.
- [x] 6.2 Add bounded deterministic socket-handshake tests proving v9 packaged ctrl is credential-free, v9 Local daemon still requires Control auth, prior-version mismatches stop before bearer-token transmission, zero-Service hello is service-neutral, and owner administration is Unix-packaged-only.
- [x] 6.3 Cover initial setup, exact CLI/TTY/exit-code behavior, diagnostic redaction, same-server repair, rejected/unreachable candidates, partial persistence failure, different-server state cleanup/rollback, concurrent administration, stopped-owner success, every reconciliation rejection variant, live acknowledgement, active old-Emby finalization, active Feed continuity, and restart-required fallback.
- [x] 6.4 Prove Emby absence disables only Emby WebSocket/capabilities/lookup/reporting and optional shared-data behavior remains unchanged and non-gating.
- [x] 6.5 Update `mbvd` usage, packaged service/operator documentation, and domain/agent guidance where stale: replace interactive identity login, explain `mbvd --connect emby`, and state trusted-LAN/Unix-permission ctrl access.
- [x] 6.6 Run targeted `mbvd`/`mbv-core` tests, `cargo check -p mbvd`, `cargo check -p mbv-core`, `cargo check -p mbv`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `make check-code-file-lines`, strict OpenSpec validation, and `git diff --check`.

## 7. Coordinated Delivery

- [x] 7.1 Keep ctrl tasks 3.1 through 5.4 in one coordinated client-and-daemon deployment: do not merge a state where one supported binary writes v9 and its counterpart still expects legacy hello semantics.
- [x] 7.2 Keep socket and process-boundary tests deterministic and hard-bounded; replace rather than troubleshoot a flaky test under the project test policy.
