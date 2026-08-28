# Orchestrator handoff — `remove-legacy-keyboard-endpoint`

Worktree `/home/slatkin/Dev/mbv/.worktrees/migrate-tui-to-tuirealm`, branch `feat/migrate-tui-to-tuirealm`.
Change: `openspec/changes/remove-legacy-keyboard-endpoint/` (proposal/design/tasks complete;
planning committed at `23466ac8`). Campaign base for implementation: **`23466ac8`**.

## Orchestration contract (binding on every unit)

- **Worker model:** `openrouter/deepseek/deepseek-v4-flash-0731` for ALL delegated
  agents (workers, recon, reviewer). User directive 2026-08-28: tight per-agent
  scope is the overriding concern — do not assign too many concerns to one agent.
- **One serial writer.** Many units touch shared files (`shell.rs`, `msg.rs`,
  `key_policy.rs`, `input.rs`, `input_lib_keys.rs`, `input_resolver.rs`,
  `shell_home.rs`). Never run two writers in parallel. One unit in flight at a
  time; verify before launching the next.
- **One unit = one bounded family + one new commit.** Do not amend, do not push.
  Implementer reports the commit SHA; orchestrator verifies with `git show <sha>`.
- **Implementer starts from the SHA the orchestrator names**, never its own HEAD.
- **Stay in lane.** If a unit requires touching another family, STOP and report;
  do not silently expand scope.
- **Leave handoff files alone** unless the unit is explicitly the inventory
  handoff (1.1) or an orchestrator update.
- **Verify gates per unit:** `rtk cargo check -p mbv` + focused nextest on the
  touched surface's tests; `rtk cargo fmt` on touched files; clippy on touched
  files. **DO NOT** run `make check-code-file-lines` until unit 7.2 (deferred gate).
  Do not reformat pre-existing fmt-dirt in files the unit didn't edit.
- **Commit hygiene:** stage only the named files for the unit, never `git add -A`.

## Current legacy endpoint (grounded from `23466ac8`)

- `CONTEXT_STACK` (11 entries) in `src/app/input_resolver.rs:130`.
- `App::handle_key_with_home_context` iterates it (`src/app/input.rs:91`); no-arg
  `handle_key` → `handle_key_with_home_context(key, false, None)`.
- `handle_key_view_dispatch` (`input.rs`) calls `handle_global_view_key` (q/Tab/BackTab/1-9/.),
  then `handle_key_alt` (`input_browse_dispatch.rs`), then Queue/Library dispatch.
- `Model::handle_legacy_key` (`shell.rs:128`): F1 Help-open special case + 5 blanket
  `push_*_content` re-projections (home, emby_browser, abs_podcast, abs_book, music).
- `apply_terminal_observer` (`shell.rs:98`): `TerminalObserverEvent::Key` reaches
  `handle_legacy_key` ONLY when `focused == UiRoot`; other Key events are dropped.
- `key_policy.rs`: static shadow table mirroring CONTEXT_STACK; `Custom("...")` gates
  for `confirm_skip_intro`/`confirm_next_up`/`playback`; `queue_column_width` gate is
  lossy (`IsMounted(Queue)` not `PanelMode::Both`+Shift); `panel_mode_cycle_x` owner is
  `ComponentId::Library` (no mounted LibraryComponent — D7 discrepancy, leave to #607).
- Raw `*Key` shell request variants in `msg.rs`: `GlobalViewKey`, `ConfirmKey`,
  `DaemonLostKey`, `RemoteReanchorKey`, `ContextMenuKey`, `FeedsManageKey`,
  `PlaybackPromptKey`, `SavePlaylistKey`, `QueueKey`, plus cursor-carrying
  `ServiceRequest::SettingsKey { cursor, key }` and `PersistRequest::SettingsKey { cursor, key }`.
- `to_crossterm_key_event` (`components/typed_key.rs`) called from 16 components
  (40 production call sites). `root.rs:73` is the `TerminalObserverEvent::Key` producer.
- `GlobalViewKey` emitters (prod, non-test): audiobookshelf_book, audiobookshelf_podcast,
  browser, home, music_workspace, tv_workspace.
- Two DIRECT `handle_key_with_home_context` call sites in `shell_home.rs` (Home `.` under
  Queue focus) bypassing `handle_legacy_key`.

## Unit breakdown (one writer per unit, serial)

The design's 7 families map to these dispatchable units. **1.1 first** (foundational
inventory handoff the rest build on). Unit SHA column filled as commits land.

