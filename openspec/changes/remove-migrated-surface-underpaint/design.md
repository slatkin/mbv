## Context

See proposal.md — Why. `migrate-tui-to-tuirealm` design D17 stage 5 ("detach
component geometry/content from legacy underpaint, then delete that surface's
legacy renderer") and D18 (Emby `wide_movies` two-stage resolution) govern this
work. D17's parity rule is binding: **current observable behaviour, including
current effect targets and current painter reachability, is authoritative**;
where the component and legacy path would paint differently, that is a blocking
discovery result, not license to pick the cleaner one.

The current draw path has three terminal draw sites. The steady-state site first
runs `App::render` (which computes `AppLayout` and paints the full legacy base
frame), then resize pushes, then mounted component views and the overlay stack.
The two startup sites currently call `App::render` only. Components consume the
installed layout, so layout publication is load-bearing even where legacy paint
is redundant.

## Goals / Non-Goals

**Goals:**
- One painter per `migrated` surface per frame at its active breakpoint.
- The legacy surface painter is provably **unreached** for a suppressed
  surface — not just visually equivalent.
- Startup and steady-state draws go through one entry point and paint
  identically.
- `AppLayout` and all geometry facts components read are still produced every
  frame, through ordered progressive checkpoints.

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

**D2 — Progressive geometry checkpoints, not one monolithic paint-free pass.**

Each frame starts a fresh draft `AppLayout`. The 2.1a root/chrome work is the
paint-free root/chrome checkpoint, not a claim that all geometry is available
before `render_main`. The base-frame orchestrator advances ordered checkpoints:
chrome result, progressive surface geometry checkpoints, and the sole legacy
paint, followed by mounted component views. Pure arrangement geometry is
published before its owning paint. Producers whose geometry is coupled to an
authoritative load or paint operation publish immediately after that operation.
Thus geometry-only is checkpoint-local, not a global property of the whole
frame.

Every checkpoint uses the existing zero-area-safe rules before mutation. A
checkpoint's result is merged into the fresh draft, and the completed draft is
atomically installed only after the frame's ordered work is complete. Each
field has one authoritative computation. Until its checkpoint lands, a deferred
field retains its existing legacy computation; no caller observes a partially
installed layout.

The dependency-first order is: root/chrome foundation; card; shared hero
arrangement; flat/letter lists; grouped albums; downstream queue and pills;
feeds/home; music; TV/widgets; then aggregate consolidation. Each family owns
only its named production modules and focused tests; it changes no interaction,
visual design, or later suppression behaviour.

**D3 — One base-frame orchestrator.**

`Model::draw_frame(&mut self, f)` is the sole draw entry point:

```
self.app.dim_backdrop_active = self.blocking_overlay_active();
let chrome = self.app.compute_frame_layout(f.area()); // 2.1a checkpoint
self.app.advance_geometry_checkpoints(chrome, f);     // ordered 2.1b–2.1j
self.app.paint_legacy_chrome(f);                      // sole legacy paint
// resize content pushes (unchanged)
self.render_playback_component(f);
... the existing mounted component views ...
self.render_overlay_stack(f);
```

The exact helper names may follow the landed seam, but there is one base-frame
orchestrator: chrome result + progressive checkpoints + sole legacy paint, then
mounted component views. All three terminal draws use it. Startup therefore
paints the same complete first frame and component loading affordances as the
steady state, without the old chrome-only flash.

**D4 — Per-surface suppression proves execution ownership.**

For each `migrated` destination body the scout marks component-owned at a
breakpoint: make `render_main`'s dispatch arm for that destination+breakpoint
return before painting the body (still computing its geometry). Then prove the
legacy body painter is unreached — a debug assertion / test counter that fails
if the legacy arm paints while the component is the active target, run under the
existing render characterization tests for that surface. Buffer-diff tests stay
as a secondary check; the primary evidence is the painter never runs.

**D5 — Sole-legacy-for-breakpoint surfaces are recorded, not hidden.**

narrow-TV, narrow-Music, album-track: the component is not the painter at that
breakpoint (`keep-destination-components-mounted` keeps it mounted-but-inactive;
it paints nothing — that change's D4). The legacy renderer is the only painter,
which is correct. The ledger Notes cell states "wide: component;
narrow/<variant>: sole legacy renderer" so the endpoint is explicit and #614 can
certify it.

**D6 — Delete a legacy body renderer only after its last reader is re-homed.**

Some legacy renderers produce `AppLayout` fields a component reads (D18's
`movies_wide_right_area` → `is_wide_movies_active` → `set_wide_movies`). Order:
suppress the paint → confirm the only remaining reason the renderer runs is to
publish a geometry field → move that derivation into the owning checkpoint or
component (per D18 step 2) → delete the renderer. If #611 already did this for
the Emby browser, this change only checks nothing regressed.

## Risks / Trade-offs

- **Side-effect geometry:** a load/paint-coupled producer could be moved too
  early. The scout inventory and checkpoint-local publication rule preserve the
  authoritative operation and its effects.
- **Partial drafts:** a component could read a field before its checkpoint.
  Fresh-frame drafts, dependency order, and atomic install prevent this.
- **Startup content:** a component may initially show loading/placeholder
  content. Startup already fetches Home and sets loading before the second draw;
  the startup characterization must assert loading affordances rather than blank
  panes.
- **Scope creep:** D5 keeps narrow variants as explicit sole legacy painters.

## Migration Plan

1. Land the D1 scout handoff (read-only, no code).
2. Land 2.1a's root/chrome checkpoint (complete/landed). It creates the fresh
   draft and typed chrome result while deferred families retain their existing
   authoritative computation.
3. Land 2.1b through 2.1j in dependency order. Each row publishes only its
   owning geometry at its natural checkpoint, retaining load/paint-coupled
   publication after the authoritative operation, and has focused gates.
4. Extract `paint_legacy_chrome` and add the sole `Model::draw_frame` entry
   point only after progressive geometry is consolidated; preserve legacy paint
   initially, resize pushes, and component order.
5. Per suppression unit from the scout's order (D4), suppress one migrated body
   painter and add the execution-ownership check. Preserve narrow sole-legacy
   variants.
6. D6: re-home straggler geometry readers and delete dead legacy renderers.
7. Update ledger notes and run final gates: check/nextest/clippy/fmt,
   ast-grep, code-file-lines, and strict OpenSpec validation.

Rollback is a plain revert per unit — no persisted state touched.

## Acceptance criteria

- Every frame starts a fresh draft `AppLayout`; root/chrome is the 2.1a
  paint-free checkpoint, later checkpoints are ordered, and installation is
  atomic with zero-area no-mutation preserved.
- Pure arrangement geometry is published before its owning paint; load/paint-
  coupled producers publish after their authoritative operation; geometry-only
  is checkpoint-local.
- One base-frame orchestrator runs chrome result, progressive checkpoints, the
  sole legacy paint, then mounted component views, and all three terminal draws
  use it.
- Each checkpoint has one authoritative computation and focused characterization
  coverage for loading, empty, populated, responsive, image/cache, and handoff
  states applicable to its family.
- 2.1b specifically preserves the card image cache `cache`/`size_for`/`fetch`
  single path across all rendering states; no state bypasses the authoritative
  operation.
- 2.1f consumes the card and pills checkpoints and does not recompute either
  geometry; its queue/pills output remains behaviour-preserving.
- Migrated surfaces have exactly one painter at their active breakpoint, with
  execution-ownership proof; narrow TV/Music and album-track remain explicitly
  sole legacy variants.
- Startup and steady-state frames are equivalent in draw sequencing, and the
  complete gate remains green: check, nextest, clippy, fmt, ast-grep,
  code-file-lines, and strict OpenSpec validation.
