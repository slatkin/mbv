# Orchestration handoff — migrate-tui-to-tuirealm, 2026-08-25 (fifth)

Supersedes `orchestration-2026-08-25d.md` where they conflict. Read that
handoff for the established workflow, the full remaining-surface list, later
order, D16 documentation issue, and unrelated follow-ups.

## Role and workflow

You are the orchestrator. The maintainer manually starts implementation agents
from prompts written in chat. Do not dispatch subagents.

- Assume the current agent is in flight until the maintainer reports a commit.
- Do not inspect its dirty diff, edit its files, or offer another implementation
  prompt while it runs.
- Verify reports with `git show <sha>`; do not repeat suites the agent reports.
- Prompts are plain prose, never fenced, quoted, or wrapped in a slash command.
- Name the exact nested `tasks.md` row and one bounded behaviour/ownership family.
- One focused existing test group, at most one narrow regression, check + focused
  nextest + fmt. Do not request a full suite for each micro-wave.
- Do not run `check-code-file-lines` until task 5.6.

The corrected sizing rule from handoff d remains important: one coherent
behaviour family, normally a few related methods across roughly 3–6 files. Do
not bundle effect keys, mouse, state deletion, legacy underpaint, synchronization,
documentation, and framework teardown together. Conversely, do not reduce a
prompt to one trivial wrapper when adjacent operations have exactly the same
authority and proof.

## Change status

Using OpenSpec change `migrate-tui-to-tuirealm`, schema `spec-driven`. The last
CLI apply read showed 59/66 top-level/parser tasks complete. Formal task 5.3 has
only 5.3d left; operationally 5.3d still contains the remaining surface
ownership teardowns, then `CONTEXT_STACK`, then `LegacyInput`.

Parent 5.3d, nested `Mirrors and framework`, task 5.5, and task 5.6 remain
unchecked. That is correct. The unchecked mouse rows superseded by D16 are not
remaining implementation work.

## Home waves verified this session

The previous verified clean base was `9ad5cccb`.

- `8636ee4f` — `5.3d: move Home keyboard navigation to component`.
  `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`, `[`, and `]` left
  `App::handle_cw_key`; obsolete App cursor/range helpers were deleted.
  `HomeComponent` claims them only with Library panel focus. Queue focus returns
  a legacy key and does not mutate Home local state. Effect keys and all later
  boundaries were retained.
- `fe3eefc1` — `5.3d: move Home effect keys to component`.
  Enter, Ctrl+Enter, Ctrl+A, Ctrl+W, and Delete are now interpreted only by
  `HomeComponent`; typed messages route to unchanged App effect bodies.
  `handle_cw_key` was deleted. `handle_key_home` remains only as the legacy
  catch-all swallow. Global `.` context-menu precedence is unchanged.
- `086cf3c2` — `5.3d: move Home mouse clicks to shell`.
  Deleted four Home-specific `impl App` mouse handlers. A Model-boundary
  `handle_home_click` now handles pill, single-row, double-row, and right-click
  requests. Component hit geometry and local cursor/section remain authoritative;
  row clicks no longer copy into `App.home.home_cursor`. Double-click timing is
  shell/App-owned. Right-click intentionally preserves the existing Home menu
  target semantics based on `continue_cursor`, not the clicked flat target.
- `964c7a51` — `5.3d: move Home wheel scrolling to component` — implementation
  review passed but this SHA is **not yet accepted as final** because its new
  test is time-dependent. The code routes `HomeScroll` through
  `Model::handle_home_scroll`, preserving gate order (`note_browse_scroll` then
  `browse_mouse_ready`), moves the component's section-local cursor, and also
  calls `App::cw_move_cursor(delta)` to preserve the independent Continue
  Watching column quirk. The Home branch is gone from the generic
  `App::handle_mouse_scroll_browse` dispatcher.

No review defect was found in the production changes of those four commits.
Reported suites were not rerun by the orchestrator.

## In-flight correction

