# Orchestration handoff — Phase 5 rescope, 2026-08-26

## State

- Change: `migrate-tui-to-tuirealm`
- Branch: `feat/migrate-tui-to-tuirealm`
- Accepted code HEAD: `354fc5c07f2a4989b70f38c14414e0f8fd30b259`
- No implementation or scout agent is active.
- Pause before delegating again; wait for the maintainer to resume.
- Intentional pre-existing `.agents/skills/ast-grep*` deletions and untracked `.pi/` remain untouched.

## Accepted work since the prior handoff

- `b414a1ec` — push Audiobookshelf podcast content at writer seams.
- `2c6bcce5` — gate the ABS drain push and cover selection-modal filter changes.
- `4eeee915` — replace podcast show-list raw keys with typed movement operations
  while preserving the split painted-cursor and App effect targets proven by
  5.3d.4.
- `0d8a4ef0` — replace episode movement/filter/exit raw keys with typed
  transitions while preserving component clamp and App filter wrap.
- `d6f67656` + `e7abcb13` — replace the podcast-specific raw action endpoint
  with typed enter/play/enqueue/modal intents while retaining unmatched global
  keys on the shared `Msg::Legacy` framework bridge.
- `4f5df745` + `354fc5c0` — replace ABS book per-frame content projection with
  mount-only reconciliation and event/key-seam pushes, guarded to the active
  Book kind.
- Luna review accepted the cumulative podcast work through 5.3d.7 and the
  corrected ABS book Phase-A unit at 5.3d.12.

## Rescope decision

Phase 5 is discovery-led ownership teardown, not mechanical `sync_*` deletion. Design D17 now requires:

1. read-only symbol-level scouting;
2. durable handoff notes;
3. separate discovery and implementation assignments;
4. normal writer units of roughly 3–6 production files;
5. staged projection → typed input/effects → state ownership → underpaint/layout → adapter removal;
6. explicit parity resolution when component and legacy behavior differ;
7. global `CONTEXT_STACK`/`LegacyInput` deletion only after every surface endpoint is gone.

Mouse is an alpha deferral under D16: delete the global router/hit map, keep already-supported component mouse paths, defer Music/modal/prompt mouse, and allow render-only layout state to remain. Do not restore a global mouse framework.

Per-unit line-cap checks are deferred to 5.6/PR. Existing per-unit compile, nextest, clippy, ast-grep, and fmt gates remain.

## Revised artifacts

The following existing OpenSpec artifacts were reconciled and `rtk openspec validate migrate-tui-to-tuirealm --strict` passes:

- `openspec/changes/migrate-tui-to-tuirealm/proposal.md`
- `openspec/changes/migrate-tui-to-tuirealm/specs/interactive-component-framework/spec.md`
- `openspec/changes/migrate-tui-to-tuirealm/specs/ui-design-system/spec.md`
- `openspec/changes/migrate-tui-to-tuirealm/design.md`
- `openspec/changes/migrate-tui-to-tuirealm/tasks.md`

`tasks.md` now contains the explicit discovery-led 5.3d.1–5.3d.24 graph. Rows 5.3d.1–5.3d.3 are complete.

## Durable scout notes

- `openspec/handoffs/scout-abs-podcast-b1-first-slice.md`
- `openspec/handoffs/scout-abs-book-phase-a.md`
- `openspec/handoffs/scout-emby-browser.md`
- `openspec/handoffs/scout-tv-workspace.md`
- `openspec/handoffs/scout-music-workspace-preliminary.md`

Music is deliberately marked preliminary and must be completed before a Music writer starts.

## Next task

Task **5.3d.4** is complete. The read-only probe proved a split current behavior:
the component's restored `selected_id` keeps the painted cursor on its local
one-item/page target, while the legacy App path still saves position and fetches
detail for its row-stride target at two columns. The exact preservation contract is
recorded in `scout-abs-podcast-b1-first-slice.md` and `tasks.md`: use the closed
`PodcastShowMove` operation enum, not an absolute cursor.

Tasks **5.3d.5–5.3d.7** landed through `e7abcb13`, and independent ABS book
Phase-A row **5.3d.12** landed through `354fc5c0`. No pre-scoped implementation
row is now ready: 5.3d.8, 5.3d.13, 5.3d.14, 5.3d.18, and 5.3d.19 are explicit
reader-inventory, parity-decision, or task-splitting rows that gate the remaining
writers. Do not automatically start another broad scout; ask the maintainer
whether to perform the next bounded planning row directly or revise the task
strategy first.
