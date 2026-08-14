## Context

See `proposal.md` and the delta specs. Packaged `mbvd` currently authenticates its daemon-owned cached Emby setup before entering `run_with_options`. That shared runner then creates an Emby WebSocket, Emby-backed Player state, capability registration, remote-command handling, and ctrl token validation from one mandatory `EmbyClient`.

The same-identity interactive-login workaround and legacy ctrl handshake are separate couplings. With `MBV_SYSTEM=1`, interactive `mbv` writes daemon-owned Emby state under system paths so `mbvd` can start. Separately, a remote `mbv` sends its own Emby bearer token in `CtrlHello`; packaged `mbvd` validates it against Emby and discards the returned user identity. Ctrl TCP is plaintext. The Local daemon already follows a different same-user Control-credential contract.

Existing core seams are useful but incomplete: `EmbyRuntime` models optional client readiness for the TUI, Player can update or clear Emby credentials in-process, and Emby setup/secret persistence already rolls back both files on partial commit. The daemon runner still needs an explicit role and optional Service context rather than inferring every policy from `MBV_SYSTEM` and an always-present client.

## Goals / Non-Goals

**Goals:**

- Give Local daemon and packaged `mbvd` one Service-independent daemon core with explicit role policy.
- Make packaged ctrl credential-free by design while ensuring an older client cannot leak its Emby token to a newer daemon.
- Reuse existing Service setup, secret isolation, generation, and Player credential-update seams.
- Reconcile owner-local administrative commits without transporting setup or credentials.
- Keep Emby lifecycle state internally coherent while unrelated queue and Feed behavior remains live.

**Non-Goals:**

- Provider-neutral shared-data identity, shared-data protocol changes, or daemon-settings migration.
- TLS, pairing, Control credentials, users, or permissions for packaged ctrl.
- Audiobookshelf setup or playback.
- `mbvd --disconnect emby`; removal can be specified separately if it becomes a product requirement.
- A generic dynamic plugin architecture for Services.

## Decisions

### 1. Pass an explicit daemon role and optional Emby owner context

Replace the runner's mandatory `EmbyClient` input with role-specific startup state containing common configuration plus optional owner-local Emby context. The role is explicit (`Local` or `Packaged`) and controls ctrl admission, listeners, lifecycle authority, and presentation integrations. It is not inferred from transport or paths inside shared code. This is a shared-code refactor, not a new Local-daemon behavior change: Local retains its existing startup and Control-credential contract; only Packaged receives the Service-independent startup guarantee in this change.

The common daemon core always constructs queue authority, Player, ctrl, Feed support, and shutdown handling. If owner-local Emby setup is usable, it additionally creates one Emby runtime holding the API client, WebSocket sender/receiver, capability registration state, and remote-command participation. Emby QueueItems are admitted only while owner context is installed.

Keeping a placeholder `EmbyClient` everywhere was rejected because it preserves ambient Emby assumptions and makes absence indistinguishable from an empty or failed client. Splitting packaged and Local daemon implementations was rejected because queue, Player, ctrl, and Service playback ownership must remain common.

### 2. Make optional Emby event sources explicit

Do not spawn an Emby WebSocket or capability worker when Emby context is absent. Represent optional event producers so the merged daemon loop receives `Ws` events only from a live Emby runtime. Gate keepalive, periodic capability refresh, Emby remote commands, item lookup, source preparation, and reporting on a generation-matched runtime snapshot.

Player retains its existing mutable optional Emby credentials. Reconciliation updates that context for future playback; an active same-server run retains its captured lifecycle. A different-server replacement first removes old-server items and settles any invalidated active Emby run before installing the replacement context.

Starting empty WebSocket reconnect loops and relying on failed HTTP calls was rejected because it keeps Emby operationally mandatory and creates misleading retries/logs.

### 3. Bump ctrl protocol before removing the bearer-token field

Deliver `CTRL_PROTOCOL_VERSION` 9 and remove `auth_token` and legacy Emby validation from the v9 hello contract. Both sides read the daemon hello first. A v9 client therefore rejects every older protocol before sending an Emby token, and a v9 daemon rejects every older protocol before accepting a credential-bearing hello.

Packaged role advertises no application-auth capability and accepts a compatible hello after transport admission. Local role continues advertising `control-auth` and validates the same-user Control credential. This is role policy, not a fallback chain.

An additive capability was rejected because old clients ignore unknown capabilities and then send `auth_token`. Retaining a deprecated ignored field was rejected because it leaves bearer material on plaintext ctrl. This is the documented exception to capability-only evolution: the in-code wire rule already requires a version bump for field removal and handshake/framing changes. The current source's v7 constant and the archived v8 contract are pre-existing drift; the delivered protocol is v9 and rejects either older peer. Giving packaged `mbvd` a distributed Control credential or pairing UX was rejected because the product intentionally trusts Unix permissions and LAN reachability.

### 4. Preserve transport- and role-scoped authority

Credential-free packaged ctrl means any peer reaching the listener can issue ordinary playback and queue commands. It does not make TCP equivalent to local administration. Existing lifecycle checks continue to reject coordinated shutdown and every future owner-administration command over TCP. The packaged Unix socket remains protected by host filesystem permissions.

TLS or application authorization was rejected for this change because packaged `mbvd` is deliberately a trusted-LAN media controller. Operators must not expose plaintext ctrl to an untrusted network.

### 5. Implement `mbvd --connect emby` as validate, commit, reconcile