An agent is correcting only the test in `964c7a51`. The new test
`shell_home_wheel_moves_component_and_continue_cursor` currently assumes its
second call occurs within the real 30 ms throttle window. That can flake on a
stalled CI worker.

The exact correction request was:

- explicitly set `last_scroll_at` sufficiently in the past before the accepted
  call;
- explicitly set it to a deterministic throttle-blocking value before the
  rejected call;
- do not depend on execution completing within 30 ms;
- production behavior and all other files unchanged;
- run the focused Home test and fmt;
- amend `964c7a51` and report the replacement SHA.

When the maintainer reports, verify the replacement commit with `git show
<sha>`. Check that only the test timing setup changed relative to the reviewed
implementation, that accepted and rejected cases are deterministic, and that
production code is untouched. Do not rerun the reported tests. Assume this
correction is in flight until that report arrives; do not offer the next prompt
before then.

## Current Home boundary after the wheel correction

Home's local keyboard navigation, keyboard effect interpretation, click hit
routing, and wheel-local cursor movement are component/shell-owned. Remaining
Home work still includes coupled state/render/synchronization teardown:

- `HomePane.home_cursor` and `HomePane.section` remain on App.
- `home_select_section` still mutates App section/cursor and persists the Home
  section preference.
- several App action/target readers still use `home.section`, the independent
  Continue Watching cursor, or per-latest-section cursors.
- legacy `App::render_home_list` still underpaints, mutates App section/cursor,
  and produces load-bearing `LayoutMain` geometry before the component paints.
- `sync_home` still pushes Home content/focus flags into the component.
- the independent Continue Watching column cursor and its playback/effect
  behavior are not automatically a Home cursor mirror; preserve that distinction.
- `CONTEXT_STACK`, `LegacyInput`, and framework fallbacks remain intentionally.

After accepting the amended wheel commit, recount actual callers before choosing
the next Home slice. Do not blindly prompt all remaining Home teardown. Likely
families include legacy underpaint/geometry removal and App cursor/section
ownership, but their order must come from actual dependencies. If exact behavior
cannot be preserved within one family, ask the agent to stop and report the
confirmed coupling rather than redesigning it.

No Home documentation or checkbox should change until Home has no App
interaction state, no legacy fallback/underpaint, and no surface mirror.

## Remaining framework order

The seven interaction surfaces recorded in
`openspec/changes/migrate-tui-to-tuirealm/scoping-5.3d-mirrors.md` remain the
framework-removal lane (Home is now partly dismantled):

- `sync_home`
- `sync_emby_browser`
- `sync_tv_workspace`
- `sync_music_workspace`
- `sync_inline_search`
- `sync_audiobookshelf_podcast`
- `sync_audiobookshelf_book`

Finish Home in bounded families, then scope each other surface independently.
Do not assume their coupling or prompt size matches Home.

After every actual surface interaction mirror and App interaction endpoint is
gone:

1. delete `CONTEXT_STACK` and its App keyboard endpoints as a bounded phase;
2. delete `LegacyInput`, `Msg::Legacy`, `LegacyTerminalEvent`, and terminal
   reconstruction/adapters, splitting behavior families if needed;
3. check parent 5.3d only after no legacy endpoint, surface mirror, temporary
   adapter, or App interaction ownership remains;
4. task 5.5 ledger flip;
5. task 5.6 final gate, including full nextest, workspace clippy, fmt,
   ast-grep, and `check-code-file-lines`.

Keep domain/player projections such as playback, queue, prompts, feeds, modal
requests, settings/playlists content, visualizer/player state, subtitle prefs,
feed refresh, and queue append synchronization. Delete by ownership, never by
the `sync_` prefix.

## Deferred issues

The D16/spec-delta mismatch from handoffs c/d remains deferred until before
archive: inspect current main specs for alpha mouse promises and reconcile the
actual final `AppLayout` state through a proper delta, not a silent main-spec
edit.

Do not fold in the unrelated follow-ups about shared Wide geometry naming,
`is_wide_tv_active()` geometry inference, or `is_podcast_library` name matching.
