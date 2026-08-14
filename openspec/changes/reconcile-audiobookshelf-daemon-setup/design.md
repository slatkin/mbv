## Context

See `proposal.md` and the delta specs. Bare-mode Audiobookshelf setup already validates an API key with `GET /api/me` and persists a per-Service mode-0600 secret; `AudiobookshelfSetup` today holds only `server_url`. Packaged `mbvd` already has the full Emby administration/reconciliation path (`mbvd --connect emby` → `ApplyServiceSetup { kind: Emby, revision }` → `reconcile_packaged_emby`), and `EmbyOwnerContext` is the packaged owner's loaded runtime context with a persisted `revision` and an in-memory `SetupGeneration`. `owner_admin_transport_allowed` currently permits owner administration only for `DaemonRole::Packaged` over `CtrlTransport::Local`. #525 landed the ctrl capability/progress seam; daemon Audiobookshelf admission is still disabled.

## Goals / Non-Goals

**Goals:**

- Mirror the Emby owner-context and administration pattern for Audiobookshelf across all three Player owners without duplicating the reconciliation boundary.
- Reuse the existing `ApplyServiceSetup` wire command as the single reread-own-storage signal for both packaged and Local-daemon receivers.
- Give packaged `mbvd` supported `--connect abs` / `--disconnect abs` administration that preserves working state on any failed candidate.
- Keep credentials, resolved URLs, and device identity out of ctrl and logs.

**Non-Goals:**

- Daemon Audiobookshelf admission, source preparation, stream resolution, progress generation, or stay-alive playback.
- Migrating other Player-owner settings or generalizing Emby administration beyond what Audiobookshelf needs.
- Any protocol-version bump or new capability string.

## Decisions

### 1. Add a persisted `revision` to `AudiobookshelfSetup`

Add `revision: u64` (serde default `1`) mirroring `EmbySetup.revision`. The lifecycle seams bump it once per successful initial setup, same-server repair, or different-server replacement. The in-memory `SetupGeneration` stays separate and advances per runtime install.

The reconciliation signal needs a persisted token the daemon can compare against its own storage; an in-memory generation cannot cross processes. A normalized-URL comparison alone was rejected because a same-server repair also changes the credential and must still trigger a reread.

### 2. Introduce an `AudiobookshelfOwnerContext` alongside `EmbyOwnerContext`

Add a context holding the loaded setup, API key (runtime-only), stable `device_id()`, in-memory `SetupGeneration`, and persisted `revision`. `DaemonStartupContext` gains an `audiobookshelf: Option<...>` field loaded the same Service-independent way as Emby (`from_packaged_storage_result` → no authentication, absent/incomplete setup produces `None`).

The device identity is the same stable non-secret `device_id()` bare mode already uses, so a later playback child needs no migration. Holding the API key in memory is required for that child; it never crosses ctrl.

### 3. Reuse `ApplyServiceSetup` as the single reconciliation signal

Extend the daemon-side handler to accept `ServiceKind::Audiobookshelf` and add a `reconcile_packaged_audiobookshelf` that rereads owner storage, compares revision, advances generation, and installs or drops the context. Widen `owner_admin_transport_allowed` to `transport == Some(CtrlTransport::Local)` so a `DaemonRole::Local` daemon accepts the signal from its own attached client while every TCP path and cross-owner path stays rejected.

A second command for the Local daemon was rejected: both owners perform the identical operation (reread own storage, compare revision, apply), and the role/transport gate already encodes the ownership boundary. One command keeps the "one persisted-source-of-truth boundary" literal.

### 4. Bare mode applies the operation directly

Bare mode is its own Player owner and already commits to the shared per-user storage it reads. It invokes the same semantic operation in-process (advance generation, install/drop context) with no ctrl round trip. When a same-user Local daemon is running, the bare client additionally sends `ApplyServiceSetup { kind: Audiobookshelf, revision }` over the Local daemon control socket so the daemon rereads the just-committed storage.

### 5. `mbvd --connect abs` mirrors `connect_emby`

Prompt locally for server URL and API key (API key via the existing hidden-password prompt), call `AudiobookshelfClient::validate_setup_bounded` (`GET /api/me`), then commit through the existing transactional seams: same-server/new → `persist_audiobookshelf_setup_and_secret`, different-server → `replace_audiobookshelf_setup_and_secret` with the Audiobookshelf-owned-state clear closure. Reuse `reconcile_running_owner` with `ServiceKind::Audiobookshelf`. Exit codes and interactive/usage checks match Emby connect.

### 6. `mbvd --disconnect abs` is a new no-confirm command

Delete setup + secret + Audiobookshelf-owned state through `remove_audiobookshelf_setup_and_secret_with_owned_state`, then reconcile a running daemon (which sees no stored setup and drops its context). No confirmation prompt: this is an explicit owner-local CLI action, not an in-TUI destructive edit. Failure semantics per the delta: durable removal is reported explicitly; a failed reconciliation reports restart required and that the running process may retain the deleted key in memory.

### 7. Admission stays disabled

No code path in this change touches the daemon Service-capability predicate or source preparation. The owner context is load-bearing only for later #524 children.

## Risks / Trade-offs

- **[Risk] Local daemon and bare mbv race on the shared config file** -> `ApplyServiceSetup` is revision-gated; the daemon applies only when the persisted revision matches, and stale signals return `RevisionMismatch`.
- **[Risk] Widen of `owner_admin_transport_allowed` accidentally admits TCP** -> Keep the gate `transport == Some(CtrlTransport::Local)` only, and cover TCP and cross-owner rejection in tests.
- **[Risk] The API key lives in daemon memory longer than before** -> It already lives in bare-mode memory; holding it is required for later playback, and it is never serialized.
- **[Risk] `AudiobookshelfSetup` gains a field, changing persisted TOML** -> The `revision` default keeps old configs valid; the lifecycle seam writes it on next commit.
- **[Trade-off] Local daemon accepts `ApplyServiceSetup` while the packaged rule once rejected Local-daemon ctrl** -> The rejection was packaged-scoped defense in depth; the ctrl-protocol delta restates the boundary as receiver-scoped.

## Migration Plan

1. Land `revision` on `AudiobookshelfSetup` with serde default; existing configs keep working.
2. Add `AudiobookshelfOwnerContext` and daemon startup loading (no behavior change while admission is off).
3. Extend the daemon `ApplyServiceSetup` handler and transport gate; add `reconcile_packaged_audiobookshelf`.
4. Add `mbvd --connect abs` and `mbvd --disconnect abs`, reusing the reconcile and lifecycle seams.
5. Wire bare-mode Services setup/repair/replacement/removal to signal a running same-user Local daemon.

Rollback removes the `mbvd` subcommands and the Local-daemon acceptance gate first; the persisted `revision` field and dormant owner context are harmless to keep while admission remains disabled.
