# Design

## Context

See `proposal.md` for motivation and the two delta specs for behavior. The one-line `WideMediaList` owns media-specific `MediaListRow` data and delegates cursor/viewport/retained geometry to `src/app/components/list/`. The tree does the same with its own row model. F3 currently clones merged `PanelTarget` values into `SessionsComponent`, which owns numeric cursor/scroll and `HitRegions<usize>`; `render/components/sessions.rs` both lays out cards and returns index-bearing rectangles. `ShellRequest::SelectSession(usize)` indexes the latest `App.panel_targets` snapshot. The three-line card is 3 content rows plus 1 blank separator; a Cast target leaves its third content row empty.

## Goals / Non-Goals

**Goals:** One persistent, embedded, flat three-line control with the same shared movement/viewport/paint-retention mechanics as the other list shapes; F3 becomes its first consumer. Keep spacing cheap to adjust when longer lists are evaluated.

**Non-Goals:** A customizable card layout engine, redesign of F3's target information or sidebar chrome, multi-selection for F3, or changing discovery/control behavior. Do not alter the one-line media row or tree presentation just to support F3.

## Decisions

### D1: Add a three-line flat presentation against the existing list seam

Use a target-keyed three-line content model: three ordered lines of presentation text with a small closed semantic styling vocabulary (for example kind label/name, muted detail, playback status, optional aqua connection badge). Keep target identity and rendered content separate from Service objects. The embedded presentation owns its logical item flow and primitive selected-target/viewport state, using `RowFlow`, `Cursored`, `Viewported`, and `PaintRetained` rather than copying movement, clamping, or point-resolution algorithms. It implements plain TuiRealm `Component::view` and delegates painting to a shared Render Component. Its mounted parent remains the only `AppComponent` and event-to-`Msg` boundary.

A flow position denotes an *item*, not a terminal line: calculate visible item count from the content height and `stride = 3 + gap`, with a final item allowed when its three lines fit even if its separator does not. Feed that item count to the existing viewport seam. Paint geometry is three lines per fully visible item; publish only those completed item rects to the retained geometry carrier, and do not publish separators or partially visible cards. For height under three, show no selectable item until enough height returns. Recalculate the viewport when geometry changes, preserving the selected target and clamping scroll. The one-line `MediaListRow` carries media-specific kind/progress/gutter semantics and its `RowGeometry` assumes height one, so neither is repurposed for session cards.

Alternative rejected: projecting each card as three one-line `MediaListRow`s makes a single target selectable three times, breaks item-step navigation and striping, and invites duplicate cursor bookkeeping. A new standalone index-and-hit-map list repeats the seam and violates `shared-list-components`.

### D2: Treat each card as the stripe and selection unit

The painter applies the established selected-row bar to all three content lines for the focused cursor item; other items alternate zebra fill by stable flow position (first ungrouped item unstriped). Separator lines keep the sidebar surface fill and do not resolve clicks. Use a small gap setting on the presentation (F3 starts at one row), not a hard-coded blank fourth line inside each card or a new app-wide preference. If the width or height clips a card, do not publish an unpainted hit target. Existing list theme roles govern foreground/background; the aqua badge retains its accent even on the selected bar.

Alternative rejected: coloring the whole four-line stride makes the gap look like item content and changes pointer semantics when the spacing is adjusted.

### D3: Keep F3 data and effects at its existing boundary

`SessionsComponent` retains its owned `PanelTarget` snapshot solely to prepare item text and resolve kind-qualified identities, while the embedded list owns cursor, scroll and row hits. On content updates project `Emby(id)` or `Cast(id)` stable keys and three-line content into the list. Render the existing shell/header/footer and loading/empty messages outside the list; only the list paints populated card rows. The parent recognizes gestures, asks the list to resolve a point and perform selection/activation, and emits a target-bearing typed request on Enter or re-click. Leave current wheel-step, outside-click dismissal, refresh, F-key, and detach handling at the mounted parent. The shell looks up the key in its *current* `panel_targets`; absent key is a no-op. Do not put full `PanelTarget` payloads into requests: they may be stale by dispatch time.

Alternative rejected: retain `SelectSession(usize)` while adopting stable selection internally; asynchronous independent discovery can reorder the shell snapshot between click/Enter and dispatch. Keep the Emby/Cast discriminator in the key so equal ids from the two discovery channels cannot collide.

## Risks / Trade-offs

- Logical item counts differ from terminal-line counts, especially at heights 0-3 and a clipped final gap -> derive full-card capacity once, test short and scrolled viewports and scrollbar bounds at several heights.
- Moving session text styling into a generic painter could leak F3 concepts -> keep closed semantic spans/roles generic; F3 assembles its own labels and values, not a generic session-aware list.
- Existing F3 connected fill is used as a status cue -> focused buffer tests must show the new aqua badge both on and off the selected bar, with no legacy fill/rail.
- Duplicate target data in parent and child could become competing state -> parent retains only shell-owned snapshot and presentation projection; child exclusively owns cursor, viewport and completed hit geometry.

## Migration Plan

1. Introduce the three-line flat control and focused painter/seam tests without switching F3.
2. Convert F3 content projection, gesture delegation, and kind-qualified request/dispatch in one verified slice; delete its numeric cursor/scroll and per-row `HitRegions`, and the old populated-row painter. Preserve header/footer and empty/loading paths.
3. Verify mounted F3 tick/mouse behavior, focused buffer output at ordinary and narrow sizes, current-frame target resolution, full test/check/lint/format gates, and manually inspect a longer target list before accepting spacing. If manual density is poor, change the presentation's gap setting rather than its data model. Rollback is the single change revert; no persisted format or wire protocol changes.
