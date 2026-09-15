## Context

See `proposal.md` for motivation. The Library panel currently chooses `WideMediaList` or `InlineMediaBrowser` through `MediaListCarrier`; `render_narrow_skeleton` then paints Inline hero content into the adapter's admitted selected-row replacement. Wide composition already has the complete Hero vocabulary in `library_panel/wide.rs`: Hero header, overview Main content box, image projection, optional selector, and canonical Workspace list.

The existing constituent selection modal is a separately mounted, blocking application overlay centered over the terminal. That ownership cannot satisfy this change because Queue must remain independently focusable and operable. The Library panel is already the mounted parent that owns Library placement, current-frame hit geometry, active destination owners, and keyboard forwarding.

## Goals / Non-Goals

**Goals:**

- Keep one fixed-row canonical browser presentation active across Wide and non-Wide geometry.
- Compose the Library Hero overlay within the Library panel and reuse the Wide Hero content and Workspace owners rather than copying them.
- Preserve central keyboard arbitration, panel-local mouse ownership, stable-target list state, and one painter per surface.
- Delete the Inline presentation and constituent selection modal paths once all destinations use the replacement.

**Non-Goals:**

- Changing Wide Hero appearance or Workspace semantics.
- Changing Inline Search, Queue presentation, provider fetches, playback behavior, or queue authority.
- Introducing a generic overlay framework or a second focus stack.
- Persisting an open Library Hero overlay across destination changes or application restarts.

## Decisions

### 1. LibraryPanelComponent owns the overlay lifecycle and geometry

`LibraryPanelComponent` will hold the open/dismissed presentation state and retained overlay/backdrop hit geometry. Opening and dismissal are local component transitions: they do not enter `App`, shell state, or the global overlay mount/focus stack. The active `LibraryContentOwner` continues to own provider content, browser and Workspace canonical lists, filters, cursors, and typed external intents.

The panel will dismiss before switching active `LibraryKey`, and will dismiss if the active Hero's stable parent target disappears. Ordinary refresh for the same target updates the projected Hero and Workspace in place.

Alternative: mount another application overlay. Rejected because mounted overlays currently take exclusive focus/mouse delivery and would make Queue inoperable or require an exception to the global overlay contract.

### 2. Non-Wide browser composition reuses the fixed-row presentation

`MediaListCarrier` will stop switching owners between Wide and Inline adapters. The existing fixed-row adapter and canonical owner remain active while geometry changes; only claim/content rectangles, viewport clamping, and paint policy change. `InlineMediaBrowser`, Inline paint policy, selected-detail admission, detail geometry, and viewport-transfer machinery that exists only for adapter handoff can then be deleted.

The existing name `WideMediaList` may remain during this change; renaming it is not required to deliver behavior and is not a reason to broaden the implementation.

Alternative: retain `InlineMediaBrowser` but configure it with zero detail rows. Rejected because it preserves a redundant adapter, responsive handoff, and dead selected-replacement semantics.

### 3. Extract one reusable Hero-content composition from Wide

The Hero header, overview Main content box, Workspace placement, selector painting, image-box result, and their paint-local geometry will be factored from the Wide skeleton into a shared Library-panel Hero-content path. Wide calls it in its existing Hero pane; the Library Hero overlay calls the same path in its framed inner rectangle.

The overlay is centered at 85 percent of the current Library pane's width and height, rounded and bounded to remain inside that pane. The shared content allocation continues to shrink artwork before starving a present Workspace viewport. Image projection consumes the box returned by whichever Hero surface is currently painted; it does not independently recalculate overlay geometry.

Alternative: render the whole Wide two-pane skeleton inside the overlay. Rejected because the overlay needs only the right-pane Hero content and rendering the browser pane twice would violate one-owner/one-painter composition.

### 4. Overlay focus is Library subfocus, not application-modal focus

