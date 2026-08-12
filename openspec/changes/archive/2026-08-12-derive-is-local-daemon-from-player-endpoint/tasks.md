## 1. Unify Locality Predicates

- [x] 1.1 Add `player_owner_is_on_this_machine()` in `remote_slot_state.rs` using the existing `PlayerProxy::is_remote()` and `is_local_daemon` inputs, and replace both hand-written compound predicates with the helper without changing behavior.
- [x] 1.2 Add focused coverage for in-process, managed-local-daemon, TCP, and Unix owner-locality classifications before changing the state representation.

## 2. Store Endpoint State

- [x] 2.1 Replace `App.is_local_daemon` with `player_endpoint: Option<DaemonEndpoint>`, initialize bare-mode app and test struct literals with `None`, and add endpoint-derived `is_local_daemon()` and `player_owner_is_on_this_machine()` helpers.
- [x] 2.2 Change `run_remote_app` and `App::new_remote` to accept the complete endpoint, derive construction-only decisions and `home_is_local_daemon` with `endpoint.is_local()`, and store the endpoint for the remote player.
- [x] 2.3 Update all remote-construction call sites and rendering/test helpers, using `DaemonEndpoint::Local` for local-daemon launch paths and a fixed TCP endpoint wherever the old fixture intentionally passed `false`.

## 3. Update Player Transitions

- [x] 3.1 Change direct-remote and library-route switch functions to accept an endpoint reference, store its clone with the newly installed remote player, and pass existing endpoint values from all callers.
- [x] 3.2 Record `DaemonEndpoint::Local` after successful restoration to the local-daemon baseline and after successful local-daemon restart; preserve the previous endpoint when baseline reconnection fails and leaves its disconnected remote proxy installed.
- [x] 3.3 Replace every current-target `is_local_daemon` field read with the derived accessor while leaving every `home_is_local_daemon` read unchanged.
- [x] 3.4 Update direct field mutations and assertions in route-state, lifecycle, construction, rendering, and shared test fixtures to assign or inspect endpoints instead.

## 4. Correct In-Process Restoration

- [x] 4.1 As a separately reviewable change, clear `player_endpoint` when `restore_local_mode` reinstates `suspended_local`, so local-daemon status and queue behavior do not survive restoration of an in-process player.
- [x] 4.2 Add a regression test that enters a managed-local-daemon-classified remote target from bare mode, restores the suspended player, and verifies both current-target locality predicates and locality-dependent state reflect in-process ownership.

## 5. Verify Representation And Behavior

- [x] 5.1 Assert `player.is_remote() == player_endpoint.is_some()` after bare construction, local and non-local remote construction, both route-switch families, suspended-player restoration, successful and failed local-daemon baseline restoration, and local-daemon restart.
- [x] 5.2 Verify managed local, TCP, Unix, and in-process targets satisfy every scenario in `player-target-locality`, including the preserved Unix-is-not-local classification and immutable launch identity after target switches.
- [x] 5.3 Run `rtk cargo fmt --all -- --check`, `rtk cargo build`, `rtk cargo clippy`, and `rtk cargo test`; resolve all failures and introduce no new warnings.
- [ ] 5.4 Smoke-test queue-scope display after a Sessions-panel connection, visualizer behavior across local and genuinely remote targets, heartbeat visibility across target switches, and local-daemon restart after daemon loss.
