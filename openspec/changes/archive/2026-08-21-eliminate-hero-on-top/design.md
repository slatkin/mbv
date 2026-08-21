## Context

See `proposal.md` for motivation. Recent changes added a shared inline selected-detail flow for generic Emby lists and hero-on-left renderers for Movies and TV, joining the existing Home, grouped Music, and Audiobookshelf book wide compositions. The migration stopped at surface boundaries:

| Surface | Current non-wide path | Current wide path |
|---|---|---|
| Movies, TV, grouped Music | inline | hero-on-left |
| Emby podcasts and home videos | inline | generic inline/two-column fallthrough |
| Audiobookshelf podcasts | separate detail block | hero-on-left |
| Audiobookshelf books | separate detail block | hero-on-left |
| Feeds | separate detail block | separate detail block |
| Home | separate detail block | hero-on-left |

The generic inline implementation currently inserts placeholder detail rows after the selected display row and leaves the selected ordinary row behind. That creates a blank legacy row, double-counts the replacement in scrolling, and gives the wrong pointer owner. The replacement must be one flow segment: an ordinary row when unselected, or one variable-height hero block at the selected row. The hero block owns the selected item's row geometry and parent activation target; explicit episode, track, chapter, and selector targets remain more specific child targets. Visual framing is also coupled to the obsolete placement through placement-named vocabulary, so deleting only layout callers would leave the old concept embedded in the component API.

The pending `enforce-mbv-ui-design-system` change is deliberately paused until this behavior is settled. This change establishes the baseline but does not implement that later change's full component/arrangement ownership model.

## Goals / Non-Goals

**Goals:**

- Route every hero-bearing browse surface through one responsive placement decision.
- Reuse existing hero-on-left geometry and existing inline flow semantics.
- Preserve provider-native content, state, focus, child-row interaction, image behavior, and activation.
- Remove the obsolete top layout and placement-specific framing vocabulary completely.
- Leave a focused architectural decision and coherent live specifications for later UI enforcement.

**Non-Goals:**

- Building the complete screen-model/component/hit-map architecture planned by `enforce-mbv-ui-design-system`.
- Redesigning hero content, typography, artwork selection, metadata order, selectors, rows, or colors.
- Changing the breakpoint, minimum-height value, Panel mode, Service APIs, browsing state, or playback.
- Adding a preference or compatibility switch for hero placement.

## Decisions

### 1. Resolve placement from geometry, never from Service or surface

The shared responsive rule is:

```text
meets width breakpoint and minimum-height guard
                    │
             yes ───┴─── no
              │           │
       hero-on-left    inline detail
       one-column      one-column
       right rail      browser flow
```

The existing minimum-height guard remains. Failing it selects selected-row replacement even when width meets the breakpoint. It never selects a separate detail block.

Surface code may choose content and whether child detail is interactive, but it may not choose placement or define another responsive threshold.

**Alternative rejected:** Define wide strictly from width and force side panes at short heights. Existing left workspaces need a minimum usable height; removing that guard would make content inaccessible rather than standardizing it.

### 2. Treat inline detail as one selected-row flow segment

The selected media item is one variable-height display-flow segment: unselected items are ordinary rows, while the selected item is replaced by its hero block. Cursor identity remains media-item based. The replacement block owns the selected item's row geometry and parent activation target. A single click focuses and a double click performs normal item activation; explicit episode, track, chapter, and selector targets take precedence. Framing, artwork, and metadata use the parent target rather than creating duplicate targets.

Each custom browser—Home sections, Feeds groups, Audiobookshelf shows, and Audiobookshelf books—must adopt the same flow invariants as the current generic Emby row renderers: replacement at the active row, scrolling that keeps the variable-height segment addressable, and suppression only when the replacement cannot fit.

**Alternative rejected:** Overlay detail or keep a pinned narrow block. Both bypass list scrolling and retain the interaction ambiguity this change removes.

### 3. Reuse hero-on-left primitives without creating the later design system early

Wide surfaces compose the existing pane split, left surface treatment, right-rail chrome, and one-column browser geometry. The surface supplies its current hero/detail painter and browser rows:

- Home, Movies, home videos, and Feeds use read-only left heroes.
- TV, grouped Music, Emby podcasts, Audiobookshelf podcasts, and Audiobookshelf books retain their existing episode, track, or chapter interaction state in the left workspace.

