# Design

## Context

See proposal.md for why this change exists. Wiring facts verified 2026-09-24:

- **Module declarations.** `src/app/mod.rs` declares about 97 private sibling modules. It pulls ex-`types_*` items into the `app` namespace with private `use self::types_x::{…}` lines, so children reach them as `super::X`. It also `include!`s `app_test_modules.rs` (67 `#[path]` test modules, among them `pub(crate) mod tests` → `tests.rs`, which components and render tests use as `crate::app::tests`).
- **Existing children.** Some families are already children of one module through `#[path]`:
  - `shell` → `shell_messages`, `shell_run` (→ `shell_run_tests`) and `shell_tests`.
  - `shell_overlays` → `menus`, `modals`, `sidebars`, and `include!` of `shell_overlays_tests`.
  - `run_loop_events` → `session` and `teardown`.
  - `queue_actions` → `queue_actions_playlist_mutation`.
  - `actions` → 7 test files.
  - `input` → 2 test files.
  - `shell_tv_workspace` → `tests` → 3 nested test files.
  - `tests_tick_integration` → `music` → `landing`/`artist`; `tests_tick_integration_library_panel` → `hero`; `tests_tick_integration_mouse` → `panels`; `tests_queue_mutation` → `playlist_save`.
  - `images.rs` `include!`s `image_fetch.rs` and `image_protocol.rs`.
- **`components/` and `render/`.** `components/mod.rs` has 12 `#[path]` test modules and `render/mod.rs` has 26. `components/music_content.rs` `include!`s 4 files and `#[path]`s its tests, which `#[path]` 6 more. `components/tv_content/mod.rs:1025` `include!`s `interaction.rs`. `library_panel/` has 7 `#[path]`s, `render/components/` 6, and `render/test_helpers.rs` 3.
- **Visibility.** `src/app/*.rs` uses about 1,576 `pub(super)`/`pub(crate)`/`pub(in …)` markers and 65 bare `pub`. Nearly all `pub(super)` in an app-root file means "visible to all of `app`".
- **Paths outside `app`.** `crate::app::{App, Model, components, render, images, layout, palette, ui_util, SidebarId, capture_launch_window, current_launch_secs, set_mouse_capture}`.

## Goals / Non-Goals

**Goals:** a folder tree that mirrors the module tree; a thin `src/app/mod.rs`; zero `#[path]`/`include!` under `src/app`; every test module located where Rust expects it; stable paths for code outside `app`.

**Non-Goals:**
- A by-feature regrouping (for example `library/` holding its shell, dispatch, state and tests together). This change is by layer. Feature grouping is a later, separate decision.
- Splitting files over the 800-line cap. A pure move doesn't change their size; the pre-PR cap check handles them.
- Any change to router precedence, key policy, component ownership, `Msg`s or rendering.
- Renaming types, functions or domain terms.
- Merging tiny files. `text_safety.rs` has 3 callers across `app` and `components`, and `types_sidebar.rs` is `SidebarId`. Folding them into a `mod.rs` would break the thin-root rule. They move like any other file.

## Decisions

