# Plan: Implement `conform-tv-latest-hero`

## Context

The OpenSpec change `openspec/changes/conform-tv-latest-hero/` (proposal db22b6f4c) is fully planned — proposal, design, 5 spec deltas, 18 tasks, 0 done. This session implements it.

Problem: TV Latest shows no hero in Wide geometry while every other Latest list does. Three TV-only rules cause it: a second breakpoint source that inspects row types (`is_wide_tv_library`), an owner hero gate on that wrong bit, and a `flat_tv_list` branch in the panel that names TV. Also, TV Upcoming rows are Emby premiere notices carried as fake `EmbyItem`s and told apart by `id.is_empty()`, so each new path can treat a notice as playable media.

Product decisions are already made (design.md; the BREAKING items were user decisions on 2026-10-11):
- TV breakpoint comes from terminal geometry only (D1).
- The panel decides Hero-pane-or-not from a typed `HeroSlot`, not from the destination key (D2).
- Upcoming premieres become their own non-media type `UpcomingPremiere` at the Emby boundary (D3).
- TV Latest's Wide hero uses the same `hero_content_emby` producer as Movies (D4).
- One rule for narrow Enter/double-click on Latest rows: play at once, no overlay (D5).
- The TV-only mini-view auto-open is deleted (D6).

Full detail lives in the change directory — this plan sequences the work, it does not restate it.

## Approach

Implement in the task-group order of `tasks.md` (1 → 7). Groups 1–4 change the panel/owner plumbing with no behavior change except TV Wide Latest hero; groups 5–6 flip Upcoming to premieres; group 7 is docs and gates. One deviation from a literal reading of task 5.1: `get_upcoming` already requests `SeriesId` (via `LATEST_EPISODES_FIELDS`, `crates/mbv-emby/src/client_library.rs:12`) and parses it (`crates/mbv-emby/src/types_parsing.rs:325`), so the stop-and-report precondition is met — confirm this again at implementation time, then proceed.

Work in a dedicated worktree via the `git-worktrees` skill: `wt conform-tv-latest-hero`.

## Files to modify

Key files (the compiler finds the rest of the `HeroSlot` sites):

- `crates/mbv-components/src/library_panel/content.rs` — `HeroSlot { Pane(Option<HeroContent>), Absent }` replaces `hero: Option<HeroContent>`; every constructor and reader updated, no wildcard arms (task 1.1).
- `crates/mbv-components/src/library_panel/panel/view.rs`, `panel/mouse.rs` — skeleton hero-pane from the slot; delete `flat_tv_list`; narrow Enter/double-click overlay refusal via `latest_list_active` (tasks 1.2, 4.1).
- `crates/mbv-components/src/tv_content/` (`panel_owner.rs`, `episode_rows.rs`, `keyboard.rs`, `interaction.rs`, `tree_projection.rs`) — Latest Wide hero, Upcoming rows from premieres, no media actions in Upcoming (tasks 2.3, 6.1, 6.2).
- `src/app/state/projection/tv_wide.rs` — delete `is_wide_tv_library`; rename to `wide_tv_show_area` with a content-mode predicate (task 2.1).
- `src/app/shell/tv_workspace.rs`, `shell/run.rs`, `input/browse_dispatch.rs`, `shell/overlays/menus.rs` — breakpoint push, mini-view auto-open deletion, Upcoming activation (tasks 2.2, 3.1, 6.2).
- `src/app/dispatch/` (`library/event/browse_loads.rs`, `music.rs`, `actions/letter_tests.rs`) — snapshot/event/selection types move to `Vec<UpcomingPremiere>` (tasks 5.3, 6.2).
- `crates/mbv-emby-model` — new `UpcomingPremiere` with `stable_key()`; `crates/mbv-emby/src/client_library.rs` — `get_upcoming` maps at the boundary, drops no-`SeriesId` rows with one log line (task 5.2).
- `crates/mbv-ui-msg` — `ShellRequest::TvOpenUpcomingSeries { series_id, series_name }` (task 6.2).
- `CONTEXT.md` — new *Upcoming premiere* term (task 7.1).

## Reuse

- `hero_content_emby` — existing hero producer Home already uses for episodes (design D4).
- `App::open_series_for_unplayable_episode` reveal logic (letter range, series navigation, Workspace open) — kept, retyped to a series reference (D3).
- `App::tv_upcoming_snapshots` store, `TvUpcomingSnapshotFetched`/`Failed` events, `maybe_spawn_tv_upcoming_snapshot` dedup — all exist from commit 55995d9d0; only their payload type changes.
- `upcoming_episode_rows` row builder — rebuilt over the new context field, same visual output.

## Steps

Checklist mirrors `tasks.md`; tick both places as work completes.

- [x] 1. Hero slot type in the Library panel (tasks 1.1–1.3). `HeroSlot`, all constructors/readers, one panel test for "non-media rows have no Hero pane".
- [x] 2. One breakpoint source + TV Latest Wide hero (tasks 2.1–2.4). `wide_tv_show_area`, geometry-only `set_is_wide`, Latest hero in every geometry, owner test for Wide Latest hero + keyboard addressing.
- [ ] 3. Remove the mini-view auto-open (task 3.1). Delete `sync_tv_mini_view_hero` chain; handle the `struct_excessive_bools` expectation correctly (remove the attribute if unfulfilled; never add a suppression).
- [ ] 4. Latest rows play on narrow Enter/double-click (tasks 4.1–4.2). `latest_list_active` trait method, panel-side refusal, one `#[case]` table test.
- [ ] 5. Upcoming premiere type and storage (tasks 5.1–5.3). Confirm `SeriesId` first; `UpcomingPremiere`, client-boundary mapping with drop+log, snapshot/event/selection retyping.
- [ ] 6. Upcoming rows, activation, media-action removal (tasks 6.1–6.3). Rows from premieres, `TvOpenUpcomingSeries`, no play/enqueue/watched/menu in Upcoming, owner test table.
- [ ] 7. Docs and gates (tasks 7.1–7.2). `CONTEXT.md` term; `cargo fmt`, clippy `-D warnings`, nextest workspace.

## Test discipline

- Every new test names its contract and layer (tasks already cite both). One `#[rstest]` table per contract, not one test per row.
- Delete (do not rework) tests that assert removed behavior: "no episode hero outside mini view", "an Upcoming row with an id plays", narrow-Latest-opens-overlay.
- All tests hermetic; no live mpv, no real config dirs, no live servers, no sleeps.

## Verification

1. Per-group: the `cargo nextest run -p <crate>` command each task names.
2. Final gates: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` — all clean.
3. `make check-code-file-lines` before push; no file over 800 lines.
4. Spec sync: apply the 5 delta specs into `openspec/specs/`, then archive the change (openspec sync + archive skills), after user review.
5. User live-verifies on their Emby server (rebuild + restart the stay-alive owner): TV Latest shows the Wide hero; Upcoming rows open their series and cannot play; Movies/Home Videos/Feeds Latest Enter plays at once in narrow geometry.

## Workflow rules honored

- No dirty worktree: commit per task group on the worktree branch and push; incomplete work still commits with commits that say so.
- Bug-fix-style deviations from tasks.md (if the compiler or tests surface something the plan missed) get recorded in the change's tasks.md, not silently absorbed.
- No lint suppressions without per-instance user approval; fix the cause.
