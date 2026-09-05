## Context

See proposal.md Why. Current state: `App::prewarm_grouped_music_album_images` (`src/app/render/components/music_wide.rs:213`) implements the intact ±3-ahead/±1-behind idle-gated loop over a caller-supplied `(albums, cursor, order)`. Its only call site (`src/app/shell_music_workspace.rs:205`) builds a fresh `wide_music_render_ctx` at render time and calls it only when `!wide && images_enabled`. The component paints from its cached pushed context plus live `album_cursor`, so the fresh render-time context's cursor/order can disagree with what is painted.

Constraints: prefetch is an `App`/image-cache effect and must stay shell-side (interactive-component boundary: no `App` in components, no effects in `view`). The per-frame render path is the established home for idle-gated image prefetch (`shell_browser.rs:461` precedent). `push_music_workspace_content` is event-scoped and must not host idle-gated per-frame work.

## Goals / Non-Goals

**Goals:**

- Restore neighbour prefetch for wide grouped Music at parity with `main`.
- Key the window off the painted cursor and display order in both presentations.

**Non-Goals:**

- No change to window size (±3/±1), idle-gate semantics, cache keys, fetch params, or image pipeline.
- No change to `Jump`-arm `last_nav_at` stamping (separable; noted in proposal discussion only).
- No viewport-scaled window tuning for the wide rail; restore parity first.

## Decisions

### 1. Drop the `!wide` gate; keep the call in `render_music_workspace_component`

The render path runs every frame in both presentations and is where the sibling `fetch_nearby_movie_posters` call lives. Alternative (prefetch at push time) rejected: push is event-scoped, so neighbours arriving after the idle delay opens would never warm. Alternative (prefetch inside `view`) rejected: components must not run `App` effects.

### 2. Warm from the painted cursor and order, read back post-`view`

The pre-view fresh context is rebuilt from `comp.album_cursor()` but its `album_order` comes from the currently settled catalog, which can lag the cached pushed context the component actually paints with. After `application.view(id, …)` the shell already reads back `take_image_paint` and layout; extend that read-back to the component's painted `(album_cursor, album_order)` and prewarm from those.

This needs a small accessor on `MusicWorkspaceComponent` exposing the painted cursor and the `album_order` of the context it painted with — read-only projection state, not a cursor mirror back into `App` (the component remains the cursor owner; the shell never writes through this path).

Alternative (keep warming from the pre-view fresh context, as `shell_browser.rs` does): accepted divergence in the generic browser, but for Music the painted order is cheaply available in the same function, so exactness costs nothing.

### 3. Skip prefetch while search is active

The wide search path paints a non-canonical grid and empties the rail control; warming `album_order` neighbours would fetch art for albums not on screen. Guard on the same `is_search_active()` predicate the painter uses. (Narrow gets the same guard; the current narrow call site lacks it.)

## Risks / Trade-offs

- [Risk] Post-`view` read-back adds a second borrow of the component per frame → Mitigation: extend the existing projection block that already downcasts post-`view`; no new borrow scope.
- [Risk] Wide rail shows more rows than ±3 covers; parity window may under-warm → Mitigation: restore ±3/±1 first, tune only with measured evidence (explicit non-goal).
- [Risk] Wide search-active guard diverges narrow/wide call shape → Mitigation: single shared guard on `ctx.list.is_search_active()` before either call.

## Migration Plan

No migration; behaviour-only restoration behind existing `images_enabled()` and idle gates. Rollback: re-add the `!wide` gate.

## Open Questions

None.
