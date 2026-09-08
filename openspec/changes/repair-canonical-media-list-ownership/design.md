## Context

See `proposal.md` for motivation. The source already has `ListCore`, `WideMediaList`, `InlineMediaBrowser`, stable opaque row targets, `ViewportAnchor`, and canonical row painters. The gap is composition: the controls are helper structs rather than TuiRealm `Component`s; Home and Feeds mutate Wide and Inline controls together; Browser, Music, TV, and Audiobookshelf retain parent selection/scroll state around canonical controls; and the Audiobookshelf Wide rail constructs a control inside render functions.

The destination remains the mounted `AppComponent`, subscription owner, raw-event boundary, and provider workspace owner. The accepted #674 wheel behavior is treated as landed: eligibility follows the latest painted surface under the pointer, losing handlers do not mutate, and each accepted wheel gesture moves one logical row under the parent's existing throttle.

## Goals / Non-Goals

**Goals:**

- Make each canonical list a real persistent embedded TuiRealm `Component` with one live-state and row-painting authority.
- Keep responsive handoff explicit, stable-target based, and limited to the transition event.
- Remove parent mirrors and render-time controls without changing provider effects, list content, visual policy, or shell-owned persistence.
- Make future ownership regressions fail through narrow structural and behavioral checks.

**Non-Goals:**

- Migrating the non-hero two-column Emby grid, Inline Search, Search sidebar, Playlists/open-playlist rows, Settings, or Sessions.
- Mounting embedded controls, assigning them identities, or giving them subscriptions or gesture recognizers.
- Changing layout breakpoints, canonical queue authority, playback, Service behavior, or #674 wheel semantics.
- Adding multi-select.

## Decisions

### D1. Implement the existing TuiRealm component contract on canonical controls

`WideMediaList<Target>` and `InlineMediaBrowser<Target>` will implement the plain TuiRealm `Component` contract already used by mounted destinations. Each control will hold the semantic view inputs needed by its row painter, and its `view` will be the sole entry point for ordinary list-row painting. The parent will invoke that view directly inside its allocated list rectangle; it will not register the child with `Application`.

`InlineMediaBrowser` will paint the ordinary/replacement flow and expose the admitted detail rectangle. The destination parent will paint its provider-owned hero payload into that reserved rectangle. `WideMediaList` will expose only its own paint result and row geometry. Both controls will continue to resolve pointer positions from the same flow they paint, while the parent retains `MouseGestureState` and translates stable targets into typed requests.

Alternative: keep the controls as helper structs and call render functions from parents. Rejected because it preserves the current split between state/geometry helpers and the framework's rendering boundary.

### D2. Configure views with semantic policy, not provider data or callbacks

The controls will receive canonical rows plus a small closed semantic paint policy: parent focus, selected-row treatment, optional throbber, and, for Inline, desired detail height. They will not receive provider clients, raw styles, callbacks, images, hero content, or effects. Existing render-layer row primitives remain the visual substrate and move behind the controls' `view` methods rather than being duplicated.

Alternative: pass a closure that paints provider detail through the child. Rejected because it obscures paint ownership and introduces callback state into the provider-neutral model.

### D3. One active control; one transition anchor

A destination that owns both Wide and Inline controls will track which presentation was last painted. Local key and wheel commands go only to the control for the current presentation. On a real Wide↔Normal transition, the parent obtains one `ViewportAnchor` from the outgoing control and applies it once to the incoming control before paint. Ordinary content pushes may refresh both controls' rows so either is ready, but each control preserves/clamps its own target; no cursor or scroll is copied between them.

The parent must not discover a transition by writing paint-resolved cursor/scroll into shared fields. It may retain only the last presentation discriminator and a one-shot pending anchor.

Alternative: continue lockstep mutation so both controls are always aligned. Rejected because two controls become simultaneous owners and an inactive control changes without being painted.

### D4. Isolate the non-hero grid instead of widening this migration

Browser's generic parent-level `cursor`/`scroll` fields currently serve both canonical hero paths and the accepted non-hero two-column grid. Replace them with an explicitly named grid-only state used solely by the two-column path. Canonical Browser paths resolve selection, movement, activation, persistence values, and responsive anchors through `wide_list` or `inline_browser`.

Alternative: migrate the two-column grid to canonical controls now. Rejected by the selected minimum scope and because a fixed-row one-column control does not express the grid's column stride.

### D5. Correct each destination at its narrowest authority seam

