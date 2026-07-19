# Library Route Production Logging Specification

**Status:** Adversarially reviewed, ready for maintainer review

**Tracks:** #270

**Design owner:** #256 endpoint-cached library routing, as implemented by PR #262 and documented by ADR 0011's #256 addendum

## Objective

Make every library-route and auto-reconnect outcome explainable from the normal persistent production log, `mbv.log`, without a debug build or temporary instrumentation.

This work does not claim to fix or identify the current routing failure. No root cause has been established. It creates the evidence needed to locate that failure and to diagnose future failures without repeating source-level archaeology.

An operator reading one attempt's logs must be able to answer:

1. What initiated route evaluation?
2. Which library and configured endpoint, if any, were selected?
3. Which decisions prevented or advanced routing?
4. Was a remote connection attempted, and what was its outcome?
5. Did mbv deliberately fall back to local playback, and was that degradation shown to the user?
6. What reconnect state was read or written, and did persistence succeed?

## Intended Behavior

#256's endpoint-cached design is authoritative:

- F2 discovers live devices and displays friendly names while the user chooses a target.
- Committing the selection stores only a resolved `tcp://host:port` value in `[library_routes]`.
- Play and enqueue route resolution are pure config reads. They do not query `/Sessions`.
- A stale or unreachable cached endpoint is not rediscovered automatically.
- A failed route connection falls back to local playback under #222's existing policy.

The merged PR #262 implementation and ADR 0011's #256 addendum supersede #256's original issue-body proposal to persist device names and rediscover failed endpoints.

Logging must describe this behavior. It must not reintroduce device-name persistence, playback-time discovery, endpoint refresh, retries, or self-healing.

## Scope

The implementation covers these lifecycles:

- application startup and effective route-table load;
- F2 library and device discovery, endpoint derivation, route commit, config save, and runtime-state update;
- play and enqueue route resolution from active-library and ancestor paths;
- ancestor-cache decisions that affect route resolution;
- remote connection attempt, success, failure, and local fallback;
- route-to-route, local-to-route, and route-to-local state transitions, including no-ops;
- auto-reconnect gating, persisted-state loading, route resolution, connection, and fallback;
- shutdown reconnect-state save, clear, atomic rename, and failure handling.

The implementation also corrects `CONTEXT.md` and any user-facing config guidance that still describes device-name route values. One reliability correction is explicitly in scope: a failed F2 config save leaves both persisted and runtime route tables unchanged, keeps the editor open, and shows a user-visible warning.

## Non-Goals

- Finding or fixing the current routing root cause in the same change.
- Changing fallback policy or adding connection retries.
- Adding metrics, distributed tracing, a new logging dependency, JSON output, or a remote telemetry service.
- Logging every media item, queue member, session payload, config value, or UI render.
- Logging credentials, API keys, auth headers, full Emby responses, or filesystem contents.

## Production Logging Contract

All events in this specification are emitted at normal production levels through the existing `log` facade and persistent `mbv.log` sink. None may be `debug`-only.

Use the existing `library_route` target for F2, resolution, connection, transition, and fallback events. Use `auto_reconnect` for startup reconnect events and `connection_state` for shutdown persistence events. Each line starts with a stable event name followed by machine-searchable `key=value` fields:

```text
route_connect_failed run_id="019f..." route_operation_id=42 route_connect_attempt_id=1 trigger=play library="music" endpoint="tcp://192.168.0.104:47788" error_kind=connection_refused error="connection refused" fallback=local user_warning=true
```

Required conventions:

- `info`: expected lifecycle decisions and successful state changes.
- `warn`: malformed configuration, swallowed external failures, configured-route connection failures, and persistence failures that mbv tolerates. A successful fallback completion is `info` with `degraded=true` because its preceding connection failure already carries the warning.
- `error`: an invariant is broken or mbv cannot establish the state it reports. Ordinary endpoint unreachability remains `warn`.
- Fields use stable names and a fixed vocabulary. Free-form error text is supplementary, not the only explanation.
- Endpoint strings may be logged because they are already user configuration and are required to diagnose routing. Session IDs, item IDs, paths, and friendly device names are logged only where needed to identify a user-selected candidate. Secrets and complete payloads are never logged.
- Route-table startup logging records counts plus rejected-entry detail. It does not dump accepted entries or the full config.