When Library holds panel focus and the overlay has a Workspace, that existing Workspace list receives local keys and pointer gestures. A leaf Hero holds overlay focus; its second Enter performs the destination's existing parent activation. Child activation performs the existing typed intent and leaves the overlay and Workspace state intact.

Moving panel focus to Queue does not close or unmount the overlay. Queue receives its ordinary keys and pointer gestures, while the Library panel paints the overlay unfocused. Returning focus to Library restores the destination owner's existing Workspace cursor, scroll, selector, and focus state.

Esc is intercepted by the Library panel only while Library is focused and the overlay is open. While Queue is focused, Queue retains its existing Esc semantics, including selection cancellation and central double-Esc behavior. No new keyboard-resolution site is introduced: the central router still decides global precedence, and the focused Library component applies this local transition only after fall-through.

Alternative: make Esc globally close any visible Hero overlay. Rejected because it steals Queue-local input while Queue is meant to remain fully operable.

### 5. Opening and pointer dismissal are panel-local operations

Enter or a double-click on a canonical browser item opens its Hero overlay instead of directly activating it. A double-click on a Workspace child performs that child's normal activation and leaves the overlay open. Inline Search remains outside this rule and preserves its existing activation.

The Library panel paints and retains both overlay and dimmed-backdrop geometry. A click inside the overlay is resolved only against overlay content. A click in the dimmed Library remainder dismisses and is consumed without forwarding to the covered browser. A click in Queue is delivered by Queue's own current-frame eligibility and leaves the overlay open.

Alternative: dismiss and replay the same click into the browser. Rejected because the component would mutate after the losing surface had already claimed the gesture, creating accidental selection or activation.

### 6. Remove the constituent modal after destination intents converge

TV, Music, Audiobookshelf Podcast, and Audiobookshelf Book will stop constructing `SelectionModal` state for constituent browsing. Their existing Workspace content, loading/empty states, stable targets, filters, and activation intents become the sole path in both Wide Hero and the Library Hero overlay. The selection-modal component and render path are deleted only after no unrelated caller remains; any generic modal-frame primitives still used elsewhere remain.

## Risks / Trade-offs

- [The shared Hero painter may still contain Wide-pane assumptions] -> Characterize Wide output first, extract content composition without changing its supplied rectangle contract, and keep Wide buffer assertions unchanged.
- [An 85-percent rectangle may be very small at Mini dimensions] -> Bound it within the Library pane and let existing constrained Hero allocation prioritize a usable Workspace list; add explicit tiny-area containment tests.
- [Overlay and covered-browser mouse geometry could both remain valid] -> Invalidate or gate browser claims while the overlay is open and consume Library-backdrop dismissal before any underlying delegation.
- [Removing the adapter may disturb selection position across resize] -> Keep the same canonical owner and fixed-row presentation, then assert stable target and clamped viewport across Wide/non-Wide transitions.
- [Provider completion may target a dismissed or changed Hero] -> Keep existing provider-native/stable-target guards and dismiss on destination or parent-target identity change.
- [Leaf activation gains one extra action] -> Make both Enter and double-click consistently open detail first and visibly advertise Esc dismissal.

## Migration Plan

1. Add focused characterization for current Wide Hero content and canonical Workspace ownership.
2. Introduce Library-confined overlay state, placement, focus, rendering, and hit geometry while retaining existing paths.
3. Route non-Wide Enter/double-click and Workspace interaction through the overlay for each destination.
4. Move all non-Wide browser lists to the fixed-row presentation and remove Inline selected-row painting.
5. Remove the constituent selection-modal callers and then delete now-unused modal and Inline presentation code.
6. Update `CONTEXT.md`, synchronize durable specs on archive, and run focused component, arrangement, render-buffer, and mounted tick/mouse verification at Wide, Narrow, Mini, and constrained-height geometry.

Rollback is a normal commit revert because the change alters only in-process presentation and interaction paths; it has no persisted-state or protocol migration.
