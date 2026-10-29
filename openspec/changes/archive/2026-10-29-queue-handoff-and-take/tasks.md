# Tasks

## 1. Domain language and decision record

- [x] 1.1 Update `CONTEXT.md` per design D9: add *Queue handoff* and *Take*, retire *Queue scope*, update *Direct remote control*, *Playhead*, *Resume source*, and the *hand-off* cross-reference. Verify with `rg -n -i "queue scope|scope pill|take of the control" CONTEXT.md` that only the retired-term entry remains.
- [x] 1.2 Add an ADR under `docs/adr/` that records "each Player owner keeps its own queue; Hand off and Take copy it; connect moves control only", notes how it updates ADR 0012, and supersedes the Queue-scope sentences of ADR 0017 (design D9). Verify the ADR links ADR 0012, ADR 0017, and this change.

## 2. ctrl start position (recipe: Add a ctrl protocol variant)

- [x] 2.1 Add `CTRL_CAP_QUEUE_REPLACE_START_POSITION` and the `#[serde(default)] start_position_secs: Option<f64>` field on `UnifiedQueueReplace` in `crates/mbv-ctrl`, with the `CtrlHello` advertisement and a `supports_*` accessor. Verify with a handshake case and a wire round-trip case in `crates/mbv-ctrl/tests/ctrl/` (contract: field is additive, absent field decodes as `None`), and `cargo nextest run -p mbv-ctrl`.
- [x] 2.2 Add the fixture under `protocol/ctrl/v10/` and regenerate schemas. Verify with `MBV_PROTOCOL_BLESS=1 cargo nextest run -p mbv-ctrl conformance`, review the diff, then `cargo nextest run -p mbv-ctrl conformance`.
- [x] 2.3 Advertise the capability from the Owner (`DaemonHelloCaps` in `crates/mbv-daemon/src/core_ctrl_spawn.rs`) and let `RemotePlayer` send the field only to owners that advertise it. Verify with `cargo nextest run -p mbv-remote-player -p mbvd` and one remote-player test (contract: field is omitted for an owner without the capability).

## 3. Owner start-position override

- [x] 3.1 Add `ResumeSource::Handoff(seconds)` in `crates/mbv-player/src/resume.rs` as a one-time override for the start slot of a replacement that carries a start position (design D4); the one-percent gate does not apply. Verify with one test in `mbv-player` (contract: Emby audio start slot starts at the carried position; the next slot uses its per-medium source).
- [x] 3.2 Pass the field into `handle_queue_replace` (`crates/mbv-daemon/src/control/queue_setup.rs`) and apply the override on its Cast, Emby Session, and local mpv branches (design D4). Verify with one `mbv-daemon` test (contract: a replace with `start_idx` and position starts that slot at the position, with no earlier audible slot) and `cargo nextest run -p mbv-daemon`.

## 4. Rename `QueueScope` to `QueueLink` (no behavior change)

- [x] 4.1 Rename `QueueScope { Local, Remote }` to `QueueLink { Local, Peer }` across `crates/mbv-ui-model`, `crates/mbv-ui-msg`, `crates/mbv-components`, and `src/app` (design D5). Verify with `cargo check --workspace` and an unchanged `cargo nextest run -p mbv` result.

## 5. Shown queue follows the connection

- [x] 5.1 Remove the user-selectable scope: the `queue_scope` field, `QueueRequest::Scope`, `set_queue_scope`, `display_peer_queue_on_connect`, the `visible_scope` half of `QueueScopeResolution`, and the `[`/`]` handler in `crates/mbv-components/src/queue/keys.rs`. The shown queue is `remote_view` when a peer link is current, else `local_view`. Delete the tests that only cover the removed scope switch. Verify with `cargo nextest run -p mbv -p mbv-components`.
- [x] 5.2 Update the edit and push paths so edits and active-index pushes address the shown link (`unified-playback-queue` and `queue-canonical-list` deltas). Extend the existing disconnect tests in `src/app/tests/tick_integration/disconnect.rs` to assert the local Owner queue is shown after disconnect and the peer link received no `Stop` (contract: disconnect leaves the peer playing). Verify with `cargo nextest run -p mbv`.
- [x] 5.3 Make the queue footer indicator always name the shown Player owner (design D7; `queue-playback-panel` delta). Verify with one render-layer test per the Tests matrix in `docs/architecture/tui-frontend.md` (contract: the label changes in the frame that first shows the peer queue).

## 6. F3 Hand off and Take requests

- [x] 6.1 Add `ShellRequest::HandOffSession(key)` and `ShellRequest::TakeSession(key)` in `crates/mbv-ui-msg`, the `h`/`t` keys in `crates/mbv-components/src/sessions.rs` gated per design D6 (`has_direct_endpoint`, false for this device's own row; `h` only while the local Owner plays; `t` only while the row shows the peer playing; both only when `home_is_local_daemon`), and the footer hints. Give each request an exhaustive dispatch arm. Verify with one component test (contract: each key emits the request with the stable key only when its D6 conditions hold) and `cargo nextest run -p mbv-components`.

## 7. Hand off

- [x] 7.1 Add the shell `PendingQueueTransfer` value and its tick-phase settlement (design D2): success, failure (rejection, missing start slot, link loss), and cancel by a newer F3 action. Verify with `cargo check -p mbv`.
- [x] 7.2 Implement Hand off (design D1 left column, D3 admission pre-check, `queue-handoff` confirm requirement through the existing replace-confirm path). When not connected, open the peer link to read its queue and attach only after confirmation; cancel drops the link. Verify with tests in `src/app/tests/` that mock both links: success stops local only after the peer observes the start slot; audio-only peer with an active video refuses and sends nothing; link loss before observation leaves local playing; an older peer is refused. `cargo nextest run -p mbv`.

## 8. Take

- [x] 8.1 Implement Take (design D1 right column) for both the connected and the unconnected case, reusing `PendingQueueTransfer`. Take while connected closes the F3 connection; Take never touches a library route (ADR 0012). Verify with tests that mock both links: success stops the peer only after local observes the start slot and the Client ends unconnected; a peer whose snapshot has no active slot (stale F3 row) receives nothing and the local queue is unchanged. `cargo nextest run -p mbv`.

## 9. Integration checks

- [x] 9.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. Verify all pass.
- [x] 9.2 Run `make check-code-file-lines` just before push and split any file over 800 lines. Verify it passes.

## Workflow follow-up

- Sync the deltas into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.
