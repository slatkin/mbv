## 1. Local feed-entry state (behavior-preserving, lands first)

- [ ] 1.1 Add the local store to `mbv-core`: `FeedEntryState` beside the other persisted
  state types, `feed_entry_state_path()` in `config_paths.rs`, and load/save plus
  put/scan-by-`(user_id, feed_id)` in `config_state.rs` using the existing atomic
  temp-file/rename write. Verify with unit tests covering: round-trip, last-write-wins
  replacement, prefix scan returning only one feed's and one user's rows, missing and
  invalid file yielding empty state without error, and a failed write leaving the
  previous file readable. Use `TestStateDirGuard` / `TestTempDir`, never a bare
  `temp_dir()` join. `cargo nextest run -p mbv-core feed_entry`

- [ ] 1.2 Move the TUI feed-state paths onto the store: hold it on `App`, load it at startup,
  and reimplement `hydrate_feed_entry_state`, `hydrate_feed_entries_for_subscription`, and
  `write_feed_entry_state` against it, relocating them from `shared_sync.rs` into
  `feed_tab_actions.rs` with unchanged signatures. Verify the Feeds tab hydrates stored state
  with no shared-data endpoint configured and that a write/read round-trip survives a
  simulated restart. `cargo nextest run -p mbv feed`

- [ ] 1.3 Verify the three user-visible behaviors are unchanged: the All / Played / Unplayed
  filter, the "Watched" meta line, and resume-on-play. Add a shell test proving that playing
  an entry from the Feeds list resumes from stored position and that re-playing it does not
  reset the queue slot's persisted position to zero. `cargo nextest run -p mbv feeds` plus
  the feed render tests at Normal and Wide breakpoints

- [ ] 1.4 Gate group 1: confirm the feed-state paths no longer consult a shared client at all
  (no client-state branch left in the three functions), with the shared modules still
  compiled. `rg -n "shared_client|SharedClientState" src/app/feed_tab_actions.rs` returns
  nothing, and a full `cargo nextest run` is green.

## 2. Remove the shared-data capability

- [ ] 2.1 Delete the TUI client surface: the remainder of `src/app/shared_sync.rs`,
  `App.shared_client` / `shared_reconnect_rx`, startup initialization, reconnect/backoff
  draining, teardown document writes, the fallback and stale-write toasts, the shared-data
  chrome glyph and its legend entry, and the settings-screen roaming-settings persist hook.
  Verify: `rg -n "shared_client|SharedClient|shared_sync" src/` returns nothing and
  `cargo nextest run -p mbv` is green.

- [ ] 2.2 Delete the core modules (`shared_client`, `shared_client_transport`,
  `shared_client_tests`, `shared_service`, `shared_protocol`, `shared_state`, `shared_store`,
  `shared_worker`), their `lib.rs` exports, and the `redb` dependency from the workspace and
  `mbv-core` manifests. Verify `cargo check -p mbv-core` passes, `rg -n redb --glob '!Cargo.lock'`
  returns nothing outside archived change docs, and the remaining core tests are green.

- [ ] 2.3 Remove daemon-side hosting and advertisement: the shared-data hosting block in
  `daemon_run.rs`, the `mbv-shared-data-tcp-port` session `supported_commands` entry and its
  parser, and the unused `CTRL_CAP_SHARED_MBV_STATE` constant. Verify
  `rg -n "start_shared_service|mbv-shared-data-tcp-port|CTRL_CAP_SHARED_MBV_STATE"` returns
  nothing and the daemon test suites are green.

- [ ] 2.4 Remove the configuration surface: the five `shared_data_*` fields, their parse and
  save paths, and their validation; drop a leftover `[shared_data]` section on the next
  settings save; remove the sections from `dist/config.toml` and `dist/mbvd.toml`. Verify with
  a config test asserting a saved file no longer contains `[shared_data]`, removal of the
  `config_tests_shared_data` module, and green config tests.

- [ ] 2.5 Remove `mbvd --export-shared-data`: the action, its usage/message strings, and its
  tests. Verify `cargo nextest run -p mbvd` is green and the documented usage line lists no
  export action.

- [ ] 2.6 Update the durable docs: delete the six "Shared data and roaming" terms from
  `CONTEXT.md`, rewrite `FeedEntry`'s roaming sentence, and clear the three shared-data
  mentions in `docs/architecture/interactive-tui-component-map.md`. Verify
  `rg -n "shared-mbv-state|shared data|shared-data|roaming" CONTEXT.md docs/` returns nothing
  that describes a live facility.

- [ ] 2.7 Correct the `feed-subscriptions` Purpose in the main spec to local-state wording
  (a delta cannot carry a Purpose change) and confirm no other main spec still describes the
  removed facility. `openspec validate --specs` passes with no zero-delta findings.

- [ ] 2.8 Final gates on the whole change: `cargo fmt --all -- --check`, `cargo clippy
  --workspace --all-targets`, `ast-grep scan`, the full `cargo nextest run`, and
  `openspec validate --change remove-shared-central-storage`. All green with no
  `#[allow(dead_code)]` added to silence removal fallout.

- [ ] 2.9 Update #687 with this change's outcome: the store's `redb` harness — the issue's
  byte-weight site, already fixed in 47926ae0 — is deleted outright, and the remaining sweep
  of `temp_dir()` sites stays open. Verify with the posted comment, or record why none is
  needed if the issue was closed by 47926ae0.

- [ ] 2.10 Archive the change, syncing the five capability deltas into `openspec/specs/`
  without asking (project archive guidance). Verify `openspec validate --specs` after archive
  and that `openspec/specs/shared-mbv-state/` no longer exists.
