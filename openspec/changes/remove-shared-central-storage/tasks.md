## 1. Local feed-entry state (behavior-preserving, lands first)

- [x] 1.1 Add the local store to `mbv-core` as a self-contained `feed_entry_state.rs` module
  (`FeedEntryState` rows keyed `(user_id, feed_id, entry_guid)`, `feed_entry_state_path()`,
  load/save through the atomic temp-file/rename pattern, put/get/scan), registered in `lib.rs`.
  Kept out of the `config_*.rs` files because a sibling refactor was splitting them in the same
  working tree, and because group 2 leaves this module standing alone (design decision 3).
  Verify with unit tests covering: round-trip, last-write-wins replacement, prefix scan
  returning only one feed's and one user's rows, absent and invalid file yielding empty state
  without error, and a failed write leaving the previous file readable.
  `cargo nextest run -p mbv-core feed_entry_state` — 5 tests pass.

- [x] 1.2 Move the TUI feed-state paths onto the store: hold it on `App`, load it at startup,
  and reimplement `hydrate_feed_entry_state`, `hydrate_feed_entries_for_subscription`, and
  `write_feed_entry_state` against it, relocating them from `shared_sync.rs` into
  `feed_tab_actions.rs` with unchanged signatures. Verify the Feeds tab hydrates stored state
  with no shared-data endpoint configured and that a write/read round-trip survives a
  simulated restart. `cargo nextest run -p mbv feed`

- [x] 1.3 Verify the three user-visible behaviors are unchanged: the All / Played / Unplayed
  filter (already covered by the `watched_filter_*` component tests), the "Watched" meta line,
  and resume-on-play. Followed the test rules: deleted the fake
  `hydration_merges_by_guid_and_ignores_unknown` test (it re-implemented the merge inline and
  exercised no production code, and it referenced the store group 2 deletes) and replaced it
  with a real App-level test that writes through the playback path, reloads from disk, hydrates
  a fresh fetch, and asserts per-user and per-feed scoping. The queue-slot clobber turned out
  not to exist: the queued item carries its own persisted position, so an empty store cannot
  zero it (design risks). `cargo nextest run -p mbv feed`

- [x] 1.4 Gate group 1: confirm the feed-state paths no longer consult a shared client at all
  (no client-state branch left in the three functions), with the shared modules still
  compiled. `rg -n "shared_client|SharedClientState" src/app/feed_tab_actions.rs` returns
  nothing, and a full `cargo nextest run` is green.

- [x] 1.5 Correct the main-spec prose a delta cannot carry: the `feed-entry-state` Purpose
  (`openspec/specs/feed-entry-state/spec.md`) still says "durable roaming state on the existing
  shared-data transport", and the `feed-subscriptions` Purpose still says the client remembers
  no playback state. Rewrite both to the local-store end state, and drop the stale `shared-data`
  entry from the shell-authority enumeration in
  `openspec/specs/interactive-component-framework/spec.md` (~lines 194, 200, 205). Verify with
  `rg -n "roaming|shared-data|shared data"` over those three files showing only local-state
  wording, and `openspec validate remove-shared-central-storage --strict` still passing.

## 2. Remove the shared-data capability

- [ ] 2.1 Delete the TUI client surface: the remainder of `src/app/shared_sync.rs`, every
  `persist_shared_document` call site (`queue_actions_playlist_mutation.rs`,
  `library_position_state.rs`, `run_loop_events_teardown.rs`), `App.shared_client` /
  `shared_reconnect_rx` (`app_struct.rs`), startup initialization (`construct.rs`),
  reconnect/backoff draining, the fallback and stale-write toasts, the shared-data chrome
  glyph and its legend entry (`chrome_status.rs`), the settings-screen roaming-settings persist
  hook (`settings.rs`) and the affected test modules. Verify
  `rg -n "shared_client|SharedClient|shared_sync|persist_roaming" src/` returns nothing and
  `cargo nextest run -p mbv` is green.

- [ ] 2.2 Delete the core modules (`shared_client`, `shared_client_transport`,
  `shared_client_tests`, `shared_service`, `shared_protocol`, `shared_state`, `shared_store`,
  `shared_worker`), their `lib.rs` exports, and the `redb` dependency from the workspace and
  `mbv-core` manifests. Verify `cargo check -p mbv-core` passes, `rg -n redb --glob '!Cargo.lock'`
  returns nothing outside archived change docs, and the remaining core tests are green.

- [ ] 2.3 Remove daemon-side hosting and advertisement: the shared-data hosting block in
  `daemon_run.rs`, the `mbv-shared-data-tcp-port` session `supported_commands` entry and its
  parser, and the unused `CTRL_CAP_SHARED_MBV_STATE` constant. Do NOT remove `daemon_core.rs`'s
  `SharedQueueState` — it is ctrl snapshot state (queue, source, observed active slot) and is
  unrelated to shared data despite the name. Verify
  `rg -n "start_shared_service|mbv-shared-data-tcp-port|CTRL_CAP_SHARED_MBV_STATE"` returns
  nothing and the daemon test suites are green (including the `mbv-shared-data-tcp-port` parser
  tests in `api_tests_client.rs`).

- [ ] 2.4 Remove the configuration surface: the five `shared_data_*` fields, their parse and
  save paths, and their validation; drop a leftover `[shared_data]` section on the next
  settings save; remove the sections from `dist/config.toml` and `dist/mbvd.toml`. Verify with
  a config test asserting a saved file no longer contains `[shared_data]`, removal of the
  `config_tests_shared_data` module, and green config tests. State the user-visible consequence
  in the commit body: a machine whose `library_routes` existed only in the shared store reverts
  to its own `config.toml`, while a machine that edited routes keeps them
  (`shell_overlays_menus.rs:696` saves config before `persist_roaming_settings` at `:700`).

- [ ] 2.5 Remove `mbvd --export-shared-data`: the action, its usage/message strings, and its
  tests. Verify `cargo nextest run -p mbvd` is green and the documented usage line lists no
  export action.

- [ ] 2.6 Update the durable docs: delete the six "Shared data and roaming" terms from
  `CONTEXT.md`, rewrite `FeedEntry`'s roaming sentence, and clear the three shared-data
  mentions in `docs/architecture/interactive-tui-component-map.md`. Verify
  `rg -n "shared-mbv-state|shared data|shared-data|roaming" CONTEXT.md docs/` returns nothing
  that describes a live facility.

- [ ] 2.7 Final gates on the whole change: `cargo fmt --all -- --check`, `cargo clippy
  --workspace --all-targets`, the full `cargo nextest run`, and
  `openspec validate remove-shared-central-storage --strict`. All green with no
  `#[allow(dead_code)]` added to silence removal fallout.

- [ ] 2.8 Update #687 with this change's outcome: the store's `redb` harness — the issue's
  byte-weight site, already fixed in 47926ae0 — is deleted outright, and the remaining sweep
  of `temp_dir()` sites stays open. Verify with the posted comment, or record why none is
  needed if the issue was closed by 47926ae0.

- [ ] 2.9 Archive the change, syncing the five capability deltas into `openspec/specs/`
  without asking (project archive guidance). Verify `openspec validate --specs` after archive
  and that `openspec/specs/shared-mbv-state/` no longer exists.
