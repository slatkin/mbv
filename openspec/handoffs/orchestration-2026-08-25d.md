# Orchestration handoff - migrate-tui-to-tuirealm, 2026-08-25 (fourth)

Supersedes `orchestration-2026-08-25c.md` where they conflict. Read that
handoff for the established workflow, the Album track-focus review criteria,
remaining order, D16 documentation issue, and unrelated follow-ups.

## Role and workflow

You are the orchestrator. The maintainer manually starts implementation agents
from prompts written in chat. Do not dispatch subagents.

- Assume the current agent is in flight until the maintainer reports a commit.
- Do not edit its files or offer another prompt while it runs.
- Verify reports with `git show <sha>`; do not repeat suites the agent reports.
- Prompts are plain prose, never fenced, quoted, or wrapped in a slash command.
- Name the exact nested `tasks.md` row and the bounded sub-unit.
- Do not run `check-code-file-lines` until task 5.6.

## Critical sizing correction

Two prompts in this session were badly oversized:

- The whole nested `Mirrors and framework` row bundled every surface mirror,
  `CONTEXT_STACK`, and `LegacyInput`. The agent correctly stopped after four
  green waves.
- The supposedly bounded Home typed-effect prep still bundled five effects,
  App helper refactors, Model routing, broad tests, documentation, and a full
  suite. It consumed more than 250k tokens.

The next attempt then overcorrected to deleting only Home Up/Down, which the
maintainer rejected as too small. The desired size is **one coherent behaviour
family / ownership slice**, normally a few related methods across roughly 3-6
files. Do not bundle multiple effect families, documentation, framework
deletion, or a full surface teardown. Do not reduce work to one trivial wrapper
pair when the adjacent operations share exactly the same authority and proof.

Default prompt shape from now on:

- one named surface and one behaviour family;
- explicit keep/delete boundary;
- no documentation update unless the unit completes a tracked row;
- one focused existing test group, plus at most one narrow new regression test;
- check + focused nextest + fmt, not the full suite for every micro-wave;
- one implementation commit and a clean report.

Re-evaluate the next size from the landed diff each time. Do not template the
same size onto Browser, TV, Music, inline search, or Audiobookshelf; their state
coupling differs materially.

## Verified landed state before the in-flight unit

The last verified clean HEAD was `9ad5cccb` (`5.3d: remove App-owned Home
scroll state`). The orchestrator committed this five-file micro-unit after the
agent left it uncommitted. It deleted `HomePane::home_scroll`; the legacy Home
underpaint uses a local scratch scroll, and `HomeComponent::scroll` is the sole
retained Home scroll state.

Earlier verified commits after Album track focus:

- `4ce46d0a` - removed `App::blocking_overlay_active`, no-op multiselect/library
  route mirrors, and precedence-gate sync.
- `4152a9e5` - removed the overlay z-order mirror.
- `b9d1abef` + `893ba2bf` - removed the feeds-management two-way mirror and
  formatted it.
- `a46d635b` - removed the library-parent routing mirror and inert
  `LibraryComponent`.
- `d5979235` - recorded those partial waves; checkboxes remained unchanged.
- `d2b24d0c` - wired the five existing typed Home effect requests and changed
  Home play/enqueue/delete helpers to accept explicit component targets.
- `0c4c11af` - recorded the Home typed-effect prep and corrected the scoping
  document's placeholder date.
- `9ad5cccb` - deleted only `HomePane::home_scroll` and its direct uses.

The nested `Mirrors and framework` checkbox, parent 5.3d, 5.5, and 5.6 all
remain unchecked. That is correct.

## In-flight unit

An agent is implementing `5.3d > Mirrors and framework`, sub-unit `Home local
keyboard navigation`, from clean `9ad5cccb`.

The exact assigned scope is the coherent local-navigation family:

- Move `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`, `[`, and `]` fully out
  of legacy `App::handle_cw_key` ownership.
- Keep their existing `HomeComponent` implementations.
- Delete corresponding App arms and helpers that become unused, including
  private range/cursor helpers used only by those keys.
- Ensure Home claims these keys only while its Library panel is focused; Queue
  focus must fall through to legacy Queue handling without mutating Home state.
- Add or update one focused component test for that guard; otherwise reuse
  existing Home component coverage.
- Keep `home_select_section`, typed effects, Enter/enqueue/delete/watched/menu,
  mouse, Home fields, `sync_home`, and all framework teardown unchanged.
- No documentation or checkbox edits.
- Verify check, focused Home nextest, and fmt; commit as
  `5.3d: move Home keyboard navigation to component`.

Do not inspect a dirty diff as a defect while this agent is running. When it
reports, verify the commit itself. Review especially that Queue-focused keys
fall through and that effect keys were not opportunistically moved.

## Remaining framework state

`openspec/changes/migrate-tui-to-tuirealm/scoping-5.3d-mirrors.md` is the current
working record. The remaining interaction surfaces recorded there are:

- `sync_home`
- `sync_emby_browser`
- `sync_tv_workspace`
- `sync_music_workspace`
- `sync_inline_search`
- `sync_audiobookshelf_podcast`
- `sync_audiobookshelf_book`

The earlier classification keeps domain/player projections such as playback,
playback prompt, queue, feeds, modal requests, settings/playlists content,
visualizer/player state, subtitle preferences, feed refresh, and queue append
sync. Delete by ownership, never by the `sync_` prefix.

After all actual surface interaction mirrors are gone, delete
`CONTEXT_STACK`/App keyboard endpoints, then `LegacyInput`/`Msg::Legacy`/
`LegacyTerminalEvent`, in that order. Do not prompt either framework layer
while a surface still needs it.

For Home specifically, reassess after the in-flight navigation commit. The
remaining work will still include effect keys, mouse seams, App cursor/section
state, legacy underpaint, and `sync_home`; do not combine all of those into one
agent. Choose the next coherent ownership slice from actual remaining callers.

## Later order

1. Finish the bounded Home ownership slices and remove `sync_home` only when
   Home has no App interaction state or legacy fallback.
2. Scope each of the other six surfaces independently at the corrected size.
3. Delete `CONTEXT_STACK` and its App endpoints as its own bounded phase.
4. Delete `LegacyInput` and terminal-event adapters in separately reviewable
   behaviour families if necessary; do not issue one repo-wide mega-prompt.
5. Check parent 5.3d only after no legacy input endpoint, context-stack
   dispatch, surface mirror, temporary adapter, or App interaction ownership
   remains.
6. Task 5.5 ledger flip.
7. Task 5.6 final gate, including full nextest, workspace clippy, fmt,
   ast-grep, and `check-code-file-lines`.

The D16/spec-delta issue and the three unrelated naming/detection follow-ups
from the previous handoff remain deferred.