1. **By-layer folders now, feature folders later.** Both audits converged on `shell/ dispatch/ state/ input/ infra/`. It's a mechanical first cut that each file can be sorted into from its content (the tables below). A feature tree needs design judgment per domain and would stall the move.
2. **File-name rule.** A moved file drops only its family marker: the `shell_`, `types_`, `input_`, `run_loop_` or `tests_` prefix, or the `_actions` suffix. A `#[path]`/`include!` child becomes a real file named after its declared `mod` name, at the path Rust expects. A module that gains child files becomes `name/mod.rs`, keeping the repo's `mod.rs` style from `tidy-repo-layout`. The one exception to the prefix rule is avoiding `clippy::module_inception` (no `cast/cast.rs`).
3. **Only `src/app/mod.rs` must be thin:** `mod` declarations, `use`/re-exports, and nothing else. Family `mod.rs` files keep their former hub-file content (for example `shell/mod.rs` = today's `shell.rs`). Moving each hub into a second file would double the churn for no reader benefit.
4. **Path rewrites are mechanical.** For each moved module, rewrite references `\b(super|self|crate::app)::<old>::` → `crate::app::<new path>::` with `sd`/`sed` across `src/`, then let `cargo check` catch the remainder. This is ordinary editing, not a checker script, so the AGENTS.md scripting ban doesn't apply.
5. **Visibility is preserved exactly.** Every `pub(super)` in a file moved from app-root depth to a deeper folder becomes `pub(in crate::app)`, which is exactly its old meaning. Files that were already children (`shell_messages`, `run_loop_events_session`, …) keep their `pub(super)`. Don't narrow or widen anything else.
6. **Stable external paths.** `src/app/mod.rs` re-exports the paths listed in Context (for example `pub(crate) use self::infra::{images, layout, palette, ui_util};`). The private `use self::types_x::{…}` block becomes `use self::state::types::x::{…}`, so `super::X` inside `app` keeps working for direct children.
7. **`app/mod.rs` extraction.**
   - `infra/signals.rs`: `QUIT_REQUESTED`, `TERMINAL_GONE`, `handle_quit_signal`, `install_signal_handlers`, `stdin_has_hup`, `start_quit_watchdog`.
   - `infra/terminal.rs`: `init_terminal`, `restore_terminal`, `set_shift_escape_mode`, `set_mouse_capture`, `open_url`, the `AppTerminal` alias.
   - `test_seams.rs`, declared `#[cfg(test)]`: the 8 `*_OVERRIDE`/`*_TEST_LOCK` statics and their fn-type aliases.
   - `infra/layout.rs`: `LEFT_WIDTH_DEFAULT`, `LEFT_WIDTH_STEP`, `TWO_COLUMN_THRESHOLD`, `MINI_VIEW_THRESHOLD`, `TABBAR_LEFT_RESERVE`.
   - `dispatch/library/search.rs`: the `impl App` block (`spawn_search_sidebar_query`).
8. **`settings.rs` + `types_settings.rs` merge into `state/types/settings.rs`.** Both are about `SettingKey`; together they're 319 lines. This avoids a name collision without renaming anything.

### Target map, production files (old → new, under `src/app/`)

**`shell/`**
| old | new |
|---|---|
| `shell.rs`, `shell_messages.rs`, `shell_tests.rs` | `shell/mod.rs`, `shell/messages.rs`, `shell/tests.rs` |
| `shell_run.rs`, `shell_run_tests.rs` | `shell/run/mod.rs`, `shell/run/tests.rs` |
| `shell_{draw,root,chrome_panels,audiobookshelf_book,audiobookshelf_podcast,emby_library,emby_library_content,feeds,feeds_manage,home,home_content,inline_search,library_panel,modal_actions,playback,playlists,queue,settings}.rs` | `shell/<suffix>.rs` |
| `shell_library.rs` + tests | `shell/library/{mod,tests}.rs` |
| `shell_music_workspace.rs` + owner tests | `shell/music_workspace/{mod,<mod name>}.rs` |
| `shell_overlays*.rs` | `shell/overlays/{mod,menus,modals,sidebars,tests}.rs` |
| `shell_tv_workspace*.rs` | `shell/tv_workspace/mod.rs` + `shell/tv_workspace/tests/{mod,group,selection,activation}.rs` |

**`dispatch/`**
| old | new |
|---|---|
| `action.rs` + `action_tests.rs` | `dispatch/action/{mod,tests}.rs` |
| `actions.rs` + 7 `actions_tests*.rs` | `dispatch/actions/{mod,<mod names>}.rs` |
| `actions_navigation.rs` | `dispatch/navigation.rs` |
| `{context_menu,home,music,notify,audio_subtitle,consume_quit,cast,cast_status}_actions.rs`, `mouse_gestures.rs` | `dispatch/<name>.rs` |
| `queue_actions.rs` + `queue_actions_playlist_mutation.rs` | `dispatch/queue/{mod,playlist_mutation}.rs` |
| `lib_cursor_actions`, `lib_event_actions`, `lib_event_actions_reconcile`, `library_{browse,load,search}_actions`, `browse_level_actions`, `cw_library_tab_actions`, `shuffle_folder_actions` | `dispatch/library/{cursor,event,event_reconcile,browse,load,search,browse_level,cw_library_tab,shuffle_folder}.rs` |
| `session_command_actions`, `session_connect`, `session_switch`, `ws_event_actions`, `player_event`, `daemon_restart`, `service_startup`, `services_settings`, `emby_service_actions`, `app_emby_service_completion` | `dispatch/session/{command,connect,switch,ws_event,player_event,daemon_restart,service_startup,services_settings,emby_service,emby_service_completion}.rs` |
| `audiobookshelf_browse_actions.rs` + `audiobookshelf_book_seek_tests.rs`, `split_browse_state_book_tests.rs` | `dispatch/audiobookshelf/browse/{mod,<mod names>}.rs` |
| `audiobookshelf_service_actions`, `app_audiobookshelf_service_completion` | `dispatch/audiobookshelf/{service,service_completion}.rs` |
| `feed_actions`, `feed_tab_actions`, `feeds_manage_actions` | `dispatch/feeds/{feed,feed_tab,feeds_manage}.rs` |
| `run_loop_events.rs` + `_session`, `_teardown`, `run_loop_drains.rs` | `dispatch/run_loop/{mod,session,teardown,drains}.rs` |

**`state/`**
| old | new |
|---|---|
| `app_struct`, `app_init`, `construct`, `bootstrap`, `library_route`, `panel_targets`, `queue_scope`, `search_sidebar`, `home_latest`, `music_grouping`, `context_menu_capabilities`, `list_pane_width`, `queue_column_width` | `state/<same>.rs` |
| `library_position_state`, `panel_focus_state`, `remote_slot_state` | `state/{library_position,panel_focus,remote_slot}.rs` |
| `playback_target.rs` + `_cast`, `_local`, `_remote` | `state/playback_target/{mod,cast,local,remote}.rs` |
| `music_artist_detail.rs` + tests | `state/music_artist_detail/{mod,tests}.rs` |
| each `types_<x>.rs` | `state/types/<x>.rs`; `types_context_menu` + tests → `state/types/context_menu/{mod,tests}.rs`; `settings.rs` merged into `state/types/settings.rs` (Decision 8) |

**`input/`**
| old | new |
|---|---|
| `input.rs` + `input_music_track_scope_tests.rs`, `input_music_track_test_support.rs` | `input/mod.rs` + `input/<mod names>.rs` |
| `input_{browse_dispatch,lib_keys,playlist_keys,queue_keys,search_sidebar_keys,resolver}.rs` | `input/<suffix>.rs` |
| `input_confirm_keys.rs` + tests | `input/confirm_keys/{mod,tests}.rs` |
| `router.rs`, `key_policy.rs` | `input/router.rs`, `input/key_policy.rs` |

**`infra/`**
| old | new |
|---|---|
| `images.rs`, `image_fetch.rs`, `image_protocol.rs` | `infra/images/{mod,fetch,protocol}.rs` (`include!` → `mod`) |
| `feed_parse.rs`, `feed_parse_date.rs`, `feed_parse_tests.rs` | `infra/feed_parse/{mod,date,tests}.rs` |
| `ui_util.rs` + `tests_ui_util.rs` | `infra/ui_util/{mod,tests}.rs` |
| `layout`, `palette`, `resize`, `visualizer`, `visualizer_worker`, `render_cadence`, `text_safety`, `fuzzy_match` | `infra/<same>.rs` |
| (new) | `infra/{signals,terminal}.rs` (Decision 7) |

`src/app/` ends up holding only `mod.rs`, `test_seams.rs`, `components/`, `render/`, `shell/`, `dispatch/`, `state/`, `input/`, `infra/` and `tests/`.

### Target map, App-level tests (`app_test_modules.rs` → `app/tests/`)

- `tests.rs` → `tests/mod.rs` (stays `pub(crate)`). It declares the groups below.
- `tick_integration/`:
  - `mod.rs` ← `tests_tick_integration.rs`
  - `harness.rs` ← `tests_tick_harness.rs`
  - `music/{mod,landing,artist}.rs`, `library_panel/{mod,hero}.rs`, `mouse/{mod,panels}.rs`
  - every other `tests_tick_integration_<x>.rs` → `<x>.rs`
- `routing_matrix/{mod,blocking,focus,globals,playback,support}.rs`
- `library_position/{mod,refresh,restore,activation}.rs`
- `queue/{mod,consume,regression,reorder,scope}.rs` + `queue/mutation/{mod,playlist_save}.rs`
- `feeds/{mod,group_loading,group_nav,tab_guard,feeds_manage}.rs`
- `podcast/{mod,loading,playback}.rs`
- `route_state/{mod,session}.rs`
- Every remaining `tests_<x>.rs`, `split_browse_state_browse_level_tests.rs` and `audiobookshelf_browse_actions_sibling_tests.rs` → `tests/<x without tests_ prefix>.rs`.
- Group `mod.rs` files that had no old counterpart hold only `mod` lines.

### `components/` and `render/` test wiring

- **Declared-in-`mod.rs` test modules** (12 in `components/mod.rs`, 26 in `render/mod.rs` including `test_helpers*` and `render/tests.rs`) → `components/tests/<mod name>.rs` and `render/tests/<mod name>.rs`, under one `#[cfg(test)] mod tests;` each.
  - `render/tests.rs` already exists as `mod tests`. Its content becomes `render/tests/mod.rs`, and the 26 siblings become its children.
  - `test_helpers` becomes `render/tests/test_helpers/{mod,mounted,fixtures,music_tree}.rs`.
  - Fix the resulting `super::` paths as the compiler reports them.
- **`components/music_content.rs`** → `components/music_content/mod.rs`. Its 4 `include!`s become `mod interaction; mod tree_target; mod workspace; mod owner;` (visibility widened to `pub(super)` only where the compiler requires). Tests → `music_content/tests/{mod,tree_fixtures,tree,artist_workspace,artist_actions,tree_pointer,tree_key}.rs`.
- **`components/tv_content/mod.rs:1025`** `include!("interaction.rs")` → `mod interaction;`. `tv_content_component_tests.rs` + `_search.rs` → `components/tests/tv_content_component_tests/{mod,search}.rs`.
- **`library_panel/` and `render/components/` `#[path]` children** → real files at their `mod` names. For example `hero.rs` `#[path = "hero_tests.rs"] mod tests` → `library_panel/hero/{mod,tests}.rs`, and `panel.rs` → `panel/{mod,view,tests}.rs`.

## Risks / Trade-offs

- [Conflicts with every in-flight `src/app` change.] Precondition task 0.1. Land each task as its own commit so a rebase can replay moves.
- [A `super::` path that still resolves after a move but to a different item.] This can't happen with glob imports of the app namespace, because names are unique across `app` today; a collision would already fail to compile. `cargo check` plus the full `mbv` test suite at every task catches the rest.
- [Doc/spec paths go stale.] Task 9 sweeps them. The `AGENTS.md` repository map in particular names `shell*.rs`, `router.rs`, `key_policy.rs`, `input_resolver.rs` and `tests_tick_integration*.rs`.
- [Tick-integration test discovery and nextest filters change module paths.] Nothing in CI filters by module path (`build.yml` runs the whole suite), so this is accepted.
- [Large, noisy diff.] Every task is `git mv` plus path edits. Reviewers read the `--stat` and the `mod.rs` files; `git log --follow` keeps per-file history.

## Migration Plan

Commit per task on `main`. Revert any task's commit to roll back. No runtime migration.
