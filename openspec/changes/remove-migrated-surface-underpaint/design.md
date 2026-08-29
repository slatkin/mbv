## Context

See proposal.md — Why. `migrate-tui-to-tuirealm` design D17 stage 5 ("detach
component geometry/content from legacy underpaint, then delete that surface's
legacy renderer") and D18 (Emby `wide_movies` two-stage resolution) govern this
work. D17's parity rule is binding: **current observable behaviour, including
current effect targets and current painter reachability, is authoritative**;
where the component and legacy path would paint differently, that is a blocking
discovery result, not license to pick the cleaner one.

Current draw path:

```
startup:      terminal.draw(|f| self.app.render(f))          x2   (shell_run.rs:32, :77)
steady state: terminal.draw(|f| {
                  self.app.dim_backdrop_active = self.blocking_overlay_active();
                  self.app.render(f);                    // (1) legacy: geometry + FULL surface paint
                  ...push_*_workspace_content on resize...
                  self.render_playback_component(f);     // (2) 11 component views paint on top
                  self.render_home_component(f);
                  ... 9 more ...
                  self.render_overlay_stack(f);
              })                                              (shell_run.rs:480-505)
```

`App::render` (`render/screens/root.rs:26`) writes a fresh local `AppLayout`,
calls `render_main(...)` which paints tab bar + player chrome + the
per-destination body + hero + status bar and fills `layout.main` / `layout.playback`
/ `layout.tabs_area`, then swaps `layout` into `self.layout` atomically. Every
`render_*_component` in step (2) then reads `self.app.layout.main.*` for its
rect. So step (1) is doing two jobs:

- **Geometry** — produce `AppLayout` (load-bearing; components depend on it).
- **Surface paint** — draw tab bar, status bar, player chrome, and each
  destination body. For a `migrated` destination the component in step (2)
  redraws that body; the legacy paint is dead pixels overwritten the same
  frame.

## Goals / Non-Goals

**Goals:**
- One painter per `migrated` surface per frame at its active breakpoint.
- The legacy surface painter is provably **unreached** for a suppressed
  surface — not just visually equivalent.
- Startup and steady-state draws go through one entry point and paint
  identically.
- `AppLayout` and all geometry facts components read are still produced every
  frame.

**Non-Goals:**
- Migrating the narrow-TV, narrow-Music, or album-track legacy painters into
  components. Those remain legacy renderers; this change makes them the
  **sole** painter for their breakpoint and records that, rather than an
  underpaint.
- Removing `dim_backdrop_active` (D16 permits this render-only adapter).
- Touching component interaction state, `App` fields, protocol, or persistence.
- Mouse (D16).
- Re-deriving Emby `wide_movies` breakpoint logic that the #611 browser change
  already owns — fold in only residue it left.

## Decisions

**D1 — Scout handoff is task group 1, blocking. No suppression before it lands.**

The handoff (`openspec/handoffs/scout-remove-migrated-surface-underpaint.md`)
must, per surface the legacy base frame paints, record: (a) the current painter
(legacy body / component view / both), (b) every `AppLayout` field the legacy
renderer produces and who reads it, (c) loading / image-prefetch / image-handoff
/ scroll-reconciliation / responsive-variant logic that exists only in the
legacy path, (d) whether a component is the active painter at each breakpoint or
a legacy variant is, (e) the smallest compile-complete suppression units in
dependency order. This mirrors D17's scout requirement and the
`scout-remove-browser-cursor-scroll-mirror.md` precedent.

**D2 — Stage the geometry/paint split by bounded surface family.**

The original one-shot row 2.1 was widened by explicit user approval after the
scout found 18 render modules and 110 layout assignments. Commit
`94002d25b75e0f34df200c3def57939c6cffd156` is a preparatory partial extraction:
it adds the seam but does not satisfy the geometry-only contract and must not
be treated as completion. The remaining work is staged so each family moves
its layout publication behind a paint-free seam, with focused compile and
characterization gates before the next family.

The family boundaries are: (A) root/chrome, (B) queue/pills, (C)
lists/albums, (D) feeds/home, (E) music surfaces, and (F) TV/widgets. A family
may touch only its named production modules and its tests; no family changes
interaction, visual design, or later suppression behavior.

**D2 — Split `App::render` into `compute_frame_layout` + `paint_legacy_chrome`."},{

`compute_frame_layout(&mut self, area) -> AppLayout` runs the existing geometry
math (the `Layout::vertical`/`Layout::horizontal` splits, breakpoint selection,
indicator rects, `layout.main` / `layout.playback` / `layout.tabs_area`) and
paints nothing. It still does the atomic `self.layout = layout` swap and the
early-return zero-area guard.

`paint_legacy_chrome(&mut self, f)` paints only what no component owns this
frame: tab bar, status bar, player chrome (until `PlaybackComponent` is the sole
painter — scout confirms), and each destination body whose component is **not**
the active painter at the current breakpoint (narrow-TV / narrow-Music /
album-track legacy variants, per-scout).

Rejected alternative: keep one `render` and pass a "components own X, Y, Z this
frame" mask. Rejected — the mask is just the breakpoint decision computed twice;
cleaner to make `render_main`'s per-destination dispatch return early for a
component-owned body.

**D3 — One Model draw entry point.**

`Model::draw_frame(&mut self, f)`:

```
self.app.dim_backdrop_active = self.blocking_overlay_active();
self.app.compute_frame_layout(f.area());   // geometry only
self.app.paint_legacy_chrome(f);           // legacy chrome + sole-legacy bodies
// resize content pushes (unchanged)
self.render_playback_component(f);
... the existing 10 component painters ...
self.render_overlay_stack(f);
```

Both startup `terminal.draw` calls and the steady-state one call
`|f| self.draw_frame(f)`. The startup draws currently call `self.app.render(f)`
directly (no component painters), so they paint a chrome-only first frame; after
this change they paint the real first frame including components, which is
strictly better (removes the startup flash where components appear on frame 2)
and must be covered by a test.

**D4 — Per-surface suppression proves execution ownership.**

For each `migrated` destination body the scout marks component-owned at a
breakpoint: make `render_main`'s dispatch arm for that destination+breakpoint
return before painting the body (still computing its geometry). Then prove the
legacy body painter is unreached — a debug assertion / test counter that fails
if the legacy arm paints while the component is the active target, run under the
existing render characterization tests for that surface. Buffer-diff tests stay
as a secondary check; the primary evidence is the painter never runs.

**D5 — Sole-legacy-for-breakpoint surfaces are recorded, not hidden.**

narrow-TV, narrow-Music, album-track: the component is not mounted / not the
painter at that breakpoint (`keep-destination-components-mounted` keeps it
mounted-but-inactive; it paints nothing — that change's D4). The legacy renderer
is the only painter, which is correct. The ledger Notes cell for the row states
"wide: component; narrow/<variant>: sole legacy renderer" so the endpoint is
explicit and #614 can certify it.

**D6 — Delete a legacy body renderer only after its last reader is re-homed.**

Some legacy renderers produce `AppLayout` fields a component reads (D18's
`movies_wide_right_area` → `is_wide_movies_active` → `set_wide_movies`). Order:
suppress the paint → confirm the only remaining reason the renderer runs is to
publish a geometry field → move that derivation into `compute_frame_layout` or
the component (per D18 step 2) → delete the renderer. If the #611 browser change
already did this for the Emby browser, this change only checks nothing regressed.

## Risks / Trade-offs

- [Risk] A component reads an `AppLayout` field that only the *painting* half of
  a legacy renderer sets (side-effect geometry), so moving to a paint-free
  geometry pass drops it.
  → Mitigation: D1 scout enumerates every `AppLayout` producer/reader before any
  split. D2's `compute_frame_layout` is extracted by moving the geometry
  statements, not rewriting them; a field that turns out to be set mid-paint is
  a blocking discovery result to hoist explicitly, per D17.
- [Risk] Image prefetch / handoff or scroll reconciliation is triggered as a
  side effect of the legacy body paint and silently stops when suppressed.
  → Mitigation: D1 scout item (c) targets exactly this; each suppression unit's
  test asserts the prefetch/handoff still fires (the `keep-destination-components-mounted`
  change already added content-refresh-on-re-point coverage to build on).
- [Risk] Startup frame now paints components; a component not yet content-pushed
  at startup paints an empty/placeholder body that the old chrome-only frame
  did not show.
  → Mitigation: startup already calls `fetch_home_at_startup` and sets
  `home_content.loading = true` before the second draw; extend the startup test
  to assert the first full frame shows loading affordances, not blank panes.
- [Risk] Scope creep — "one painter per surface" invites migrating the narrow
  variants.
  → Mitigation: D5 is explicit; narrow variants stay legacy, recorded as sole
  painter, out of scope.

## Migration Plan

1. Land the D1 scout handoff (read-only, no code).
2. D2/D3: extract `compute_frame_layout` + `paint_legacy_chrome`, add
   `Model::draw_frame`, route all three draw sites through it — behaviour
   identical (legacy still paints everything, components still overdraw).
   Full test suite + startup test green.
3. Per suppression unit from the scout's dependency order (D4): suppress one
   `migrated` body painter, add the execution-ownership check, retest that
   surface. One unit ≈ one surface × breakpoint.
4. D6: re-home any straggler geometry reader, delete the dead legacy renderer.
5. D5: ledger Notes updates; ADR 0022 wording checked (reconciled under #614).
6. Final gate: `rtk cargo check/nextest/clippy/fmt`, `rtk ast-grep scan`,
   `rtk make check-code-file-lines`.

Rollback is a plain revert per unit — no persisted state touched.

## Open Questions

- Is `PlaybackComponent` already the sole painter of the player chrome, or does
  `render_main` still paint it underneath? Resolved by the D1 scout before any
  split; does not change the approach (either way the chrome paint moves to
  `paint_legacy_chrome` or is suppressed), only the unit count.
