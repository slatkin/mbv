# Tasks

Rules for every task:

- Moves use `git mv`. The target paths are in design.md's "Target map" tables.
- Apply design Decisions 2 (names), 4 (path rewrites), 5 (`pub(super)` → `pub(in crate::app)` for files moved down from app-root depth) and 6 (re-exports).
- No renames of types, functions or domain terms. No logic edits.
- **Gate:** `cargo check -p mbv --all-targets`, `cargo nextest run -p mbv`, `cargo clippy -p mbv --all-targets -- -D warnings` and `cargo fmt` all pass, then commit with the task number in the message.
- **Verify** means the gate passes plus the task's own check.

## 0. Preconditions

- [x] 0.1 Confirm `tidy-repo-layout` is archived and `openspec list` shows no active change that edits `src/app/`. As of planning, `gate-all-queue-replacements`, `coverage-quick-wins` and `three-line-flat-list` must be archived first; `modularize-mbv-core-layout` should preferably be archived too. Verify: `ls openspec/changes` lists none of them outside `archive/`.

## 1. `infra/` and a thin `app/mod.rs`

- [x] 1.1 Create `infra/` and move the files from the `infra/` table: `images` becomes a directory (`include!` → `mod fetch; mod protocol;`), `feed_parse` a directory with `date` and `tests`, and `ui_util` a directory with `tests`, where `tests_ui_util.rs` leaves `app_test_modules.rs`. Re-export `images`, `layout`, `palette` and `ui_util` from `app/mod.rs` as `pub(crate) use self::infra::{…};`. Verify: gate, `rg -n 'crate::app::(images|layout|palette|ui_util)' src` still compiles unchanged, and `rg -n 'include!' src/app/infra` is empty.
- [x] 1.2 Extract the logic out of `app/mod.rs` per design Decision 7 into `infra/signals.rs`, `infra/terminal.rs`, `test_seams.rs` (`#[cfg(test)] mod test_seams;`), the `infra/layout.rs` constants, and `dispatch/library/search.rs` (create `dispatch/mod.rs` and `dispatch/library/mod.rs` with just those `mod` lines; the rest of `library_search_actions.rs` moves in task 4.1). Re-export `set_mouse_capture`. Verify: gate, and `rg -n '^(static|fn|extern|impl|const|type|pub\(super\) const|pub\(super\) fn|pub\(crate\) fn)' src/app/mod.rs` is empty.

## 2. `state/`

- [x] 2.1 Move every `types_*.rs` to `state/types/<x>.rs` (`context_menu` as a directory with its tests), merge `settings.rs` into `state/types/settings.rs` (Decision 8), and replace the `use self::types_x::{…}` block in `app/mod.rs` with `use self::state::types::x::{…}`. Keep `pub(crate) use … SidebarId`. Verify: gate, and `ls src/app/types_* src/app/settings.rs` matches nothing.
- [x] 2.2 Move the rest of the `state/` table (app_struct, app_init, construct, bootstrap, the `*_state` files, `playback_target/`, `music_artist_detail/`, etc.). Keep the `pub use self::…::App`, `capture_launch_window` and `current_launch_secs` re-exports stable. Verify: gate, and none of the `state/` table's old paths exist.

## 3. `input/`

- [x] 3.1 Move the `input/` table, including `router.rs` and `key_policy.rs`. `input.rs`'s two `#[path]` children become `input/<mod name>.rs`, and `input_confirm_keys` becomes a directory with `tests.rs`. Verify: gate, `ls src/app/input_* src/app/router.rs src/app/key_policy.rs` matches nothing, and `rg -n '#\[path' src/app/input` is empty.

## 4. `dispatch/`

