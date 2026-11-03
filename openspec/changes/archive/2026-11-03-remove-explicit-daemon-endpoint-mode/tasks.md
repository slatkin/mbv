# Tasks

## 1. Startup and CLI (src/main.rs)

- [x] 1.1 Remove `connect_daemon_arg`, its two tests, the `--connect-daemon` help lines, and the `cli_daemon_endpoint` field from `StartupArgs`. Add the rejection in `pre_config_startup` per design D1 (any `--connect-daemon` or `--connect-daemon=` argument exits 1 with guidance, before startup side effects). Verify with one test in `src/main.rs` tests (contract: the flag is rejected with the removal message) and `cargo nextest run -p mbv main`.
- [x] 1.2 Collapse the startup branch: `main` calls `run_local_instance` directly, keeping the `startup.application.started` event; delete `run_configured_startup`; inline `run_remote_app` into `attach_owner_process` (design D4). Verify with `cargo check -p mbv` and `cargo nextest run -p mbv`.

## 2. Config key (crates/mbv-config)

- [x] 2.1 Remove `daemon_client_endpoint` from `Config` and its default; stop reading `mbvd.client_endpoint` in `parse.rs` and add the warn-once check per design D2 (event `config.mbvd.client_endpoint`); stop writing `[mbvd.client] endpoint` in `save.rs`. Replace the `parse_daemon_client_endpoint` test with one that asserts a config carrying the stale key still parses with the other `[mbvd]` values intact (contract: stale key ignored, rest of file unaffected). Verify with `cargo nextest run -p mbv-config`.

## 3. Launch-mode state removal (src/app)

- [x] 3.1 Remove `launched_as_remote` and `home_is_local_daemon` from `App` (`app_struct.rs`, `construct.rs`, `construct/remote.rs`) and apply the design D3 reductions in `switch.rs`, `connect.rs`, and `teardown.rs` (shutdown conjunct drops; persistence gate keeps only `auto_reconnect`; `restore_local_mode`'s bare branch becomes the unconditional reconnect). Update the daemon-restart doc comment that names the field. Verify with `cargo check -p mbv`.
- [x] 3.2 Update the touched tests: delete `teardown_skips_persistence_for_an_explicit_remote_daemon_launch` (its contract is gone with the mode), drop the `home_is_local_daemon` assertions in `tick_integration/disconnect.rs`, plant `stay_alive`/setup explicitly where the tests relied on the field, and reword `new_remote_does_not_auto_reconnect_for_an_explicit_remote_daemon` to its surviving contract (a non-local endpoint construction never auto-reconnects). Verify with `cargo nextest run -p mbv`.

## 4. Sessions transfer gates (interactive boundary)

- [x] 4.1 Remove `home_is_local_daemon` from `SessionTransferGates` (`crates/mbv-ui-model/src/panel_targets.rs`), from the sidebars projection (`src/app/shell/overlays/sidebars.rs`), and from the component gates in `crates/mbv-components/src/sessions.rs` and its tests. Verify with `cargo nextest run -p mbv-components -p mbv-ui-model` (contract: `h`/`t` still emit only when the D6 source-play and direct-endpoint conditions hold).
- [x] 4.2 Rework the chrome-status regression test that plants `daemon_client_endpoint` (`an_empty_named_peer_labels_the_connected_endpoint_not_the_configured_one`) to assert the surviving contract directly: the pill names the connected endpoint's host, not a configured one (design D3). Verify with `cargo nextest run -p mbv chrome_status`.

## 5. Documentation

- [x] 5.1 Amend ADRs 0006, 0010, 0015, and 0016 in place per design D5 (amendment blocks; 0016 records the removal of `home_is_local_daemon` and names `player_endpoint` as the surviving record). Verify by reading the four ADRs for leftover mode wording: `rg -n "connect-daemon|daemon_client_endpoint" docs/adr/`.
- [x] 5.2 Check `CONTEXT.md` for terms the removal retires (launch mode, explicit endpoint) and update it with the change. Verify with `rg -n -i "connect-daemon|daemon client endpoint" CONTEXT.md docs/recipes.md` returning nothing.

## 6. Integration checks

- [x] 6.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. Verify all pass.
- [x] 6.2 Run `make check-code-file-lines` just before push and split any file over 800 lines. Verify it passes.

## Workflow follow-up

- Sync the deltas into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.
