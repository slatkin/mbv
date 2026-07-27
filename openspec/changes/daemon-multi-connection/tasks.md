## 1. ADR Supersession

- [x] 1.1 Write new ADR (0014) that supersedes ADR 0003 (exclusive ctrl connection) and documents the new multi-connection model
- [x] 1.2 Update ADR 0007 (control authority) to reflect: authority returns on next ctrl command (not on connect), connect does NOT override EmbyRemote, authority clears only on last client disconnect

## 2. Protocol Version

- [x] 2.1 Bump protocol version constant from 3 to 4 in `ctrl.rs`
- [x] 2.2 Update `CtrlCompatibility::for_peer()` to accept v4 only (reject v2 and v3)
- [x] 2.3 Update `CtrlHello` validation to require v4

## 3. Connection Registry

- [x] 3.1 Change `CtrlClients.connection` from `Option<CtrlClient>` to `Vec<CtrlClient>`
- [x] 3.2 Update `CtrlClients::connect()` to append new client, remove eviction logic
- [x] 3.3 Remove `Disconnected { reason: TakenOverByCtrlClient }` send from `connect()`
- [x] 3.4 Update `CtrlClients::connect()` to NOT set authority if current authority is `EmbyRemote`
- [x] 3.5 Update `CtrlClients::remove()` to remove client by ID from vec, set authority to `None` only if vec is empty
- [x] 3.6 Update `has_driver()` to check `!connection.is_empty()`
- [x] 3.7 Update `has_client(id)` to search vec for matching ID

## 4. Authority Model

- [x] 4.1 Change `AuthorityHolder::Ctrl(CtrlClientId)` to `AuthorityHolder::Ctrl` (no ID)
- [x] 4.2 Update all call sites that match on `AuthorityHolder::Ctrl(id)` to use `AuthorityHolder::Ctrl` without ID
- [x] 4.3 Replace `take_authority_for_emby_remote()` implementation: broadcast `Disconnected { TakenOverByEmbyRemote }` to all clients, do NOT send `CtrlOutbound::Close`, do NOT disconnect
- [x] 4.4 Remove or repurpose `disconnect_driver()` — it becomes dead code once 4.3 is done
- [x] 4.5 Remove or repurpose `disconnect()` — it becomes dead code once 4.4 is done
- [x] 4.6 Update `send_to_driver()` / rename to `broadcast_to_all()` — send to all clients in vec, remove failed clients on send failure
- [x] 4.7 Update `remove()` authority logic: set authority to `None` only when last client disconnects and authority is `Ctrl`

## 5. Broadcast Fan-Out

- [x] 5.1 Update `broadcast()` helper to use new fan-out logic via `broadcast_to_all()`
- [x] 5.2 Update periodic status broadcast thread to fan out to all clients
- [x] 5.3 Handle send failures per-client: remove failed client from vec, continue broadcasting to others

## 6. Command Rejection

- [x] 6.1 ~~Add authority check in ctrl command handler: reject if authority is `EmbyRemote`~~ — replaced by 6.3 (authority flip, no rejection)
- [x] 6.2 ~~Send `CtrlEvent::CommandRejected` with reason "Emby remote has authority" when rejected~~ — not needed; authority flips on next ctrl command
- [x] 6.3 Implement authority return: set authority to `Ctrl` when ctrl command arrives and authority is `EmbyRemote`
- [x] 6.4 Update `AdoptQueue` handler comment referencing ADR 0003

## 7. Disconnected Event Semantics

- [x] 7.1 Remove `DisconnectReason::TakenOverByCtrlClient` variant from `ctrl.rs`
- [x] 7.2 Ensure `Disconnected { reason: TakenOverByEmbyRemote }` does NOT send `CtrlOutbound::Close` after it

## 8. Spectrum State

- [x] 8.1 Review spectrum stop logic at `daemon_run.rs:377-382` — document phase 1 behavior (stop on any disconnect) in comments
- [x] 8.2 Add TODO comment for phase 2 per-client spectrum tracking

## 9. TUI Client: Protocol and State Machine Rework

- [x] 9.1 Update TUI client protocol version to 4
- [x] 9.2 Update `remote_player.rs` reader thread to handle `Disconnected { TakenOverByEmbyRemote }` as authority notification, NOT connection close — do NOT call `restore_local_mode()`
- [x] 9.3 Update `apply_ctrl_event` to distinguish authority-change `Disconnected` from connection-ending events
- [x] 9.4 Update `player_event.rs` to NOT call `restore_local_mode()` on authority-change notifications
- [x] 9.5 Handle `CommandRejected` events with authority reason — display message to user
- [x] 9.6 Rework `connected_session_id` / `direct_remote_connected` state machine to reflect multi-connection model (no eviction, no takeover tracking)
- [x] 9.7 Remove all logic that assumes eviction on new connection
- [x] 9.8 Audit all ~80 references to `connected_session_id` and `direct_remote_connected` across TUI files for correctness under new model

## 10. Testing (rewrite broken tests only)

- [ ] 10.1 Rewrite existing `daemon_tests.rs` tests that assert eviction behavior
- [ ] 10.2 Rewrite existing `daemon_tests_connection.rs` tests that assert single-connection invariants
- [ ] 10.3 Update tests that assert `AuthorityHolder::Ctrl(id)` with IDs