### Correlation

Logger initialization creates a mandatory `run_id` that is attached to every event in this specification. It must be unique across launches; a UUID or equivalent random identifier is sufficient.

Every user-initiated play/enqueue route evaluation receives a monotonically increasing in-process `route_operation_id`. A startup reconnect evaluation receives `auto_reconnect_operation_id`; if it proceeds past gating into route resolution, it creates a child `route_operation_id`. All resolution and application events carry the route operation ID. An actual network connection additionally receives `route_connect_attempt_id`; an operation that never connects does not.

F2 popup sessions and shutdown persistence use `route_edit_id` and `connection_state_operation_id`. A popup session may perform multiple commits, so each commit additionally receives `route_commit_id`. These IDs need only be unique within their `run_id`.

## Required Events

The event names below are normative. Implementations may add fields, but may not replace a required event with unstructured prose.

### Startup Route Configuration

`route_config_loaded` at `info`:

- `configured_count`
- `valid_count`
- `invalid_count`

`route_config_entry_rejected` at `warn`, once per invalid entry:

- `library`
- `raw_value`
- `reason` from `missing_scheme`, `malformed_endpoint`, or `non_tcp_endpoint`
- `accepted_shape="tcp://host:port"`

Startup validation reports but retains raw entries in the existing route table; it does not create a second validated map or mutate config. Resolution must return a typed classification equivalent to `selected`, `missing`, or `invalid`, so invalid data cannot collapse into `None`. Invalid-entry detail is logged once at load or change and again only if an operation actually selects that library. Accepted entries are covered by the summary and are logged when selected, avoiding a per-entry startup dump.

### F2 Route Editing

`route_editor_opened` at `info`: `route_edit_id`, `existing_route_count`.

`route_editor_libraries_loaded` at `info`: `route_edit_id`, `library_count`, `result` from `nonempty` or `empty`.

`route_editor_libraries_failed` at `warn`: `route_edit_id`, `operation=get_views`, `error_kind`, `error`, `result=empty_list`. This replaces the silent `get_views().unwrap_or_default()` outcome.

`route_editor_sessions_loaded` at `info`: `route_edit_id`, `session_count`, `candidate_count`, `unroutable_count`, `result` from `nonempty` or `empty`.

`route_editor_sessions_failed` at `warn`: `route_edit_id`, `operation=get_sessions`, `error_kind`, `error`, `result=empty_list`. This replaces the silent `fetch_sessions_blocking().unwrap_or_default()` outcome.

`route_session_candidates_summarized` at `info`: `route_edit_id`, candidate and rejection counts grouped by `missing_advertised_port`, `non_ipv4_host`, and `local_or_self_filtered`. Do not log every rejected candidate or unrelated non-mbv session individually.

`route_device_name_collision` at `warn` when same-name sessions resolve to different endpoints: `route_edit_id`, `device`, `endpoint_count`, `selection_policy`. Deduplication must be deterministic and observable.

`route_current_assignment_rejected` at `warn` when the configured value cannot be parsed for F2 preselection: `route_edit_id`, `library`, `raw_value`, `reason`, `accepted_shape`.

`route_selection_refused` at `warn` when the user tries to commit an unroutable candidate: `route_edit_id`, `device`, `reason`. Invalid cursor or vanished popup state emits the same event with `reason=invalid_selection_state`.

`route_commit_started` at `info`: `route_edit_id`, `route_commit_id`, `library`, `device`, `endpoint` or `action=clear`.

`route_commit_completed` is the one terminal persistence outcome for every started commit. At `info`, it has `outcome=saved`, `library`, `action`, and `effective_route_count`. At `warn`, it has `outcome=failed`, `operation`, `error_kind`, `error`, `runtime_updated=false`, and `user_warning=true`.

