## Why

The steady-state draw path paints the entire legacy surface every frame —
`terminal.draw(|f| self.app.render(f))` at `src/app/shell_run.rs:487`, and again
at the two startup draws (`:32`, `:77`) — and *then* paints the 11 TuiRealm
component views on top (`shell_run.rs:494-504`). For a `migrated` surface this
means two painters run per frame: the legacy renderer draws the surface body,
the component overdraws it.

`migrate-tui-to-tuirealm` design D17 stage 5 requires the opposite endpoint:
"detach component geometry/content from legacy underpaint, then delete that
surface's legacy renderer." A `migrated` row whose body is still painted by a
parallel legacy path has not reached the completion gate — issue #607's
acceptance criterion "Migrated surfaces are not painted by a parallel legacy
path" is unmet, and #614 cannot certify the ledger/ADR/source agree.

The legacy `App::render` also computes `App::layout` (the `AppLayout` with
`main`, `playback`, `tabs_area`, and indicator rects) that every component
painter reads for its geometry (`shell_run.rs` component painters read
`self.app.layout.main.*`). So `self.app.render(f)` cannot simply be deleted:
its geometry computation is load-bearing, only its surface painting is
redundant.

This is issue #613's underpaint slice, split out from the deleted
`resolve-migrated-surface-correctness` bundle. It is the last of the three #613
slices and lands after the other two.

## What Changes

- **Scout first (D17).** A read-only handoff at
  `openspec/handoffs/scout-remove-migrated-surface-underpaint.md` inventories,
  per surface the legacy base frame still paints: which painter owns it now
  (legacy body, component view, or both); every layout value the legacy
  renderer produces that a component or a still-legacy surface reads; loading
  states, image prefetch/handoff, scroll reconciliation, and responsive
  variant selection that live only in the legacy path; and the smallest
  compile-complete suppression units in dependency order.
- Split `App::render` into a **geometry pass** (computes `AppLayout` and the
  per-frame facts components read, paints nothing that a component owns) and a
  **legacy chrome/body pass** (paints only surfaces no component owns this
  frame — tab bar, status bar, player chrome not yet component-owned, and any
  destination body whose component is not the active painter at the current
  breakpoint, e.g. narrow-TV / narrow-Music / album-track legacy variants).
- Route the two startup draws and the steady-state draw through **one Model
  draw entry point** so startup and steady state paint identically.
- For each `migrated` surface, suppress its legacy body painter once parity is
  proven — meaning the legacy path is demonstrably **unreached** for that
  surface at that breakpoint (execution ownership), not merely that the final
  buffer looks the same.
- Where a surface has both a component variant (wide) and a legacy variant
  (narrow) that is still the only painter for that breakpoint, the legacy
  variant's renderer is retained as the **sole** painter for that breakpoint
  and explicitly recorded as such; it is no longer an underpaint beneath a
  component.
- Delete each surface's legacy body renderer only after its last reader (a
  component geometry signal, another surface) is re-homed — folding in D18
  step 2 for any `wide_movies` / `movies_wide_right_area` residue the #611
  browser change did not already remove.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `interactive-component-framework`: the "Complete conversion with no
  mixed-framework endpoint" requirement gains a concrete underpaint
  scenario — a `migrated` surface SHALL have exactly one painter per frame at
  its active breakpoint, and the shell SHALL NOT run a legacy surface painter
  beneath a component that owns that surface. Verification is execution
  ownership (the legacy painter is unreached), not final-buffer similarity.

## Impact

- `src/app/shell_run.rs` (the three draw sites collapse to one entry point),
  `src/app/render/screens/root.rs` (`render`/`render_main` split into geometry
  vs. paint), and the per-surface legacy renderer modules under
  `src/app/render/` for each surface whose body painter is suppressed or made
  sole-for-breakpoint.
- No `App` interaction state, protocol, or persistence change. `AppLayout` and
  its fields survive; the geometry pass still produces them.
- Sequencing: lands **after** `keep-destination-components-mounted` (#613) —
  a re-shown destination must own its own state before the legacy fallback
  paint is removed — and after the four #611 mirror-removal changes
  (#615–618), several of which already detach their own surface's
  `wide_movies` residue (D18 step 2) and explicitly leave the shared
  `self.app.render(f)` call for this change.
- `docs/architecture/interactive-surface-ledger.md`: each affected row's Notes
  cell records single-painter ownership (or sole-legacy-for-breakpoint).
- ADR 0022 completion-gate wording is checked against the landed state as part
  of #614; this change makes the source match, #614 reconciles the artifacts.
