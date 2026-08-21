## Context

Ratatui 0.30.2, single Rust binary, `TestBackend` for buffer tests. No separate UI
crate. Existing arrangement specs are `right-panel-arrangements`,
`library-list-hero`, and `ui-design-language`; this change extends them.

The tree as it stands: `src/app/render/` is 77 files / 23k lines (21k non-test),
`render/mod.rs` is a flat list of 44 modules, and nothing is labelled a screen or a
component. `hero.rs`, `list.rs`, `card.rs`, and `queue.rs` each compose *and* paint.
509 `palette::` references span 56 files. Mouse handling is 1,558 lines in
`src/app/input_mouse*.rs`, working from app state.

## Goals / Non-Goals

**Goals:**

- Give the ownership rules a module structure to attach to.
- Make the palette rule compiler-enforced rather than reviewer-enforced.
- Preserve existing visual output, proven per surface by a characterization test.
- Give agents a boundary they cannot each define differently.
- Ship in independently mergeable steps.

**Non-Goals:**

- Redesign any screen's visual presentation.
- Make mbv a general-purpose UI framework or support UI plugins.
- Migrate every surface before this change can close. The ledger tracks the
  remainder.
- Move application state or provider logic into the UI layer.
- Supersede `right-panel-arrangements`, `library-list-hero`, or
  `ui-design-language`.

## Decisions

### Screen / arrangement / component classification (step 1)

The split is decided by one criterion, applied per function, so two agents reach
the same answer:

| Takes | Is a | May |
|---|---|---|
| `&App` / app state, returns nothing | **screen** | call arrangements, build typed content models |
| typed content model + `Rect` + `&mut Buffer` | **component** | paint, compute its own geometry |
| typed content model + `Rect`, places components | **arrangement** | own breakpoints, split rects, place components |

A function that reads app state *and* paints is split at that seam; the state-reading
half stays in the screen module, the painting half moves. Screen modules end up with
no `use ratatui::` beyond re-export types, no `Layout::`, no `Rect` construction.

Target layout:

```text
src/app/render/screens/      -- app state in, typed models out
src/app/render/arrangements/ -- placement, breakpoints, rect splitting
src/app/render/components/   -- painting, self-owned geometry
src/app/render/theme/        -- roles public, primitives private
```

The move is mechanical and behaviour-preserving. Files near the 800-line cap
(`feeds.rs`, `mod.rs`, `audiobookshelf.rs`, `queue.rs`) split during the move.

This boundary is settled by the maintainer in step 1 and is not re-litigated by
per-surface work. Everything downstream depends on it, so nothing else starts
first.

### Palette: primitives private (step 2)

`src/app/palette.rs:51-69` currently defines roles as public aliases of public raw
constants in the same file:

```rust
pub const SURFACE_FOCUSED: Color = BG_GREEN;
pub const ACCENT: Color = AQUA;
```

Rewriting call sites to prefer the alias enforces nothing — `BG_GREEN` stays
reachable. Instead: primitives move to a private module inside `theme/`, roles
become the only public surface, and the compiler produces the call-site diff. Where
no role fits a site, a role is added; the added roles are the real output of this
step and are worth reviewing.

This is the one place where the rule needs no check, no lint, and no reviewer.

### Components own hit targets — pending a design gate (step 4)

Components deriving hit targets from their painted geometry is correct in principle.
It is not yet designed, because the consumer is not a screen:
`input_mouse_panels.rs` handles a mouse event on a later tick, from app state
(`row >= content_top`), not from render output.

Making this work requires a render→input channel with answers to:

- who stores the hit map between frames, and where
- what invalidates it (resize, tab switch, scroll, state change without repaint)
- what a mouse event does before the first paint, or after a resize but before the
  next paint
- whether components publish into a shared map or return one that arrangements
  aggregate upward

Step 4 produces those answers as a written design against the real files, then a
go/no-go. If it does not fall out cleanly, hit-target ownership is deferred and the
existing coordinate arithmetic stays — one consistent mechanism. A partial
migration leaving some surfaces on hit maps and the rest on coordinate math is the
worst outcome available and is explicitly out of bounds.