Only the smallest common row-flow geometry needed to prevent duplicated breakpoint, pane, replacement, scroll, or hit-target arithmetic should be shared. A universal screen model, renderer callbacks, component registry, or new crate belongs to #563, not this prerequisite.

**Alternative rejected:** Patch each top call site independently. That would remove the visible symptom while preserving duplicated arrangement and hit geometry.

**Alternative rejected:** Implement #563's full closed UI framework now. That enlarges this behavioral prerequisite and risks formalizing assumptions before the final render tree is known.

### 4. Separate selected-detail framing from placement names

The existing selected-detail shell may remain visually unchanged, but its API and style vocabulary must describe framing or selection state, not hero position. Inline code must not call a placement-specific border variant. Once all callers use hero-on-left or selected-row replacement, remove the separate layout structure, helper, border variant, separate activation path, and stale comments/tests.

The `hero` component/module may remain; only the top arrangement is deleted.

**Alternative rejected:** Keep top symbols privately for shared borders. Dormant placement vocabulary would remain an attractive wrong path and would force #563 to account for a nonexistent arrangement.

### 5. Record removal as an architectural decision and domain migration

Add a presentation ADR stating that hero-on-left and selected-row replacement are the only supported placements, that short-height degradation selects replacement or restores the ordinary row, and that a separate top block is not a fallback. It should reject retaining the retired placement as compatibility behavior, paralleling ADR 0013's deletion of a redundant UI path.

Update `CONTEXT.md` in the same implementation change:

- Remove the retired separate-placement term.
- Define **Inline hero** as selected detail in narrow browser flow.
- Update **Hero-on-left** so it is the sole wide hero arrangement.
- Update **Panel mode** descriptions that currently permit top placement.

This is an explicit removal/rename of established domain vocabulary, approved by this proposal rather than an incidental terminology cleanup.

### 6. Verify the invariant as a closed inventory

Completion requires both positive and negative checks:

- Representative render tests for every surface family at wide, narrow, and width-wide/height-short dimensions.
- Row-map, replacement geometry, scroll, suppression, focus, parent activation, and child-target checks for custom browsers.
- Wide right-rail one-column and left-workspace checks.
- A repository search proving no production, test, live-spec, glossary, or current-ADR reference retains retired-placement terminology or symbols, excluding archived OpenSpec history.

Temporary visual captures may supplement focused `TestBackend` assertions but are not committed snapshots.

## Risks / Trade-offs

- [Risk] Home, Feeds, and Audiobookshelf use different row/group models, so copying the generic Emby algorithm could produce divergent scrolling or hit behavior. → Extract or reuse only the common display-flow accounting and test each custom browser's identities and non-item rows.
- [Risk] Audiobookshelf podcast/book inline detail contains interactive child rows unlike a read-only hero. → Preserve explicit child hit targets while keeping all remaining hero rows inert.
- [Risk] Wide conversion can move selectors into the wrong pane. → Keep browser-level pills/search in the right rail and selected-item filters/child selectors with the left workspace according to their owning state.
- [Risk] Removing top activation changes mouse behavior users relied on. → Make the media row the activation source in inline mode and retain only existing explicit child targets; verify keyboard and double-click playback paths from rows.
- [Risk] Broad terminology cleanup may accidentally alter archived history or unrelated "top" concepts. → Scope the negative check to current source, tests, live specs, glossary, and current ADRs; do not rewrite archived changes.
- [Trade-off] Surface painters remain partly bespoke until #563. → Accept that temporary boundary; do not add new local geometry or a speculative framework.

## Migration Plan

1. Add the ADR and update live delta-driven terminology/spec expectations so implementation has one invariant.
2. Establish placement-neutral selected-detail framing and reusable inline-flow/hero-left geometry seams without changing already-correct Movies, TV, or grouped Music output.
3. Convert Audiobookshelf podcasts, Audiobookshelf books narrow, Feeds, Home narrow, and Emby podcast/home-video wide paths one surface family at a time with focused tests.
4. Remove top layout, border variant, activation behavior, stale tests/comments, and all current terminology after the final caller is gone.
5. Run focused render/input tests, full package verification, lint, file-size checks, OpenSpec validation, and the closed-inventory search.
6. Re-read and revise `enforce-mbv-ui-design-system` against the resulting render tree before its implementation begins.

Rollback is a change-level revert. There is no persisted-data, protocol, configuration, or Service migration.
