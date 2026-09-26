# Tasks

Read design.md Decisions 1–4 before starting any group. They are the whole rule set.

**Count a family:** `cargo nextest list -p mbv 2>/dev/null | awk '{print $2}' | grep -c '^<prefix>'`

**Group gate** (run at the end of every group, then commit the group):
`cargo nextest run -p mbv` passes, `cargo clippy --workspace --all-targets -- -D warnings` is clean, and `cargo fmt --all -- --check` is clean. Fix any `dead_code` by deleting the helper, never with `allow`/`expect`.

## 1. Tick integration (`app::tests::tick_integration`, 290 → ≤ 60)

- [x] 1.1 List the cross-boundary contracts in design.md Decision 2 and pick one carrier test for each from existing tests (in `src/app/tests/tick_integration/`). Verify: the carrier list is recorded in the group's commit message.
- [x] 1.2 Delete every other per-screen test in `tv`, `music`, `music_mouse`, `library_panel`, `emby_library`, `podcast`, `home`, `mouse`, `feeds`, `sessions` and the remaining files, except behaviour unique to one screen (Decision 2). Verify: family count ≤ 60.
- [x] 1.3 Collapse surviving `#[case]` tables per Decision 3, and delete `…_through_tick` state tests per Decision 4 (write a fresh `app::state`/`app::dispatch` unit test only when no owner exists). Verify: `grep -c '::case_'` for the family is ≤ 10; group gate passes; commit.

## 2. Other app tests, part A (`app::tests::{queue, music_grouping, route_state, routing_matrix}`, 242 → ≤ 90)

- [x] 2.1 Apply Decisions 1 and 3 to `queue`, `music_grouping`, `route_state` and `routing_matrix`. `routing_matrix` keeps one row per distinct routing precedence rule, not per key × surface. Verify: combined count ≤ 90; group gate passes; commit.

## 3. Other app tests, part B (all remaining `app::tests::*` except tick_integration, 292 → ≤ 110)

- [ ] 3.1 Apply Decisions 1, 3 and 4 to `library_navigate_reveal`, `library_position`, `lifecycle`, `feeds`, `podcast`, `services_settings_lifecycle`, `panel_focus`, `remote_commands` and the rest. Delete whole files that are left owning nothing. Verify: `app::tests::` count minus tick_integration ≤ 200 overall (with group 2); group gate passes; commit.

## 4. Components, part A (`app::components::{tests, library_panel, list}`, 312 → ≤ 110)

- [ ] 4.1 Apply Decisions 1 and 3. Per the skill's layer matrix, component tests keep local state transitions, emitted `Msg`s, viewport and hit resolution. Delete placement, glyph and spacing assertions owned by arrangement or painter tests. Verify: combined count ≤ 110; group gate passes; commit.

## 5. Components, part B (all remaining `app::components::*`, 339 → ≤ 120)

- [ ] 5.1 Apply the same rules to `music_content`, `tv_content`, `podcast_content`, `media_list`, `help`, `sessions`, `search_sidebar`, `settings`, `mouse`, playback panels, `confirm` and the rest. Content owners that share a canonical list keep list behaviour tests only in `list`/`media_list` (break everywhere or nowhere). Verify: `app::components::` total ≤ 230; group gate passes; commit.

## 6. Dispatch and state (`app::dispatch::*` 208 → ≤ 110, `app::state::*` 110 → ≤ 60)

- [ ] 6.1 Apply Decisions 1 and 3. Delete M-TAUTOLOGICAL-TESTS cases (asserting constants, `Default`s, trivial accessors, or branch-mirroring). Keep tests for real state transitions and for dispatch arms with cross-boundary effects. Verify: both counts within budget; group gate passes; commit.

## 7. Shell, infra, input and the rest of the binary (145 + ~158 → ≤ 140)

- [ ] 7.1 Apply Decisions 1 and 3 to `app::shell::*` (≤ 70) and `app::infra::*`, `app::input::*`, `mpris`, `tray`, `local_daemon`, `single_instance`, `config` (≤ 70 combined). `app::input` keeps one test per precedence rule in `router.rs`/`key_policy.rs` and per chord behaviour in `resolver.rs`. Verify: counts within budget; group gate passes; commit.

## 8. Cleanup and docs

- [ ] 8.1 Delete test helpers, fixtures and harness functions left unused (including in `src/app/tests/tick_integration/harness.rs`), and delete empty test files with their `mod` lines. Verify: group gate passes.
- [ ] 8.2 Repoint or drop every test name cited in `docs/architecture/interactive-surface-ledger.md` so each resolves to an existing test. Verify: each cited name matches `rg -n 'fn <name>' src/`.
- [ ] 8.3 Add to the "## Tests" section of `.agents/skills/mbv-frontend/SKILL.md` a short rule: a new TUI test must own a contract no existing test owns, or reproduce a real bug; per-screen copies of a shell contract and breakpoint tables whose cases share an outcome are not added. Verify: text present.
- [ ] 8.4 Final check: `cargo nextest list -p mbv | wc -l` ≤ 800, `cargo nextest run --workspace` passes, and `make check-code-file-lines` passes. Record the final per-family counts in the commit message; commit.