### Closed structural vocabulary, extensible content

Arrangements, components, and structural variants are closed and centrally defined.
Screen models stay extensible for titles, metadata, rows, and images.

An enum or sealed trait for a small closed variant set; policy constructors exposing
named valid combinations rather than public booleans. A registration-based extension
trait was rejected — it permits arbitrary Ratatui painting and restores the
convention-only failure mode.

Screens select a named policy or variant and supply semantic data. Overrides live in
the owning central component, arrangement, or theme.

### Hero additional-content styles

The hero has a closed family of approved styles already represented in the tree: the
Movie overview/detail block, the TV season/pill and episode workspace, the Music
track-list workspace, and the other provider-specific styles in use. Provider data
and row semantics stay extensible within a style; screens do not invent another one.

Each surface is mapped to its style as part of that surface's migration, not as an
up-front matrix — the mapping is only trustworthy once someone has read the surface
closely enough to move it.

Screens supply data and interaction state. The arrangement owns pane placement,
height budgeting, image/text stacking, optional-block placement, and responsive
presentation.

### Semantic theme API

Components consume semantic roles or component style policies. Screens do not pass
arbitrary `Color` or `Style` into shared components. Step 2 makes this structural.

### Enforcement, honestly

Three mechanisms, in descending order of strength:

1. **The compiler** — private palette primitives. Cannot be bypassed.
2. **ast-grep, path-scoped** — `use ratatui::`, `render_widget`, `Layout::`, and
   `Rect` construction inside `screens/`. Catches the common bypass; catches
   nothing subtler.
3. **Review, against the skill's checklist** — duplicated arrangement geometry,
   hit targets drifted from painting. These are the failures the change exists to
   prevent, and no static check finds them.

The earlier draft named the source check as *the* enforcement mechanism. It is not:
ast-grep can find `render_widget` under a path and no more. Step 1 exists precisely
so that mechanism 2 has a path to scope to, and mechanism 3 has a name for what it
is reviewing. Buffer tests verify component behaviour and preserved output; they do
not establish conformance.

### Migration gate: characterization test first

Buffer coverage is uneven. `list_tests.rs` (758 lines) and `movies_wide_tests.rs`
(359) pin the list/hero paths well. The 12 overlay files (~90KB) have essentially
none, and four surfaces queued for migration are overlays.

So each surface migration lands in two commits: the characterization `TestBackend`
test first, proving current output; then the migration, with that test unchanged.
Migrating an untested overlay is a blind refactor and "output preserved" is an
unverifiable claim there.

### Layered module boundary

```text
screens -> arrangements -> components -> Ratatui
                   \-> hit maps (step 4, if it clears)
```

Module visibility, private theme primitives, and typed component APIs are the
boundary. A separate crate is not required and would not close same-crate import
bypasses.

Centralisation means one authoritative owner per geometry, style, and interaction
concern — not one monolithic renderer. Arrangements and components stay distributed
and independently testable.

## Risks / Trade-offs

- [Risk] The step-1 module move is a large mechanical diff that could hide a
  behaviour change. -> Move only; no logic edits in the same commit. Existing
  render tests must pass unchanged.
- [Risk] Step 2 surfaces sites where no semantic role fits, tempting a
  pass-through alias. -> Adding a role is the correct answer and is reviewable;
  a `TEXT`-shaped escape hatch is not.
- [Risk] The ledger stops shrinking and the change stalls half-done. -> Accepted.
  Half-done here is a strictly better tree than today's, and enforcement still
  applies to everything anyone touches.
- [Risk] Closed vocabulary makes new UI work feel bottlenecked in one module.
  -> Keep content models extensible; keep named policy additions small.
- [Risk] Agent skills can be ignored or unavailable elsewhere. -> Non-negotiable
  rules in `AGENTS.md`; the skill carries workflow.
- [Risk] Ratatui's buffer escape hatch permits in-crate bypasses. -> Confined to
  `components/` by the ast-grep scope; visible in review.
