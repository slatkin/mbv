# Handoff: migrate-tui-to-tuirealm — task 3.3 apply attempt

**Date:** 2026-08-24
**Checkpoint commit:** `88aa5310` on `feat/migrate-tui-to-tuirealm`. Includes
everything through the prior foundation/leaf/medium-surface work plus this
session's doc-only correction — treat it as the current baseline.
**Progress:** 17/40 tasks complete in `tasks.md`, unchanged this session.
No code was written.

## What happened

An `/opsx:apply` session was scoped strictly to task 3.3 ("Convert inline
library Search"). Before writing code, it traced `LibraryTab.search`'s
actual callers and `render_search_box`'s actual call sites, and found the
task is not implementable in isolation as currently described — it has a
real, previously-unscoped prerequisite. No code changes were made; `tasks.md`
3.3 is still unticked.

**Full findings (file:line detail, exact functions, recommended path
forward) are written up in
`openspec/changes/migrate-tui-to-tuirealm/scoping-3.3-3.5.md`, under
"Correction (2026-08-24, session 3)".** Read that section before attempting
3.3 or 3.5 again — do not re-derive it from scratch.

## The short version

- **Input/cursor/mouse:** `LibraryTab.search` is read/written directly in 4
  files that are task 3.5's territory, not 3.3's — `lib_cursor_actions.rs`,
  `actions_navigation.rs`, `input_mouse.rs`, `lib_event_actions.rs`.
  Tractable once `InlineSearch` owns its own results cursor, but bigger than
  scoped.
- **Render (the bigger problem, not previously scoped at all):**
  `render_search_box` only paints the query input; the results list renders
  through the same unconverted painter as the plain browse list —
  `render/components/list.rs`, `tv_wide.rs`, `movies_wide.rs`,
  `music_wide.rs` (~1,500+ lines, 5 files). This is a real slice of task
  3.5b's deferred render-seam extraction, not something 3.3 can do
  standalone — doing it separately from 3.5b risks duplicate/diverging work
  on the same functions.

## Open decision — resolve before starting 3.3 or 3.5

3.3 is structurally downstream of 3.5's render-seam work (and, for full
coverage of every library kind, downstream of 4.2/TV and 4.3/Music too,
since `render_search_box` is also called from their wide renderers). But an
earlier session's notes say **"3.5 must stay last among the currently-visible
options per a standing user constraint"** — the reasoning behind that
constraint isn't recorded anywhere read this session. These two facts
conflict. Ask the user whether that constraint still holds now that 3.3's
dependency on 3.5 is known before resequencing the task list.

## Unrelated: `CLAUDE.md` / jcodemunch note

Early this session, `CLAUDE.md` was found modified on disk mid-session
(uncommitted, not present at session start) adding a mandatory
"always use jCodemunch, never Read/Grep/Bash" policy, at the same moment the
session had determined jcodemunch's index was stale for this worktree. It
was flagged to the user as a suspected injection rather than silently
followed; the user then reindexed and confirmed it was intentional.
jcodemunch's index should be current for this worktree now.