| Unit | Tasks | Bounded family / scope (files) | Base SHA | Commit |
|------|-------|--------------------------------|----------|--------|
| **U1** | 1.1 + 1.3 | Read-only inventory: write `openspec/handoffs/remove-legacy-keyboard-endpoint.md` mapping every raw-key producer/consumer, the two direct call sites, F1 case, the 5 blanket pushes, pure-swallow notes, and the 6 precedence quirks (1.3) with their matrix rows. NO code edits. | `23466ac8` | `c0e853d0` |
| **U2** | 1.2 | Add the table-driven `Application::tick()`-level routing-matrix integration test; verify it passes against current (unconverted) behavior. Test-only. | `c0e853d0` | in flight |
| **U3** | 2.1 | `key_policy.rs`: replace descriptive/`Custom` gates with concrete TuiRealm subscriptions; fix lossy `queue_column_width` + per-key `playback` gates. | `c0e853d0`+U2 | — |
| **U4** | 2.2 | `root.rs` + `shell.rs` UiRoot: move overlay/force-clear/refresh/Panel-mode/tab/quit + F1 + `handle_key_alt` into UiRoot semantic requests; remove `TerminalObserverEvent::Key` fallback. | U3 sha | — |
| **U5** | 2.3 | `playback.rs` component + `input_lib_keys.rs::handle_playback_key`: move playback/visualizer chords + Space/Escape double-tap into Playback with typed `PlaybackRequest`s. | U4 sha | — |
| **U6** | 2.4 | Expose `skip_intro_end_ticks`/`next_up_item` as Playback-component attrs; `confirm_*` gates → real `HasAttrValue`. | U5 sha | — |
| **U7** | 2.5 | Resolve shared-globals crux (q/Tab/BackTab/1-9/.): `.` → focused destination emitting explicit target incl. Home Continue Watching case. Touches `input_lib_keys.rs`, `shell_home.rs`, destination components. | U6 sha | — |
| **U8** | 3.1 | Convert confirm/daemon_lost/remote_reanchor/context_menu/playback_prompt to accept/cancel/move/submit/dismiss intents; delete their `*Key` emitters + `to_crossterm` calls. | U7 sha | — |
| **U9** | 3.2 | Convert settings/feeds_manage/save_playlist incl. cursor-carrying `SettingsKey` variants; local text/nav state in components. | U8 sha | — |
| **U10** | 4.1 | Queue ownership: move `handle_queue_key`/`QueueKey` into `QueueComponent`; preserve `[`/`]` scope, Shift+Up/Down reorder, Ctrl+t/r, 500ms cursor-hold. | U9 sha | — |
| **U11** | 5.1 | Home + generic Emby Browser: replace `GlobalViewKey` fallthrough with component-local interpretation + explicit target requests; preserve `handle_key_emby_library` interception branches + `handle_lib_key` Ctrl/Alt catch-all. | U10 sha | — |
| **U12** | 5.2 | TV + Music workspace: replace `GlobalViewKey` fallthrough with component-local interpretation. | U11 sha | — |
| **U13** | 5.3 | Audiobookshelf podcast + book: replace `GlobalViewKey` fallthrough. | U12 sha | — |
| **U14** | 6.1 | Global deletion: `GlobalViewKey`, remaining raw `*Key` variants, `typed_key.rs`, `handle_legacy_key`, `handle_key_with_home_context`, `CONTEXT_STACK`, obsolete handlers/tests, static policy scaffolding; replace 5 blanket pushes with targeted pushes. Verify zero production raw-key consumers FIRST. | U13 sha | — |
| **U15** | 6.2 | Extend architecture gate to reject Crossterm `KeyEvent` under `components/`; remove superseded characterization tests; update interactive-surface ledger. | U14 sha | — |
| **U16** | 7.1 + 7.2 | Final gates: `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, `clippy --workspace --all-targets`, `ast-grep scan`, `make check-code-file-lines`; verify zero production matches for the 6 symbols. | U15 sha | — |

### Per-unit dispatch checklist (orchestrator)

1. Confirm working tree clean (only handoff/tasks edits tracked).
2. Dispatch ONE `subagent` worker: agent `worker`, model
   `openrouter/deepseek/deepseek-v4-flash-0731`, `async:false`, worktree isolation
   OFF (shared cwd — single serial writer), tightly-scoped plain-prose `task`
   naming the unit's task rows + exact file scope + base SHA + gates + "commit
   once, report SHA, do not amend/push".
3. On completion: `git show <sha>` to verify scope + message; run focused
   gates if the worker's report is ambiguous.
4. Update this handoff's commit column + mark `tasks.md` checkboxes.
5. Launch ONE reviewer pass (deepseek-v4-flash-0731) only if a unit changed
   behavior-critical precedence. Otherwise fold review into orchestrator
   `git show`. (Per memory: one review per unit max, no second pass.)

### Gates deferred to U16

`make check-code-file-lines` is deferred until the final unit. Do not run it
earlier. `ast-grep scan` baseline is 69 diagnostics from the prior campaign —
do not chase pre-existing findings; only flag NEW diagnostics from this change.

## Status

- [x] Planning committed (`23466ac8`)
- [x] U1 — inventory handoff (`c0e853d0`)
- [ ] U2 — routing-matrix test (in flight, run `c715bbc3`)
- [ ] U3–U7 — activate routing
- [ ] U8–U9 — overlay/form intents
- [ ] U10 — Queue
- [ ] U11–U13 — Library destinations
- [ ] U14–U15 — deletion + gate
- [ ] U16 — final gates