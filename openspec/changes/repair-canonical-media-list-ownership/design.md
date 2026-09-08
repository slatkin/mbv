# Repair Canonical Media-List Ownership Design

## Context

See `proposal.md` for motivation. Landed PR #683 (`6b58a608`) supplies the painted-owner mouse arbitration and one-row wheel baseline. The source has `ListCore`, `WideMediaList`, `InlineMediaBrowser`, `ViewportAnchor`, and canonical row painters, but the controls remain helper-like: their painters accept separate outer and content rectangles and return geometry; Browser rows target volatile indices; Music and TV projections retain position; and Audiobookshelf still seeds selection and paints list controls through render helpers.

The destination remains the mounted `AppComponent`, subscription owner, raw-event boundary, and provider-workspace owner. The accepted non-hero two-column Emby grid is not a canonical list presentation. TV episode and Audiobookshelf episode/chapter panes are provider workspaces, not destination rails in this migration.

## Goals / Non-Goals

**Goals:**

- Make each destination rail/list a real persistent embedded TuiRealm `Component` with one live-state and ordinary-row-painting authority.
- Give the constrained TuiRealm `Component::view(Frame, Rect) -> ()` API a closed configuration/result protocol that retains one frame's list geometry without a second paint or parent geometry reconstruction.
- Make canonical row targets stable through refresh, reorder, and responsive handoff; remove position from ordinary content projection.
- Keep the non-hero grid and provider workspaces explicit, isolated exceptions rather than accidental alternate owners.

**Non-Goals:**

- Migrating the non-hero two-column Emby grid, Inline Search, Search sidebar, Playlists/open-playlist rows, Settings, or Sessions.
- Mounting embedded controls, assigning identities, or giving them subscriptions or gesture recognizers.
- Migrating TV episode or Audiobookshelf episode/chapter workspaces into destination rails, changing their typed activation/seek intent, changing layout breakpoints, queue authority, playback, Service behavior, or #683 wheel semantics.
- Adding multi-select.

## Decisions

### D1. Embedded views use stored closed paint configuration and retained results

Before one child `Component::view(frame, outer_paint_area)` call per frame, the parent supplies a closed `MediaListPaintPolicy`: focus state, selected-row treatment, throbber, desired Inline detail rows, and the named content inset policy. The child derives its content area from the outer area and policy, paints all ordinary rows once, then retains `MediaListPaintResult` for that frame. A Wide result exposes row geometry and selected-row rect; an Inline result additionally exposes its admitted detail rect. The parent reads the retained result after `view` to paint provider detail or resolve a recognized point; it neither passes a second rect pair nor recomputes row flow or invokes a second painter.

This accommodates TuiRealm 4.1's `view` return type of `()` while making outer paint and inner content ownership explicit. Provider data, callbacks, raw styles, images, and effects remain outside the policy.

Alternative: retain free render functions with separate `paint_area`/`content_area` arguments. Rejected because it leaves the API's geometry/result ownership ambiguous and makes a second painter easy.

### D2. One active control; one transition anchor

A destination with both Wide and Inline controls tracks the last painted presentation and a one-shot pending `ViewportAnchor`. Local key and wheel commands go only to the current presentation. On a real Wide↔Normal transition, the parent obtains one anchor from the outgoing control and applies it once to the incoming control before paint. Ordinary content pushes may refresh both controls' position-free rows, but each control preserves or clamps its own stable target; no cursor or scroll is copied between them.

Alternative: lockstep mutation. Rejected because an inactive control becomes a simultaneous state owner.

### D3. Canonical rows carry identity; content snapshots carry no position

Browser canonical rows use stable Emby identity rather than a sorted/content index. Browser maps that target to its current content only at the effect, persistence, context-menu, or navigation boundary. Home rows use `QueueItemContentId` (the provider-qualified QueueItem content identity), not `QueueItem::id()`, so equal native ids from different Services cannot collide.

`MusicContent`, `TvContent`, and analogous Audiobookshelf browse-content projections exclude cursor and scroll. A separately named discrete resting-position/re-anchor input carries any shell-owned restore value. A source ratchet and focused tests reject cursor/scroll in ordinary content pushes, rather than merely ignoring them.

