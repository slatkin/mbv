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

### Components own hit targets — design gate resolved: no-go (step 4)

Components deriving hit targets from their painted geometry is correct in principle.
It was not designed, because the consumer is not a screen:
`input_mouse_panels.rs` handles a mouse event on a later tick, from app state
(`row >= content_top`), not from render output.

#### How each surface currently resolves a click (4.1)

Read directly from `src/app/input_mouse.rs`, `input_mouse_dispatch.rs`, and
`input_mouse_panels.rs` (1,558 lines total). Every surface hit-tests against
`self.layout` (`AppLayout`/`LayoutMain`), populated by the *previous* completed
render, except where noted:

| Surface | Mechanism |
|---|---|
| Tab bar | Arithmetic over `layout.tabs_area` + per-tab title widths, recomputed per click (`tab_idx_at`); no stored per-tab rect list |
| Playback controls, settings toggle, context-menu anchor | One stored `Rect` each (`layout.playback.*`, `layout.settings_area`, `layout.context_menu_rect`) |
| Seekbar | One stored `Rect` + fractional arithmetic for click-to-position |
| Queue panel | One stored `Rect` + `queue_row_map: Vec<Option<usize>>` parallel to displayed rows |
| Home tab | `home.hitmap: Vec<(Rect, usize)>` — explicit rect-per-card list |
| Feeds / Emby plain and letter-grouped lists | `left_item_rows`/`left_row_targets`/`left_row_map` (three fallback tiers) combined with column/cell arithmetic (`library_cell_width`/`library_column_count`) |
| Emby TV wide | `tv_wide_episode_rows`, `tv_wide_season_tabs` (explicit rect lists) + `left_row_map` reuse for the right-pane list |
| Emby Music wide | `wide_music_track_hitmap` (via `wide_music_track_at`), plus `selector_tabs`/`left_row_targets` |
| Audiobookshelf podcast / book | `audiobookshelf_episode_rows`, `audiobookshelf_book_chapter_rows` (explicit rect lists) + `selector_tabs`/`left_row_targets` |
| Breadcrumbs (Emby only) | `breadcrumbs: Vec<(u16, u16, u16, usize)>` — explicit per-crumb span list |
| Settings overlay | `settings_line_of_cursor: Vec<usize>`, a reverse line→cursor lookup |
| Sessions overlay | Fixed-stride arithmetic (`ENTRY_H = 4`); no stored per-row data at all |
| Playlists overlay | Re-walks `playlists_open_items` at click time, summing each item's 1-or-2-line height live, until the click line is reached; no stored geometry consulted |

The mechanisms are not one shape wearing different field names — they range from
explicit rect lists, to parallel index arrays, to pure scroll/stride arithmetic, to
(in the playlists case) a full live recomputation of variable-row-height layout
duplicated from the renderer.

That last case is a live instance of the exact risk this design exists to prevent:
`render_open_playlist_panel`'s `item_lines` closure (`screens/playlists.rs`) and
`handle_mouse_panels`'s click handler (`input_mouse_panels.rs`) each independently
compute `if label.len() <= text_w.saturating_sub(6) { 1 } else { 2 }`. Two copies of
the same formula, not one shared calculation — hit targets already drift from
painting in this codebase; it just hasn't broken yet because both copies still
agree.

#### Design questions (4.2)

- **Where is the hit map stored between frames, and by whom?** A single field on
  `App`: `self.layout: AppLayout`. `App::render` builds a fresh, local
  `AppLayout::default()` and threads `&mut layout.main` (etc.) top-down through the
  entire render call graph as an explicit out-parameter; every function that paints
  a surface also writes that surface's hit geometry into its named field on this
  shared struct, in the same call. Only once the full pass completes does `render`
  swap it into `self.layout` in one atomic assignment (`root.rs:60,178`).
