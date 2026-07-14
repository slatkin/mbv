# Implementation Plan: Issue #199 Sticky Library Position

## Overview
Implement restart-persistent library browsing position, scoped by library and by view (`default` vs `power`). A library position restores the drill-down path, active selector/group where applicable, and focused item; scroll remains derived/clamped to keep the focused item visible. Power View queue-side vs library-side panel focus is a separate restart-persistent preference.

## Architecture Decisions
- Store library position in a separate `library_position_state.json`, not `prefs.json`, with one file covering all libraries.
- Persist restore identities and fetch context only, not full item lists. Each path segment records `parent_id`, `focused_item_id`, fallback `cursor_index`, `title`, and fetch context (`item_types`, `unplayed_only`, `sort_by`, `sort_order`).
- Restore lazily when a library/view becomes active. If saved state exists, avoid rendering root first and then jumping; use existing loading/restoring surfaces until the saved path or fallback is ready.
- Keep default library view and Power View isolated both across restart and in-session. Switching views should activate that view's independent position for the library, not share one live `nav_stack`.
- Restore stale state by validating each path segment against current Emby data. Keep the deepest valid prefix, prefer saved item IDs, fall back to clamped indices, and rewrite the state file only after a stable fallback is successfully resolved.
- Manual refresh/rescan clears the active library/view position immediately on request. Other libraries and the other view scope remain intact.
- Keep hidden or currently missing library entries in the state file. They are inert while hidden/missing and may restore if the library returns.
- Do not persist search, detail-panel state, album track-selection focus, or Home/CW internal dashboard position.

## Task List

### Phase 1: Persistence Foundation
- [ ] Task 1: Add library position state types and config I/O.
- [ ] Task 2: Add snapshot/restore conversion helpers around existing library view state.

### Checkpoint: Foundation
- [ ] Focused persistence/helper tests pass.
- [ ] No UI behavior changes yet.

### Phase 2: Default View Sticky Position
- [ ] Task 3: Save default-view library position on logical navigation changes.
- [ ] Task 4: Lazily restore default-view library position without root-first flash.

### Checkpoint: Default View
- [ ] Default library restart restore works.
- [ ] Stale default-view fallback works and rewrites only after stable restore.

### Phase 3: Power View Isolation
- [ ] Task 5: Persist Power View panel focus in `prefs.json`.
- [ ] Task 6: Save and restore independent Power View library position.
- [ ] Task 7: Swap isolated default/power positions during in-session view changes.

### Checkpoint: Power View
- [ ] Default and Power View positions do not bootstrap from or overwrite each other.
- [ ] Power View queue-side/library-side focus survives restart independently from library position.

### Phase 4: Reset Boundaries and Regression
- [ ] Task 8: Clear active-view position immediately on refresh/rescan request.
- [ ] Task 9: Add acceptance/regression coverage and documentation checks.

### Checkpoint: Complete
- [ ] Full test suite passes.
- [ ] GitNexus `detect_changes()` shows expected affected symbols/flows before commit.
- [ ] `CONTEXT.md` remains current; add ADR only if implementation introduces a surprising hard-to-reverse trade-off.

## Risks and Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| `BrowseLevel` and `App::build` are high-blast-radius symbols. | High | Keep changes additive, avoid signature churn, and test existing navigation/render paths after each slice. |
| Current code likely uses one in-memory `nav_stack` per library. | High | Introduce explicit view-scoped snapshot/activation helpers before changing user-facing behavior. |
| Lazy restore requires network fetches and async event sequencing. | High | Build restore as a small state machine with focused tests for success, stale fallback, and error/no-jump behavior. |
| Opportunistic writes can become chatty. | Medium | Write only on logical position changes; debounce/coalesce only if existing event-loop seams make that simple. |
| Hidden/missing libraries may look like stale data. | Medium | Treat absent library IDs as inert, not prune candidates. |

## Open Questions
- None. Remaining issue #199 choices are resolved by the recommended defaults captured above.

## Implementation Notes
- Before editing existing functions/classes/methods, run GitNexus impact analysis per `AGENTS.md`.
- Prefer tests that exercise app behavior at existing seams over tests that assert private implementation details.
- Do not open a PR unless explicitly requested.
