# Handoff — migrate-tui-to-tuirealm (task 2.1 complete)

Status of OpenSpec change `migrate-tui-to-tuirealm` after the fourth
implementation session. Nothing is committed; all work is in the worktree,
uncommitted. **All Phase 1 (Foundation) tasks 1.1–1.9 and task 2.1 (Help
sidebar) are done and ticked** in
`openspec/changes/migrate-tui-to-tuirealm/tasks.md`.
Progress: 10/40 tasks complete.

## Task 2.1 — Help sidebar (complete)

### What was done

**Shell wiring (`src/app/shell.rs`):**
- `is_blocking_overlay_open()` — checks the 6 blocking overlays (context
  menu, selection modal, daemon-lost, confirm, remote-reanchor,
  save-playlist). Excludes Settings-child popups (multiselect/library-routes),
  which `mount_help` closes by closing settings.
- `mount_help()` — closes non-blocking overlays (settings/sessions/playlists),
  mounts `HelpComponent` at `Overlay(Help)`, makes it active (TuiRealm LIFO
  focus push).
- `umount_help()` — unmounts; LIFO stack auto-restores focus to `LegacyInput`.
- `render_help_overlay(f)` — if mounted, downcasts via `get_component_mut`+
  `as_any_mut`, sets destination (`effective_panel_focus` + `tab`) and panel
  area, then calls `application.view()`. Called after `self.app.render(f)` in
  the run loop's `terminal.draw` closure.
- F1 interception in the `Msg::Legacy(Key)` arm: if F1, help not mounted, no
  blocking overlay → `mount_help()`. Otherwise passes to `app.handle_key`.
- `Msg::Shell(ShellRequest::*)` arms: Quit→quit, DismissHelp→umount,
  OpenSettings/Sessions/Playlists→umount + set the matching App flag/method.

**Render-seam re-export (`src/app/render/mod.rs`):**
- Added `pub(in crate::app) use components::help::{help_destination,
  render_help_panel, HelpDestination};` so the Interactive Component can
  import the free functions without widening the private `render::components`
  module.

**Component cleanup (`src/app/components/help.rs`):**
- Fixed `handle_key` from a dead `let cmd = match ...` (all-return) to a clean
  match.
- Fixed test helper `make_key` — TuiRealm's `KeyEvent` has no `kind`/`state`
  fields (crossterm-only); removed them.
- Widened `set_destination`/`set_panel_area` to `pub(in crate::app)` (params
  `PanelFocus`/`TabSelection` are `pub(super)`).

**Legacy removal:**
- `input.rs`: removed F1 from `handle_key_global_overlay_open` (F2/F3/F4 stay).
- `input_settings_keys.rs`: deleted `handle_key_help`; removed F1 arms from
  `handle_key_settings` and `handle_key_sessions`.
- `input_playlist_keys.rs`: removed F1 arm from `handle_key_playlists`.
- `services_settings.rs`: removed F1 arm.
- `input_resolver.rs`: deleted `help_resolve`; removed `help` ContextEntry from
  `CONTEXT_STACK`; `#[allow(dead_code)]` on `KeyResolution::Swallow` (was only
  constructed by `help_resolve`; future surface conversions will use it).
- `action.rs`: removed 7 help `Command` variants (`Quit`, `CloseHelp`,
  `ShowSettings`, `ShowSessions`, `ShowPlaylists`, `ScrollBy`, `ScrollHome`),
  `help_command_for_key`, the `Command::Quit` dispatch arm, and the help
  dispatch arms; updated module doc.
- `render/screens/root.rs`: removed `show_help` render block; removed
  `show_help` from `any_other_modal_open`.
- `input_mouse_panels.rs`: removed help from `panel_w` selection,
  outside-click, and the help scroll mouse block.
- `app_struct.rs`: removed `show_help` and `help_scroll` fields.
- `construct.rs` + `tests.rs`: removed `show_help`/`help_scroll` from
  constructors.
- `library_load_actions.rs`: removed `show_help = false` from
  `open_playlists_panel`.
- `key_policy.rs`: removed `help` entry from `KEY_POLICY` table (kept in sync
  with `CONTEXT_STACK` per the `key_policy_order_matches_context_stack` test).

