# Orchestration handoff - migrate-tui-to-tuirealm, 2026-08-25 (third)

Supersedes `orchestration-2026-08-25b.md` where they conflict. Read that
handoff for earlier failure modes and decisions D14-D16.

## Role and workflow

You are the orchestrator. The maintainer is manually starting implementation
agents from prompts you write. Do not dispatch subagents unless the maintainer
changes this workflow again.

- Assume the current agent is in flight until the maintainer reports a commit.
- Do not edit its files or offer the next prompt while it runs.
- Verify reports by reading `git show <sha>`, not by repeating reported suites.
- Prompts are plain prose, never fenced, quoted, or wrapped in a slash command.
- Name the exact nested `tasks.md` row so agents do not wander.
- Do not run `check-code-file-lines` until task 5.6.

## State at handoff

Verified HEAD before the in-flight unit: `f57d7d7d` (`5.4: prove six
precedence/mouse contracts via static KEY_POLICY table (D15 declined)`). The
worktree was clean then. An agent is now implementing the nested 5.3d
`Album track focus` unit; expect an in-flight dirty tree until it reports.

Recent commits, oldest first:

- `7dc15166` deleted the legacy mouse framework by filename but accidentally
  retained its global coordinate router under `mouse_gestures.rs`.
- `f795bdc8` deleted `click_set_cursor` and moved migrated surfaces to typed
  targets, but initially emitted stale/current targets instead of clicked ones.
- `792435b7` fixed Browser/Home/Queue/TV target resolution with rendered
  component tests.
- `713e2df0` fixed Home right-click to update component-local cursor state.
- `f57d7d7d` completed 5.4 through D15's static-table path. It intentionally
  left `KEY_POLICY` dead until the final framework teardown.

The 5.4 commit added verbose but real table assertions. Do not reopen or
shorten them during later units unless they directly block teardown.

## In-flight unit: Album track focus

The agent was told to make `MusicWorkspaceComponent::track_cursor` the sole
owner and delete all of these symbols:

- `LibraryTab.album_track_focus`
- `ATTR_ALBUM_TRACK_FOCUSED`
- `AlbumTrackMove`
- `AlbumTrackDismiss`

It must move focused-track target resolution from `actions.rs` and
`input_resolver.rs` to the shell/component boundary, use typed messages for
track-mode interaction, preserve non-track legacy routing, and keep narrow mode
explicitly unfocused. It must check only the nested `Album track focus` row,
not parent 5.3d.

Review its diff for:

- Copy instead of move: old App handlers or render branches surviving beside
  new component/shell code.
- A replacement mirror: no shell/App `Option<usize>` may duplicate
  `MusicWorkspaceComponent::track_cursor`.
- Enter/activation regression: focused-track Enter and action commands must
  resolve the component's track, while album-list Enter keeps existing behavior.
- Escape/navigation leakage: track-mode Esc, Up/Down/j/k must not also reach
  lower legacy contexts.
- Narrow mode: component mounted, inline track focus disabled, album activation
  still opens the established modal path.
- Tests rewritten at the component/shell boundary rather than merely deleted.
- Parent 5.3d, Mirrors/framework, 5.5, and 5.6 remain unchecked.

Do not panic if the reported `ast-grep scan` still shows the known 69
`render/screens` boundary diagnostics. They predate this unit, but task 5.6
cannot complete while they remain.

## Remaining order after Album track focus

1. `Mirrors and framework`: delete the surface `sync_*` mirrors, then
   `CONTEXT_STACK`, then `LegacyInput`, in that order. This is the last 5.3d
   child and requires every earlier child.
2. Mark parent 5.3d complete only after its actual completion criteria hold:
   no legacy input endpoint, no context-stack dispatch, no surface mirrors,
   no temporary adapters, and no remaining App interaction ownership.
3. Task 5.5: flip every interactive-surface ledger row to `migrated` with
   verification records; no `legacy` or `component` rows may remain.
4. Task 5.6 final gate: check, full nextest, workspace clippy, fmt, ast-grep,
   and `check-code-file-lines`. Split files only here if over 800 lines.

Before prompting Mirrors/framework, recount actual `sync_<surface>` methods;
the old estimate was 29 across 28 files and is now stale. Distinguish surface
mirrors from unrelated domain methods such as queue synchronization,
subtitle-preference sync, and visualizer/player state sync. Delete by ownership,
not by the `sync_` prefix.

## Open documentation issue

D16 was added only to this change's `design.md`. Before archive, inspect current
`openspec/specs/` for any requirement that still promises alpha mouse behavior.
If one exists, add the proper change delta and sync it before archive; do not
guess or silently edit the main spec. The change delta currently still says the
global `AppLayout` hit map is removed on completion, while D16 permits
load-bearing `AppLayout` geometry, so reconcile the requirement wording from
the actual final state.

## Known unrelated follow-ups

- `LayoutMain`'s `tv_wide_*` / `wide_music_*` names describe shared
  hero-on-left geometry. There is one Wide arrangement and no "TV layout."
- `is_wide_tv_active()` infers arrangement state from painted geometry.
- `is_podcast_library` matches a library name containing "podcast."

Do not fix these during the migration teardown.
