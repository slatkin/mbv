## Context

See `proposal.md` and the `mouse-input` delta. `MouseGestureState` already owns the 30 ms throttle and normalizes terminal wheel input to a signed one-notched `MouseGesture::Scroll` delta. The divergence begins after recognition: components multiply that delta, page by layout-derived values, use stale/ad-hoc rects, or forward local movement through shell-only relay requests.

Canonical media lists deliberately own point resolution while their destination components own cursor and viewport state. Some surfaces (text sidebars and the Audiobookshelf irregular row layouts) do not embed a persistent canonical control, so their painter-published geometry is the authoritative equivalent.

## Goals / Non-Goals

**Goals:**

- Make the existing normalized gesture delta the one shared wheel unit for every scrollable surface.
- Keep wheel mutation and hit testing with the component that owns the relevant local state.
- Remove the Home, Browser, and TV relays and the shell Queue wheel mover without changing unrelated click, keyboard, persistence, or playback behavior.
- Cover each affected surface, its painted scroll region, and each applicable breakpoint with focused component or tick-harness tests.

**Non-Goals:**

- Change `WHEEL_THROTTLE`, click/double-click/right-click behavior, keyboard paging, list layout, or scroll policy configuration.
- Replace irregular-row Audiobookshelf layouts with canonical media-list controls.
- Add a second mouse router, shell geometry reads, or a generic state abstraction that would mirror component-local cursor/viewport state.

## Decisions

### D1: Reuse the normalized gesture delta as the sole policy unit

Each recognized `MouseGesture::Scroll` already carries `-1` for up and `+1` for down. Every affected arm will consume that value directly for one local row or viewport-line move; multipliers (`* 3`, column/page strides) and wheel-specific page behavior are removed.

No new policy type or configuration is introduced. A wrapper around a signed one-step delta would duplicate the existing gesture boundary without removing any caller-specific state transition.

Alternative considered: a configurable shared helper with per-surface step sizes. Rejected because the chosen behavior has no overrides, and retaining them would recreate the inconsistency this change removes.

### D2: Claim the painted scroll region at the component boundary

A destination using `WideMediaList` or `InlineMediaBrowser` will use that embedded control's point-resolution/claim path before applying its one-step local move. Text viewports and Audiobookshelf irregular layouts use their painter-published region or row geometry when they compete with another eligible surface. A focused sidebar that is the sole eligible overlay accepts wheel independently of pointer position; click and row activation remain geometry-resolved.

Inline Search continues to take first refusal where it overlays its host list; its currently ignored wheel gesture becomes a one-step move within its own results viewport if it is scrollable.

Alternative considered: a shared global hit map or a component-wide `Rect::contains` convention. Rejected by ADR 0024 and the mouse-input contract: eligibility and geometry ownership are local to the component that painted the surface.

### D3: Apply movement locally and preserve only semantic effect messages

Home, Browser, TV, and Queue will update their component-owned cursors/viewports directly. `HomeScroll`, `BrowserScroll`, and `TvScroll`, their shell dispatch arms, and the shell Queue mover are deleted. Browser will use its existing resolved cursor/persistence request path where a library position must be saved, rather than forwarding a viewport offset for the shell to interpret.

Components retain existing typed messages only when a resolved index or selection must cross the ownership boundary for navigation, persistence, or an external effect. The receiver consumes that resolved value and never reapplies the wheel delta.

Alternative considered: retaining no-op relay requests as refresh signals. Rejected because component updates already repaint local state and relays obscure which side owns the movement.

### D4: Change every current wheel consumer in one bounded sweep

The implementation sweep covers Browser (including narrow TV ownership), Home, Queue, TV workspace, Music workspace, Feeds, Audiobookshelf podcast and books, Settings, Help, Sessions, Playlists, and Inline Search. It also updates any shared list utility required to make canonical-control claims available without duplicating geometry. `MouseGestureState` remains the sole throttle/direction producer.

The closed reference commit `e32f2858` may inform Home and Browser's local-first rewrites, but it is not applied wholesale: its Music page movement and retained Browser shell offset path conflict with the one-step policy.

## Risks / Trade-offs

- [A list claim API cannot represent a surface's current painted region] → extend the existing embedded control's public-to-parent claim API minimally; do not add a parallel rect registry.
- [A local move stops a required persistence update] → trace each removed request's consumers first; retain or reuse the existing resolved-value request only at the identified persistence/effect seam.
- [Wide and narrow use different mounted owners] → test both mounted components through the shell tick path where the breakpoint changes destination ownership.
- [Scroll lands at an empty or boundary state] → use each component's existing clamping/move routine and assert no invalid cursor/offset or message is produced.

## Migration Plan

1. Add focused characterization tests that expose current multiplier/page/relay behavior before changing the affected components.
2. Make the shared gesture semantics explicit through the component/list claim seam, then migrate the affected components in ownership-compatible groups.
3. Delete the obsolete requests, handlers, and tests only after their replacement local behavior and any required resolved-value persistence path are proven.
4. Update the mouse specification delta and ledger alongside the implementation; there is no data migration or compatibility release step.
5. Roll back by reverting the implementation commit(s); this change has no persisted format, protocol, or configuration change.