Add structured CLI parsing for `mbvd --connect emby`. The command uses the system-instance paths and runtime identity already used by packaged `mbvd`, prompts locally, authenticates into a candidate `EmbyClient`, and derives persisted `EmbySetup` plus token. Username and password remain transient.

Use the existing practical transaction for setup and secret. `EmbySetup` gains a persisted `revision: u64`, initially `1` for a successful installation and incremented for every successful repair or replacement. It is distinct from the in-memory `service_runtime::SetupGeneration`: the persisted revision identifies one durable setup commit across processes, while the runtime generation invalidates stale asynchronous work after each in-process install. Both the command and daemon read the stored revision; no setup or secret hash is used on the wire.

Classify a candidate as same-server exactly by normalized `EmbySetup.server_url` equality, using the existing `EmbySetup::new` normalization (trim whitespace and a trailing slash). The current persisted model has no stable Emby server ID, so user ID changes alone do not make a server replacement. Same-server repair changes setup/secret and revision without clearing Service-owned state.

Before a different-server commit, take a restorable owner-state snapshot and invoke an explicit `clear_emby_owned_state` seam. It clears only Emby queue entries and persisted queue state, Emby library positions, library routes, and Emby caches, preserving Feeds and every other Service. Clear state first, commit the new setup/secret/revision second, and restore the complete snapshot if either action fails. Serialize this sequence with an owner-local administration lock so concurrent connect commands cannot interleave snapshots and commits.

Writing files separately was rejected because it can create mixed setup/token state. Asking administrators to run interactive `mbv` was rejected because it couples daemon administration to the TUI and its user identity.

### 6. Reconcile through a credential-free acknowledged local request

After commit, connect only to the packaged daemon's local Unix ctrl socket and send `CtrlCmd::ApplyServiceSetup { kind: ServiceKind::Emby, revision: u64 }`. The running owner rereads setup and secret from its own storage and compares the persisted `EmbySetup.revision` exactly to `revision`. It returns either `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }`, where `reason` is one of `UnsupportedService`, `RevisionMismatch`, `StorageUnavailable`, or `TransitionRejected`. No setup field, setup hash, or credential appears on the wire. TCP and Local-daemon ctrl reject this command.

If no packaged daemon is running, the command does not send the request: the durable commit is complete and next startup loads it. If a packaged daemon is running but the Unix request cannot connect or acknowledge, or it returns any rejection, retain the durable commit and report restart required. The command never claims live success from enqueueing alone.

For a same-server repair, the daemon installs a new in-process runtime generation for future Emby work while a currently active Emby run retains its captured lifecycle. For a different-server replacement, it first removes all old Emby Bound queue entries; if an old-server Emby item is active, it stops that Player run and performs its terminal lifecycle reporting within `EMBY_REPLACEMENT_FINALIZE_HARD_BOUND` (five seconds). Only after finalization succeeds does it install the replacement runtime. A failure or timeout returns `TransitionRejected`, leaves the replacement durable but not live, and requires restart; it never activates the new server against old-server state. Non-Emby active playback continues.

Unix process signals were rejected because they cannot acknowledge the durable revision actually applied. Reusing ordinary TCP ctrl was rejected because host administration is not a trusted-LAN playback command. Restarting automatically was rejected because it would interrupt unrelated playback and hide whether live adoption succeeded.

### 7. Leave shared data on its current independent path

Shared-data hosting remains optional and retains its current Emby-scoped authentication and storage behavior. Start it only when its existing prerequisites are present; otherwise disable or degrade that facility without affecting ctrl or playback. Do not route shared-data authentication through the new packaged ctrl rule.

Folding shared data into this migration was rejected because it is immature, separately optional, and has a per-user identity requirement that packaged playback ctrl intentionally does not have.

## Risks / Trade-offs

- **[Protocol bump requires coordinated client and daemon upgrades]** -> Fail before any older client can transmit its bearer token and provide a specific upgrade diagnostic.
- **[A trusted-LAN listener permits any reachable peer to control playback]** -> Document this as the intentional boundary, keep administrative/lifecycle requests local-only, and avoid implying encryption or identity that does not exist.
- **[Optionalizing Emby leaves hidden mandatory-client assumptions]** -> Introduce explicit runtime context first, then compile and test zero-Service startup across queue, Player, WebSocket, capabilities, ctrl, and shutdown paths.
- **[Different-server replacement can mix old IDs with a new server]** -> Validate first, perform provider-scoped cleanup transactionally, generation-gate reconciliation, and never activate on cleanup failure.
- **[Live reconciliation races active Emby playback]** -> Same-server repair affects future lifecycle calls through a generation transition; different-server replacement performs bounded finalization and removes invalidated items before context swap.
- **[Shared data may be unavailable without Emby]** -> Preserve its existing optional fallback and make startup/playback tests prove there is no reverse dependency.

## Migration Plan

1. Introduce explicit daemon role and optional Emby owner context while preserving current behavior behind adapters.
2. Move WebSocket, capability registration, Emby command handling, lookup, and reporting behind the optional runtime; prove zero-Service packaged startup.
3. Add v9 ctrl framing, remove the token field and validator path, and update client diagnostics in one coordinated change.
4. Add transactional `mbvd --connect emby`, provider-scoped replacement cleanup, administrative locking, and secret-redaction checks.
5. Add the acknowledged local reconciliation request and running-owner runtime transition.
6. Update packaged service/operator documentation to replace interactive identity login and state the trusted-LAN boundary.

Rollback requires rolling client and daemon binaries back together because v8 and v9 intentionally refuse one another. Existing validated Emby setup remains readable by the prior binary; no new credential format is introduced.