**Test updates:**
- `action_tests.rs`: removed `help_command_for_key` tests (10) and help
  dispatch tests (8) + `dispatch_quit` test (used removed `Command::Quit`).
- `input_resolver_tests.rs`: removed 3 `help_resolve` tests.
- `input_resolver_handle_key_tests.rs`: removed `help_f1_closes_help`,
  `help_swallows_unbound_key`, `f1_opens_help`; replaced `show_help` with
  `show_sessions` in `context_menu_open_is_refused_over_sidebar_surface`;
  removed `!app.show_help` from swallow assertion; removed `"help"` from
  `context_stack_order_is_pinned`.
- `tests_queue_scope.rs`: removed `shift_resize_is_blocked_by_help_overlay`
  (help swallow behavior now in HelpComponent tests).
- `input_movie_detail_tests.rs`: removed `help_overlay_blocks_resize_shortcuts`
  (same reason).
- `input_playback_header_mouse_tests.rs`: removed
  `panel_bounds_consume_clicks_over_the_physical_sidebar` (help mouse now in
  HelpComponent tests).

### Verification (all pass)
- `rtk cargo check -p mbv` — clean (0 errors, 0 warnings)
- `rtk cargo nextest run -p mbv help` — 27 passed
- `rtk cargo nextest run -p mbv` — 1106 passed
- `rtk cargo clippy --workspace --all-targets` — 0 new warnings (2 pre-existing
  in `list.rs`/`visualizer_worker.rs`)
- Interactive-component-boundary ast-grep rules — clean on `src/app/components/`
- `rtk make check-code-file-lines` — pass

### Notes
- No shell-routing test was added (Model::new starts a real crossterm listener
  thread, which is unreliable in headless test environments). The help
  swallow/scroll/click behavior is covered by 19 HelpComponent unit tests +
  8 render characterization tests. A shell-routing test may be feasible once
  the message fold is extracted into a testable method — deferred to avoid
  flakiness.
- `KeyResolution::Swallow` is now dead code (was only constructed by
  `help_resolve`); `#[allow(dead_code)]` added. Future surface conversions
  (Confirm modal, etc.) will construct it.
- The `no-ratatui-import-in-screens` ast-grep rule reports 71 pre-existing
  errors across `src/app/render/screens/*.rs` — these are not from this task
  and exist on the base branch.

## Phase 1 (complete, prior sessions)

All Phase 1 details preserved for reference:

### 1.1 — Dependency
- `tuirealm = "4.1"` added to the `mbv` package `[dependencies]`.

### 1.2 — MSRV
- `rust-version = "1.88"` in `[workspace.package]`; `rust-version.workspace
  = true` added to `mbv`, `crates/mbv-core`, `crates/mbvd`.

### 1.3 — `src/app/components/`
- `ComponentId`, `Msg`, `UserEvent` per design D3–D5.

### 1.4/1.5 — Shell Model + TuiRealm loop
- `src/app/shell.rs`: `Model { app, application }`. `Model::run` is the
  moved body of `App::run`. `LegacyInput` bridge mounted active at `UiRoot`.

### 1.6 — UserEvent token types
- All 10 newtypes with public fields per design Table A.

### 1.7 — key_policy precedence table
- `src/app/key_policy.rs`: 24-entry table mirroring `CONTEXT_STACK` order
  (now 23 after help removal).

### 1.8 — Mouse subscription pattern
- Documented as comment block in `key_policy.rs`.

### 1.9 — Enforcement scaffolding
- Four ast-grep rules, fixtures, CI job pinning ast-grep 0.44.1.

## Conventions

- Placeholder types carry `TODO(migrate-tui-to-tuirealm):` + owning task.
- Temporary CP1 adapters (`Msg::Legacy`, `LegacyInput`) marked with
  removal-at-5.3 TODOs.
- Render seam extractions convert `impl App` methods to free functions with
  `pub(in crate::app)` visibility so Interactive Components can call them.
- `Msg::Legacy(NoOp)` is the redraw signal for local state changes (design
  D12): a non-empty `tick` result marks the frame dirty via `had_events`.
- Delegation reports for each task are in the session transcript; this file
  is the durable record.