- **What invalidates it?** The atomic per-frame rebuild is the only invalidation
  mechanism, and it is sufficient for every case checked:
  - *Resize*: `Event::Resize` sets `force_clear` and clears image caches; it does
    not rebuild layout itself. But the run loop (`app/mod.rs`) reads and processes
    exactly one terminal event per iteration, and `wants_terminal_render` returns
    true whenever an event was handled OR `force_clear` is set — so the very tick
    that processes the resize also renders and installs the new layout before the
    loop polls again. No mouse event can observe pre-resize geometry.
  - *Tab switch*: guarded explicitly, not by timing. `layout.main.browse_destination`
    tags the destination the installed frame was drawn for; `is_browse_layout_current`
    /`browse_mouse_ready` refuse to interpret browse-surface hit targets when the tag
    doesn't match the live tab (design §4, pre-existing). Queue-area and queue-scope
    clicks are explicitly exempted from this gate since they're drawn on every
    destination.
  - *Scroll*: no separate guard, and none is needed — the same single-event-per-tick,
    render-after-every-event loop means a scroll event is always followed by a
    render before the next input event can be read.
  - *State change without a repaint*: does not occur under the current loop, because
    every processed terminal event forces `wants_terminal_render` to true. A change
    driven by a background thread (e.g. a completed fetch) without an intervening
    terminal event could in principle lag on the slow (150ms/1s) idle cadence, but
    no mouse-affecting state is currently mutated that way outside the tab-switch
    case already covered by the tag.
- **What does a mouse event do before the first paint, or after a resize but before
  the next paint?** Before the first paint, `self.layout` is `AppLayout::default()`:
  every `Rect` is zero-area and every `Vec`/map is empty, so every `.contains()` and
  lookup naturally no-ops — no special-cased guard needed. After a resize, the
  single-event-per-tick guarantee above means there is no observable window where a
  mouse event sees pre-resize geometry.
- **Do components publish into a shared map, or return one that arrangements
  aggregate upward?** Neither, today. There is one centrally-defined struct
  (`LayoutMain`, ~25 fields), threaded by `&mut` reference through the whole
  top-down render call tree, and whichever function currently paints a given
  surface also writes that surface's named field directly, inline, as a side
  effect. Ownership of each field is documented by comment, not enforced by type.

#### Go/no-go (4.3): **no-go**

The existing mechanism above is not broken — every invalidation case checked
resolves correctly, and the one live drift risk found (playlists) is a
pre-existing, narrow bug, not evidence the architecture is unsound. But it is
screen-owned, not component-owned: it depends on a caller-supplied `&mut LayoutMain`
threaded alongside painting, with per-surface named fields. The step-1 component
signature (`model + Rect + &mut Buffer`, locked and non-negotiable) has no slot for
that — a component cannot write into a surface-specific named field on a shared
struct without either breaking its generic, reusable signature or requiring a new
generic hit-map type that has not been designed against any of the thirteen
structurally different mechanisms in the table above (explicit rect lists, parallel
index arrays, pure stride arithmetic, and live variable-height recomputation all
appear, not one shape).

Unlike step 5's per-surface painting migration, task 4.4 forecloses an incremental
path here: hit-map ownership must migrate every mouse-handling surface in one PR or
not at all. Committing to that now would mean designing and validating a new,
generic, typed contract against all thirteen mechanisms above with no fallback
ledger to de-risk it one surface at a time — a materially larger jump in scope and
risk than any other step in this change, for a mechanism that is not currently
broken.

**Decision**: defer. The existing coordinate arithmetic in `input_mouse*.rs` stays
as the one consistent mechanism. The hit-target-ownership requirement is dropped
from the `ui-design-system` delta spec; the actual spec edit is made in step 6.3,
which reconciles that spec with this outcome. The playlists duplication found above
is left as a narrow, surface-local fix for that surface's own step-5 migration
(ledger row 10), not a reason to revisit this decision.

#### No partial migration (4.4)

Satisfied by the no-go: nothing migrates to hit-map ownership, so no surface can end
up half-migrated relative to another.

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