`route_runtime_updated` at `info` follows a successful commit: `route_edit_id`, `route_commit_id`, `library`, `action`, `effective_route_count`.

`route_editor_refresh_failed` at `warn` covers the post-commit `/Views` refresh currently hidden by `unwrap_or_default()`: `route_edit_id`, `route_commit_id`, `error_kind`, `error`, `committed=true`. This does not retroactively mark a successful persisted commit as failed.

`route_edit_cancelled` at `info` is reserved for closing the popup without a pending commit: `route_edit_id`, `stage`, `pending_commit=false`. Navigating back from device selection to the library list is not cancellation and need not be logged.

The route must become effective in memory only after config persistence succeeds. Implementation must reorder the current operation or restore its previous runtime and config values on save failure. A failed save keeps the editor open. The logs must never claim both save failure and effective update for the same commit.

### Play and Enqueue Resolution

`route_operation_started` at `info`:

- `route_operation_id`
- `trigger` from `play`, `enqueue`, or `auto_reconnect`
- `context` from `active_library`, `item_ancestors`, `queue_route`, or `persisted_library_route`
- `active_route`, using `none` when local
- `configured_route_count`

Compound resolution may inspect active queue route and then ancestors. Intermediate choices are fields on one `route_resolution_completed` event, not separate terminal events. Ancestor lookup adds `cache_status=hit|miss|expired` and, only when a lookup occurs or fails, the opaque Emby `item_id`.

Exactly one `route_resolution_completed` follows each started operation:

- At `info`, `outcome=selected`, with `library`, `endpoint`, and `source` from `active_library`, `ancestor_cache`, `ancestor_lookup`, `queue_route`, or `persisted_state`.
- At `info`, `outcome=not_configured`, with `library` and `reason=library_missing`.
- At `info`, `outcome=skipped`, with `reason` from `route_table_empty`, `thin_client_owns_playback`, or `no_library_context`.
- At `warn`, `outcome=failed`, with `reason` from `ancestor_lookup_failed` or `invalid_config`, plus bounded `operation`, `error_kind`, and free-form `error` when applicable.

Thin-client bypass occurs before the current playback route helper and must be instrumented at that earlier ownership boundary. It still emits both the resolution completion with `outcome=skipped` and the operation terminal event below.

An explicitly configured library must never collapse into an unexplained `None`. Invalid configuration is a warning; a missing route for an unconfigured library is normal `info`. The resolver must expose typed distinctions equivalent to `selected`, `missing`, and `invalid`; exact Rust type names are not normative.

### Application, Connection, and Fallback

Exactly one `route_application_completed` event terminates every route operation, including operations that never connect.

For play and reconnect, outcomes are:

- `connected`: remote connection succeeded and route-owned mode is active.
- `already_active`: the selected route already owns playback; resolution still reports `selected`.
- `stayed_local`: a selected route failed while playback was already local.
- `restored_local`: a selected route failed while another remote/route mode was active and existing local restoration completed.
- `restore_failed`: local restoration reported or demonstrated failure. Do not emit this outcome unless implementation has a truthful readiness/error signal; otherwise `restore_local_mode` remains treated as infallible and this outcome is omitted.
- `thin_client_bypass`: another thin-client mode owns playback.
- `local_no_route`: no route was selected and playback remains or becomes local according to existing behavior.

For enqueue, which never initiates a daemon connection, outcomes are:

- `enqueue_accepted_matching_route`
- `enqueue_accepted_local`
- `enqueue_rejected_route_conflict`
- `thin_client_bypass`

Each application event includes `route_operation_id`, `outcome`, `previous_mode`, `resulting_mode`, and `degraded`. Rejection and failed-route outcomes are `warn`; successful/no-op outcomes are `info`.

