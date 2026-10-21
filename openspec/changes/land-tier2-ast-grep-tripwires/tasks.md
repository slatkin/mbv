# Tasks

## 1. The two new CI-tree rules

- [x] 1.1 Land `rules/ast-grep-rules-ci/rules/mbv/mbv-no-panic-macro-production.yml`: the four
      panic-family macros outside test items, the invariant-arm `unreachable!()` exclusion
      bound to `unreachable!` only, and its census and blind-spot comments. Verify:
      `ast-grep test --skip-snapshot-tests`.
- [x] 1.2 Land its fixtures `rule-tests/mbv-no-panic-macro-production-test.yml`: firing cases
      for each macro and two-sided cases for the invariant-arm exclusion and the test-item
      exclusion. Verify: the fixture run fails if either side is removed.
- [x] 1.3 Land `rules/ast-grep-rules-ci/rules/mbv/mbv-dispatch-no-wildcard-arm.yml`: the `_`
      wildcard arm under `src/app/dispatch/`, the test-item exclusion, and its census and
      blind-spot comments. Verify: `ast-grep test --skip-snapshot-tests`.
- [x] 1.4 Land its fixtures `rule-tests/mbv-dispatch-no-wildcard-arm-test.yml`, including the
      directive form. Verify: the fixture run fails if either side is removed.

## 2. The panic census triage

- [x] 2.1 `crates/mbv-render/src/components/help.rs`: `playback_label` returns `String` and
      falls back to `humanize_action_id` instead of `panic!`. Verify:
      `cargo check -p mbv-render`.
- [x] 2.2 `crates/mbv-theme/src/palette_json.rs`: the impossible-colour arm uses the
      invariant-arm `unreachable!()` form; the file is a `#[cfg(test)]` module the path globs
      cannot see. Verify: `cargo nextest run -p mbv-theme`.
- [x] 2.3 `src/app/shell/chrome_panels.rs`: a mount failure logs `chrome_panel.mount_failed`
      and the shell continues, instead of panicking. Verify: `cargo check -p mbv`.
- [x] 2.4 `crates/mbv-daemon/src/control.rs`: `answer_queue_op` logs a non-queue projection
      event and replies `QueueOpOutcome::Rejected`. Verify:
      `cargo nextest run -p mbv-daemon queue_op`.
- [x] 2.5 `crates/mbv-daemon/src/ws.rs`: `handle_ws_play` logs a non-Play event and returns.
      Verify: `cargo nextest run -p mbv-daemon ws_play`.
- [x] 2.6 `crates/mbv-ui-model/src/settings.rs`: `label_for` falls back to
      `"Unknown setting"`. Verify: `cargo nextest run -p mbv-ui-model`.

## 3. The wildcard census triage

- [x] 3.1 `src/app/dispatch/action.rs`: declare the typed sub-enums beside `Command`, convert
      the four sub-dispatchers to take them, and give `dispatch` one arm per domain. The
      `nest-command-by-domain` change (`openspec/changes/nest-command-by-domain/`) owns that
      shape on `main`: it renames the four sub-enums to `*Intent` (`VisualIntent`,
      `PlaybackIntent`, `NavigationIntent`, `PanelIntent`), nests `Command` as
      `Shell(ShellCommand)` / `App(AppCommand)`, and gives `App::dispatch` one arm per domain,
      which removes the flat-variant translation this tripwire gates on. Rebase this branch onto
      that change; the two invariant arms keep their `unreachable!()`. Verify:
      `cargo check -p mbv && cargo nextest run -p mbv dispatch`.
- [x] 3.2 Name the variants in the seven `Option`/enum matches:
      `library/browse/loading.rs` (`TvContentMode`), `library/browse/navigation.rs`
      (`AlbumIndexState`), `library/load.rs` (two matches), `session/emby_service.rs`,
      `session/service_startup.rs` (all 7 `AudiobookshelfFailureClass` variants),
      `session/services_settings.rs` (all 3 `ServiceEntry` variants), and the three
      exhaustive `SettingKey` lists in `settings.rs`. Verify: `cargo check -p mbv`.
- [x] 3.3 Add the seven per-instance directives with their reason line directly above each
      directive: `action.rs`, `library/browse/loading.rs`,
      `library/browse/navigation.rs` (two sites), `music.rs`, `run_loop/drains.rs`,
      `settings.rs`. Verify: the scan reports no unused-suppression finding.

## 4. The sleep residuals triage

- [x] 4.1 `crates/mbv-remote-player/src/connect/handshake.rs` + `connect.rs`: thread the
      seed-read deadline through as a parameter and add
      `connect_stream_with_seed_timeout(...)` beside `connect_stream`; production passes
      `PLAYBACK_OUTPUT_SEED_READ_TIMEOUT`. Verify: `cargo check -p mbv-remote-player
      --all-targets`.
- [x] 4.2 `crates/mbv-remote-player/src/connect/tests.rs`: order the peer thread on an
      `mpsc` channel the test signals after the handshake returns; the 1 s sleep is gone.
      Verify: `cargo nextest run -p mbv-remote-player
      a_late_partial_seed_line_is_never_consumed`.
- [x] 4.3 `crates/mbv-net/src/mock_http.rs`: add `wait_for_requests(count, timeout)` on a
      `Condvar` notified from `transmit_output`. Verify: `cargo check -p mbv-net`.
- [x] 4.4 `crates/mbv-audiobookshelf/tests/audiobookshelf/playback.rs`: wait for the outcome
      instead of the 5 ms poll loop. Verify: `cargo nextest run -p mbv-audiobookshelf
      playback`.
- [x] 4.5 `crates/mbv-net/src/bounded.rs`: keep the hang-simulator variant's sleep under a
      per-instance `// ast-grep-ignore: mbv-no-sleep-in-tests` with its reason. Verify: the
      scan reports nothing at that site.

## 5. One plain scan over the tree

- [x] 5.1 Give the four CI-tree rules their `ignores` test-path globs (`**/tests/**`,
      `**/tests.rs`, `**/*_tests.rs`, `**/test_*.rs`) and update their scoping comments.
      Verify: `ast-grep scan .` reports no test-path finding.
- [x] 5.2 Add the root `sgconfig.yml` merging both rule trees and both fixture directories.
      Verify: `ast-grep scan .` and `ast-grep test --skip-snapshot-tests` both pass at the
      repo root, with no unused-suppression findings.
- [x] 5.3 Probe the teeth in both directions: a production `panic!` and a fresh dispatch
      `_` arm fire; a shell `_` arm and the test paths stay quiet. Verify: the probe run
      (probe files deleted afterwards).

## 6. The build step

- [x] 6.1 Add `ast-grep` to the `build` job's package list. Verify: the Arch `extra`
      repository ships `ast-grep 0.44.1-1`.
- [x] 6.2 Add the tripwire step after Lint: the fixture run, then `ast-grep scan .`, with the
      severity semantics in its comment. Verify: the step is the only new step and the
      workflow parses.

## 7. Gates

- [x] 7.1 `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings`
      clean; `cargo nextest run --workspace` green.
- [x] 7.2 `make check-code-file-lines` reports no file over 800 lines
      (`src/app/dispatch/action.rs` grew with the typed sub-enums).
- [x] 7.3 The two ast-grep gates: `ast-grep test --skip-snapshot-tests` (6 suites) and
      `ast-grep scan .` (clean) at the repo root.
