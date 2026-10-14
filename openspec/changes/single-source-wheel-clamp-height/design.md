# Design

## Context

See `proposal.md` for the problem. Current state, checked in the code:

- `PaintRetainedState` (`list/paint.rs`) holds one `Option<CompletedPaint>`. `begin()` and `invalidate()` set it to `None`. `content_rect()` therefore returns `None` after any invalidation.
- `ThreeLineFlatList` calls `invalidate()` after every `scroll_viewport`, `move_selection`, `select_target` and `set_content`. A second wheel notch before the next paint would find no retained rectangle.
- `WideMediaList::delegate_operation` does not invalidate paint (comment in `media_list/wide.rs`). Its retained rectangle stays valid across wheel notches. It forwards to `MediaList::delegate_operation`, whose `Scroll` arm calls `scroll_wheel`. That arm clamps with `viewport_len = 1` (#926 made it length-only). The core owner has no geometry.
- `TreeBrowser::viewport_height` returns the panel-declared content height (`configured_geometry`), else the area of the last `view` (`last_painted`).
- `SessionsComponent.painted_content_area` is written in `view` and read by the wheel arm and by `content_area_for_test`.

## Goals / Non-Goals

**Goals:**

- One place stores the height that wheel clamps use. Every list surface reads that place.
- A wheel at the end of any painted list stops with the last row at the bottom of the viewport.
- Drop `ThreeLineFlatList::scroll_viewport`'s height argument, `SessionsComponent.painted_content_area`, and `TreeBrowser.last_painted`.

**Non-Goals:**

- No change to the wheel step (three rows), anchor rules, selection rules, or paging.
- No change to how `resolved_viewport_offset` corrects an offset at paint time. That correction stays as the safety net.
- No change to `TreeBrowser`'s precedence of declared geometry over painted geometry.

## Decisions

### D1: The single source is a last-completed content rectangle in `PaintRetainedState`

Answers #928 question 1: yes, the retained paint state is the single source, with one correction.

`content_rect()` cannot be the source. It is `None` after `invalidate()`, and `ThreeLineFlatList` invalidates on each notch. A burst of two notches in one frame would clamp the second at the one-row minimum. That regresses `Sessions`, which survives this today because its own field is never cleared.

So add `last_content_rect: Option<Rect>` to `PaintRetainedState`. `store_completed` sets it. `begin` and `invalidate` leave it alone. It is the height of the last painted frame. Next paint replaces it. `content_rect()` keeps its "current completed frame only" meaning for its three existing callers (reach index, TV viewport height, wide accessor).

The type enforces the rule: a shape cannot clear the height by invalidating, because invalidation never touches the field.

Alternative: keep each surface's own field and only align the call shapes. Rejected: it keeps four copies of one fact.

Alternative: make `begin`/`invalidate` keep `content_rect`. Rejected: `content_rect()` callers rely on `None` meaning "not painted this frame" (`last_painted_reach_index`), and an existing test in `paint.rs` asserts it.

### D2: Fallback when no frame has painted is the one-row minimum

`None` maps to a viewport of one row (`.max(1)` as today). Wheel claims need a painted surface, so this case only arises in tests and in the first frame. No new rule.

### D3: `MediaList` takes the viewport length from its presentation

Answers #928 question 2. The one-row clamp protected one thing: the canonical `MediaList` owner has no painted geometry, so it had no height to use. The next paint's `resolved_viewport_offset` clamps the offset to `len - height`. The snap-back is visible: at the end of the list a notch can leave the last row at the top of the viewport, and the next frame moves it.

Change: the core's operation entry takes the viewport length. The core's own public `delegate_operation` keeps calling it with `1`, so the owner-only tests and the #926 "never builds the row flow" test stay valid. `WideMediaList::delegate_operation` calls it with `paint.last_content_rect()` height (or `1`). The arithmetic stays length-only (`scrolled_viewport_offset`), so #926 is not undone.

`MediaListCarrier` and every destination reach the list through `WideMediaList`, so every `MediaList` surface gets the fix with no per-destination edit.

### D4: `ThreeLineFlatList` reads its own retained height

Answers #928 question 3. `scroll_viewport(delta)` drops `painted_height`. It reads `paint.last_content_rect()` and keeps `visible_items(height).max(1)`. `SessionsComponent` calls `scroll_viewport(delta)`.

### D5: `SessionsComponent` loses `painted_content_area`

Its only readers are the wheel arm and `content_area_for_test`. The test accessor returns the list's `last_content_rect()`. `painted_panel_area` stays: hit testing needs the panel rectangle, which is a different fact.

One difference: `view` wrote `painted_content_area` even when `has_content` is false, and the list did not paint then. In that state the list is empty and a wheel has nothing to scroll. No behavior depends on it.

### D6: `TreeBrowser` drops `last_painted`

`last_painted` stores the `area` that `view` received. When `configured_geometry` is `None`, `view` uses `(area, area)`, so that area equals the content rectangle the paint stores. `viewport_height` becomes `configured_geometry` content height, else `paint.last_content_rect()` height. Precedence is unchanged.

The first frame reconciles its viewport against the content area `view` is painting in that frame, passed in as a value. The retained rectangle serves calls between frames.

## Risks / Trade-offs

- [A resize between paint and wheel clamps with the old height for one frame] → Same as today for `TreeBrowser` and `Sessions`. The paint-time `resolved_viewport_offset` corrects it. `MediaList` goes from "always one row" to "last painted height", which is closer to correct in every case.
- [`PaintRetainedState` holds two content rectangles with different lifetimes] → Documented on both accessors. A single rectangle cannot serve both meanings (D1, rejected alternatives).
- [Empty or zero-size paint] → `ThreeLineFlatList::view_in` returns before `store_completed` when `content_rect` is empty. The last non-empty height stays. A zero-height rectangle gives `.max(1)`.