An actual connection emits `route_connect_started` with `route_connect_attempt_id`, `route_operation_id`, `trigger`, `library`, `endpoint`, and `previous_mode`, followed by exactly one `route_connect_succeeded` at `info` or `route_connect_failed` at `warn`. Failure includes `operation=connect`, bounded `error_kind`, free-form `error`, intended fallback, and `user_warning`.

Do not duplicate a connection warning with a second fallback warning. The terminal application event is `info` with `degraded=true` after successful fallback, or `error` only when a measurable restoration failure occurs.

The user-visible warning for a configured endpoint connection failure remains required. A production log entry alone is not sufficient feedback for an explicit route that degraded to local playback.

### Auto-Reconnect

`auto_reconnect_evaluated` at `info` on every startup: `auto_reconnect_operation_id`, `enabled`, `launched_as_remote`, `state_file_status` from `present`, `missing`, `unreadable`, or `invalid`.

Every evaluation ends with one of:

- `auto_reconnect_skipped` at `info`: `auto_reconnect_operation_id`, `reason` from `disabled`, `launched_as_remote`, or `state_missing`.
- `auto_reconnect_state_rejected` at `warn`: `auto_reconnect_operation_id`, `operation`, `error_kind`, `error` where applicable, and corrupt-file cleanup outcome.
- the normal route resolution and connection events for a persisted library route;
- `auto_reconnect_direct_session_started` followed by a success or failure event for persisted direct-session state.

The state loader must expose typed distinctions equivalent to `missing`, `loaded`, `unreadable`, and `invalid`; `Option` is insufficient. For a persisted library route that no longer resolves, emit the normal resolution completion, `route_application_completed outcome=local_no_route degraded=true`, then `auto_reconnect_failed` at `warn` with `fallback=local`. Do not silently return.

Direct-session reconnect logging must distinguish `device_not_found`, `sessions_fetch_failed`, `endpoint_unavailable`, and `connect_failed`. This specification does not change direct-session reconnect behavior.

### Shutdown Persistence

`connection_state_persist_evaluated` at `info`: `connection_state_operation_id`, `auto_reconnect`, `launched_as_remote`, `active_mode`, `decision` from `save_library_route`, `save_direct_session`, `clear`, or `skip`.

For saves, emit `connection_state_write_started` at `info` with `state_kind` and the non-secret identifying field (`library` or `device`). Do not log serialized JSON.

Emit exactly one terminal event:

- `connection_state_write_succeeded` at `info`: `connection_state_operation_id`, `state_kind`.
- `connection_state_cleared` at `info`: `connection_state_operation_id`, `file_existed`.
- `connection_state_persist_skipped` at `info`: `connection_state_operation_id`, `reason`.
- `connection_state_persist_failed` at `warn`: `connection_state_operation_id`, `operation` from `serialize`, `create_dir`, `read_existing`, `parse_existing`, `write_temp`, `rename`, or `remove`, plus `error_kind` and `error`.

`NotFound` while clearing is success with `file_existed=false`. Filesystem and serialization helpers must return enough result information for the caller to log truthfully. Existing ignored create-directory, write, rename, remove, and corrupt-file cleanup failures are not acceptable.

## Noise and Tolerance Rules

- Log decisions once at their ownership boundary. Lower-level helpers return typed outcomes or errors rather than duplicating the same failure at every call layer.
- Do not emit per-frame, per-render, polling-loop, or repeated route-table logs.
- Candidate rejection is summarized once per F2 device-list construction; detailed rejection is logged only after the user selects an unroutable candidate.
- Cache status is a field on the operation's single resolution event, not a separate event stream.
- Expected absence is `info`; tolerated degradation is `warn`; broken invariants are `error`.
- Every `warn` describing tolerated degradation includes the fallback or resulting state.

## Documentation

Update `CONTEXT.md` and related config help to state that `[library_routes]` maps lowercased library names to cached `tcp://host:port` endpoints. Friendly device names exist only in F2 discovery UI. ADR 0011 remains the design record and should not be replaced.

## Testing Strategy

Automated tests verify event semantics at decision boundaries without treating tests as proof of product behavior:

