## Why

Colour *definitions* each live in one place, but colour *decisions* — which
surface paints what, and when it reacts to focus — are spread over ~66
production sites through six mechanisms: the shared lever
(`resolve_surface_focus`), direct role names, private duplicate roles, one role
carrying several meanings, per-screen focus bits, and geometry owned elsewhere.
The archived change `unify-surface-colour` (2026-09-11) solved this, but it
bundled a focus-semantics normalisation (a frame-level `FocusState` replacing
each site's own focus input) that was sampled on screen and declined: panels
lit with focus where main kept them still, and inconsistently. PR #689 carries
that version; this change re-runs the unification with behaviour locked to
main's.

## What Changes

- Add the closed `Surface` identity table (the archived change's 34-identity
  inventory, re-validated against main) in `src/app/render/theme/`:
  `surface.rs` / `surface_table.rs` / `surface_resolve.rs`.
- One resolver entry point, `surface_colors(surface, focused: bool) ->
  SurfaceColors`, where `focused` is each call site's own existing focus input
  passed through verbatim — component focus, a pane's `held` bit, a Panel focus
  bool, or (for the few cursor-driven sites) the site's cursor predicate, each
  exactly as main computes it today.
- Migrate every production painter to name a `Surface` instead of a role,
  private role, or `resolve_surface_focus` call. No call site changes *which*
  bool it passes; only the colour *name* changes.
- Wire the two ast-grep guardrails (no role names or foreign resolvers in
  painters; no backgrounds filled from roles) with the rule text adjusted for
  the single bool-taking resolver.
- Port the surface-conformance test to the bool resolver, with expectations
  derived from the table.
- Retire value-aliased role names and split shared primitives (same bytes,
  purpose-named), as the archived change's rows 4.3 and 5.1 did.

Nothing rendered changes: not a value, not a focus behaviour, not a test
expectation. main's buffer tests pass byte-identical, and that is the
acceptance bar.

**Explicitly out of scope** (all visible, all deferred): any focus-semantics
change — no `FocusState`, no column-focus normalisation, no newly reactive
surfaces; the declined rework's behaviours (hero panes, gutters, selected row,
inset recesses); and the two highlight-gating fixes that exist only on the
declined branch (9bc41c92's ancestors a114ac17 / 763aca18), which are behaviour
fixes and follow-up candidates in their own right.

## Capabilities

### New Capabilities

- (none — the surface-table requirement lands in the existing design-language
  capability)

### Modified Capabilities

- `specs/ui-design-language/spec.md`: ADD the surface-table requirement — every
  painter names a `Surface` and resolves through the table; the resolver's
  focus input is the call site's own bit, unchanged from main's behaviour. The
  existing colour-role requirements are unchanged and continue to hold.

## Impact

- `src/app/render/theme/` (new table modules), `src/app/palette.rs` and
  `src/app/render/mod.rs` (re-exports, role retirements), and the ~66
  production paint sites across `src/app/render/` and `src/app/components/`.
- No shell, layout, sync, focus, or test-expectation changes. The archived
  change's `layout.rs` / `panel_focus_state.rs` work is deliberately absent.
- Guardrails: `rules/frontend-boundary/` + `rules/frontend-boundary-tests/`,
  ported with adjusted rule text.
- Reference material: `openspec/changes/archive/2026-09-11-unify-surface-colour/`
  (inventory, deviations, residuals) and PR #689 (the declined variant).