Alternative: retain numeric indices in canonical controls or position fields in snapshots. Rejected because refresh/reorder and a shell projection can silently retarget a live selection.

### D4. Browser grid state and geometry are isolated

The non-hero two-column Browser path owns `BrowserGridState` and `BrowserGridGeometry`, including its cursor, scroll, columns, row maps, and hit mapping. Only the grid arrangement can read or write those types. Canonical Browser paths derive selection, movement, persistence, effects, and geometry from their active control and stable Emby target. Structural ratchets exempt this named grid path only.

Alternative: migrate the grid. Rejected by the selected minimum scope and because a fixed-row one-column control does not express its column stride.

### D5. Destination rails and provider workspaces have separate seams

- **Home and Feeds:** retain both persistent controls, stop lockstep movement/selection, and use one transition anchor.
- **Browser:** remove canonical cursor/scroll and index target use; retain only isolated grid state/geometry and a discrete resting re-anchor input.
- **TV and Music:** project position-free series/album content to persistent controls; resolve effects/persistence through stable targets. TV seasons/episodes and Music track focus remain parent workspace state.
- **Audiobookshelf Podcast and Book:** project show/book rows during content updates; remove render-time rail construction, seeded selection, and parent list offsets. Podcast episode and Book chapter state remain parent workspaces.
- **TV episode and Audiobookshelf episode/chapter panes:** are explicit parent-owned workspace exemptions from destination-rail constructor/mirror/paint ratchets. Their parents retain typed episode activation and book chapter seek targets; the exemption does not permit duplicate row geometry for a destination rail.
- **Queue:** retains its existing persistent fixed-row control and removes only compatibility geometry/state superseded by the child view.

Alternative: add a generic destination wrapper or broaden the migration to each provider workspace. Rejected because it adds indirection or scope without repairing the rail ownership defect.

### D6. Delete compatibility output after its last canonical consumer

The controls retain only their per-frame results. Canonical callers stop receiving `left_item_rows`, `left_row_map`, render-time constructors, and ordinary-render position writeback. Parent-owned pills, Queue scope controls, TV seasons, and provider workspace regions retain their own paint-time geometry. The #683 eligibility/arbitration and one-row wheel contract is unchanged; rerouting a wheel action may only delegate it to the active embedded control.

### D7. Ratchet concrete regressions, not named exemptions

Add rules for production canonical-control construction under `src/app/render/**`, parent cursor/scroll mirrors, paint writeback, and position-bearing ordinary content snapshots. The rules permit the named `BrowserGridState`/`BrowserGridGeometry` path and explicit TV/Audiobookshelf workspace seams only. Keep one compile-time `Component` trait bound, control tests for one-view/retained-result behavior and anchor handoff, and focused regression tests: Browser reorder/refresh and Wide↔Normal target+offset; mixed-Service Home identity; position-free Music/TV/Audiobookshelf pushes; and source/buffer evidence for one painter. Reuse live `Application::tick()` mouse tests for #683 behavior.

## Risks / Trade-offs

- **[A stale retained result is read before view]** → result getters are frame-configured/empty until the single view call, and parents consume them only afterward.
- **[Identity lookup fails after refresh]** → controls clamp locally; effect boundaries map a missing target explicitly rather than falling back to an index.
- **[Named exemptions grow]** → ratchets name only the current grid and provider workspace seams; any new exemption requires a design/spec revision.
- **[#683 behavior regresses during reroute]** → retain its existing live-tick arbitration, throttle, and one-row tests unchanged.

## Migration Plan

1. Start from PR #683 (`6b58a608`) and characterize the list APIs and wheel path.
2. Implement the policy/result component seam and source-of-truth controls before deleting compatibility output.
3. Migrate Home/Feeds, then Browser/TV/Music, then Audiobookshelf and Queue; convert stable targets and position-free inputs before removing each mirror.
4. Add narrow ratchets, update `CONTEXT.md`, comments, and the interactive-surface ledger, and run focused tests per family.
5. Run package, formatting, clippy, architecture, and file-size gates. A human verifies representative Normal/Wide/sidebar behavior and #683 wheel behavior before acceptance.
6. Sync the delta specs into the main specs and archive only after implementation and human acceptance.

Rollback is a normal commit revert; there is no persisted-data or wire-format migration.