- **Home and Feeds:** retain both persistent controls, stop lockstep movement/selection, delegate to the active control, and use one transition anchor.
- **Browser:** move canonical cursor/scroll reads and writes into the active control; keep only isolated grid state and discrete resting-position re-anchor input.
- **TV:** remove the parent series-cursor shadow and cursor-based mouse fallback; resolve the series target and ordinal from its persistent list. Keep season/episode workspace state in TV.
- **Music:** remove parent album cursor/scroll and painter writeback; make persistent Wide/Inline album controls authoritative. Keep track focus and the existing track control/workspace in Music.
- **Audiobookshelf Podcast and Book:** add persistent Wide show/book controls to the destination, project rows outside rendering, and remove `state.selected_id`/numeric browser offset as live list authority. Keep episode/chapter/filter/bucket workspace state in the parent.
- **Queue:** preserve the existing persistent fixed-row control and remove any remaining parent compatibility geometry/state now made redundant by the component view.

All shell requests that cross authority carry the control-resolved stable target or resting position. The shell does not replay a movement delta.

Alternative: create one generic destination wrapper to perform all composition. Rejected because existing parents already own provider-specific workspaces and typed requests; a wrapper would add indirection without removing authority.

### D6. Preserve painting and mouse behavior while deleting compatibility output

The existing canonical row painters remain, but callers stop receiving compatibility `left_item_rows`/`left_row_map` output once no production consumer needs it. Selected-row/detail geometry remains available from the control for parent-owned context menus and hero payload painting. Pointer resolution continues through `resolve_point` (or a stable-target equivalent); parent-owned pills, Queue scope controls, TV season pills, and provider workspaces keep separate irregular hit regions.

The #674 eligibility/arbitration and one-row wheel contract is a hard regression boundary. This change may reroute a wheel command from parent fields to the active embedded control, but it must not add a shell raw-delta relay, alter throttle timing, or let an unpainted control move.

Alternative: preserve compatibility maps indefinitely. Rejected because they duplicate the control's row flow and make a second hit-geometry owner possible.

### D7. Ratchet only the violations that structural checks can prove

Add architecture rules that reject production `WideMediaList::new()` or `InlineMediaBrowser::new()` beneath `src/app/render/**`, and known forbidden mirror/writeback patterns removed by this change. Add a compile-time trait-bound assertion for both canonical controls because an AST absence rule cannot reliably prove trait implementation. Extend existing tests rather than creating broad parallel suites:

- one control-level state test for active-only movement plus stable-target/offset handoff;
- focused destination tests for the real mirror bugs (Home/Feeds lockstep, Browser grid isolation, Music/TV/Audiobookshelf authority);
- existing buffer/conformance tests to prove one painter and unchanged representative output at Normal and Wide presentations;
- existing `Application::tick()` mouse tests to prove painted-owner eligibility and one-row wheel behavior through the shell synchronization pass.

Do not add geometry snapshot tests or duplicate tests already stronger than the proposed assertion.

## Risks / Trade-offs

- **[Parent detail painting can overlap the Inline row flow]** → The child returns one admitted detail rectangle, and existing buffer tests assert representative content and one-painter behavior.
- **[A kept-mounted inactive control has stale rows]** → Content pushes refresh both controls, but never copy interaction state; the transition anchor chooses the incoming selection explicitly.
- **[Removing parent numeric cursors can disrupt persistence/effects]** → Convert callers to stable targets or event-time resting positions before deleting each source field, then use exhaustive request matching and focused tests.
- **[The #674 branch differs when it lands]** → Rebase the implementation on its accepted HEAD first and treat its tests/spec as source constraints; do not recreate its arbitration.
- **[The migration touches many destinations]** → Change source-of-truth controls first, then migrate one destination family at a time while keeping each family compiling and tested.

## Migration Plan

1. Start from the accepted #674 HEAD and characterize any changed control API or wheel path before editing.
2. Add component-view support and remove compatibility paint output only after all consumers have replacements.
3. Migrate responsive shared parents, then Browser/TV/Music, then Audiobookshelf and Queue, converting callers before deleting each mirror.
4. Update architecture rules, `CONTEXT.md`, comments, and the interactive-surface ledger to match the final ownership.
5. Run focused tests per family, then the required package, formatting, clippy, architecture, and file-size gates. A human verifies representative Normal/Wide/sidebar behavior and #674 wheel behavior before acceptance.
6. Sync the delta specs into the main specs and archive only after implementation and human acceptance.

Rollback is a normal commit revert; there is no persisted-data or wire-format migration.