- [x] 4.1 Move `dispatch/library/*`, `dispatch/session/*` and `dispatch/audiobookshelf/*` (including `audiobookshelf_browse_actions`' two `#[path]` test children). Verify: gate, and the old paths for those rows don't exist.
- [x] 4.2 Move the rest of the `dispatch/` table: `action/`, `actions/` (7 `#[path]` test children → `actions/<mod name>.rs`), `navigation`, the loose `*_actions`, `mouse_gestures`, `queue/`, `feeds/`, `run_loop/`. Verify: gate, `ls src/app/*_actions*.rs src/app/run_loop_* src/app/action*.rs` matches nothing, and `rg -n '#\[path' src/app/dispatch` is empty.

## 5. `shell/`

- [x] 5.1 Move the `shell/` table. `shell.rs` becomes `shell/mod.rs`. Every existing `#[path]`/`include!` child (`messages`, `run/tests`, `tests`, `library/tests`, `music_workspace/…`, `overlays/{menus,modals,sidebars,tests}`, `tv_workspace/tests/…`) becomes a real file, and the redundant `#[path = "shell_library_panel.rs"]` goes away. Verify: gate, `ls src/app/shell*.rs` matches nothing, and `rg -n '#\[path|include!' src/app/shell` is empty.

## 6. App-level tests

- [x] 6.1 Create `tests/` from `tests.rs` (as `tests/mod.rs`, still `pub(crate)`) and move the `tick_integration/` group, including the nested `music/`, `library_panel/` and `mouse/` children and `harness.rs`. Remove those entries from `app_test_modules.rs`. Verify: gate, the test count in `cargo nextest run -p mbv` is unchanged from before the task (record both numbers here), and `ls src/app/tests_tick_*` matches nothing. (Test count recorded: 2356 before → 2356 after.)
- [x] 6.2 Move the remaining groups (`routing_matrix/`, `library_position/`, `queue/`, `feeds/`, `podcast/`, `route_state/`) and the loose test files into `tests/`. Delete `app_test_modules.rs` and its `include!` in `app/mod.rs`. Verify: gate, test count unchanged (record it), `ls src/app/*.rs` prints only `mod.rs` and `test_seams.rs`, and `rg -n '#\[path|include!' src/app/*.rs src/app/{shell,dispatch,state,input,infra,tests}` is empty. (Test count recorded: 2356 before → 2356 after.)

## 7. `components/` and `render/` test wiring

- [x] 7.1 `components/`: move the 12 `#[path]` test modules into `components/tests/`, make `music_content` a directory (4 `include!`s → `mod`s, tests → `music_content/tests/`), replace the `tv_content/mod.rs` `include!("interaction.rs")` with `mod interaction;`, and turn `library_panel/`'s 7 `#[path]` children into real files (design "components/ and render/"). Verify: gate, test count unchanged, and `rg -n '#\[path|include!' src/app/components` is empty.
- [x] 7.2 `render/`: move the 26 `#[path]` test modules plus `test_helpers*` under `render/tests/` (with `render/tests.rs` → `render/tests/mod.rs`), and turn `render/components/`'s 6 `#[path]` children into real files. Verify: gate, test count unchanged, and `rg -n '#\[path|include!' src` is empty. (Deviation: the whole-`src` form of that last check reports exactly one pre-existing, out-of-scope site, `src/mpris.rs:562`; `src/app` is clean. Reviewer-confirmed out of scope. Test count 2356 → 2356.)

## 8. Final gates

- [x] 8.1 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace` all pass, and `src/app/mod.rs` contains only `mod`, `use` and `pub … use` lines (plus attributes and comments).

## 9. Docs

- [ ] 9.1 Update path citations for the moved files in `AGENTS.md` (repository map; router/key_policy/input_resolver; `tests_tick_integration*.rs`), `.agents/skills/mbv-frontend/SKILL.md`, `docs/architecture/`, `docs/invariants/`, `CONTEXT.md` and `openspec/specs/**`. Find them with `rg -n 'src/app/[a-z_]+\.rs' AGENTS.md CONTEXT.md docs .agents openspec/specs` and leave archived changes alone. Verify: every `src/app/…` path cited in those files exists (`test -e`), then commit.