- config validation classifies accepted TCP, malformed, legacy device-name, Unix, and local values;
- F2 library/session fetch errors remain distinguishable from successful empty results;
- route resolution produces one terminal classification for table-empty, missing-library, invalid-value, selected-route, ancestor failure, cache hit, and cache miss; already-active is an application outcome after successful selection;
- enqueue produces accepted-matching-route, accepted-local, rejected-conflict, and thin-client-bypass outcomes without initiating a connection;
- configured endpoint connection failure records local fallback and requires user-visible warning state;
- auto-reconnect classifies disabled, missing, invalid, unresolved-library, direct-session failure, and successful route paths;
- persistence propagates and classifies serialization, create-directory, read/parse, temporary-write, rename, remove, and corrupt-file cleanup failures.

Tests should assert stable event names and critical fields through a test log sink or structured outcome-to-log seam. They should not assert complete formatted log lines or timestamps.

## Runtime Verification

Unit tests are insufficient for completion. Capture and inspect `mbv.log` from these real flows:

1. Open F2, select a live remote mbv device, commit a route, confirm `config.toml` contains `tcp://host:port`, and verify the edit events through runtime update.
2. Start playback from the routed library with the endpoint reachable. Verify one correlated resolution and successful connection trail.
3. Repeat with the endpoint unreachable. Verify connection failure, visible warning, completed local fallback, and usable local playback.
4. Play from an unconfigured library. Verify a normal not-configured outcome without warning noise.
5. Quit while route-owned with `auto_reconnect = true`. Verify successful state persistence and inspect the state file.
6. Restart and verify the correlated auto-reconnect decision trail and connection outcome.
7. Induce one persistence failure in a disposable environment and verify the operation and error are visible in `mbv.log`.

For each flow, a reviewer must be able to reconstruct the outcome using only the log and user-visible state, without reading source code.

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Manual verification uses the normal application binary and `~/.local/state/mbv/mbv.log`; exact launch flags depend on the local Emby and daemon environment and must be recorded in the implementation PR's verification notes.

## Boundaries

- Always: preserve #256 endpoint-only route semantics; emit normal-production events; attach `run_id` and the relevant operation ID; propagate persistence errors; verify real log output.
- Ask first: change fallback policy, add retries or rediscovery, add a logging dependency, change the on-disk log format, or log additional user-identifying data.
- Never: claim the logging change fixes the unknown routing failure; hide a configured-route failure behind `debug`; log credentials or complete server payloads; treat unit tests as runtime proof.

## Success Criteria

- Every required event has a cross-launch-unique `run_id` and its lifecycle's operation ID.
- Every play/enqueue operation, and every persisted library-route reconnect that proceeds past startup gating, has one route start event, one terminal resolution event, and one terminal application event sharing a `route_operation_id`.
- Every selected play/reconnect route has a connection outcome or `already_active`; every enqueue has a non-connection acceptance/rejection outcome.
- Every configured-route connection failure identifies the endpoint, error, local fallback outcome, and whether the user warning was shown.
- F2 discovery failure cannot appear identical to a successful empty result.
- Every auto-reconnect startup explains why it connected, failed, or skipped.
- Every shutdown explains whether reconnect state was saved, cleared, skipped, or failed, including the failed filesystem operation.
- Documentation consistently describes endpoint-cached routes.
- The required runtime flows leave a complete, readable trail in the normal production `mbv.log`.

## Implementation Constraints Requiring Typed Outcomes

The current `Option`/`()` helper contracts erase distinctions this logging requires. Implementation must preserve typed outcomes at least until the ownership boundary logs them:

- route lookup: selected, missing, or invalid with bounded reason;
- direct endpoint derivation: accepted or rejected with bounded reason;
- reconnect-state load: missing, loaded, unreadable, or invalid, including cleanup result;
- config and reconnect-state persistence: success or failure with operation and error kind.

Exact Rust enums and module placement are intentionally unspecified. The change should be kept as narrow as possible and must not create a parallel routing model.
