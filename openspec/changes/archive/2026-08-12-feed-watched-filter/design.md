## Context

See `proposal.md` for motivation and `specs/feed-subscriptions/spec.md` for the browsing contract. #492 added a capability-gated feed-entry table and feed-scoped prefix scan keyed by `(user_id, feed_id, entry_guid)`. #493 then gave `FeedEntry` the normalized subscription URL, position, and played fields and wired point reads and event-driven writes at playback boundaries. Its design deliberately deferred prefix-scan consumption to this change.

The Feeds tab currently keeps canonical per-subscription entry vectors plus an All vector rebuilt from them. Rendering, cursor movement, mouse row mapping, play, and enqueue all index the selected group's visible entries. Feed fetching already completes through the App's refresh-result path, while shared-state access is optional and synchronous.

## Goals / Non-Goals

**Goals:**

- Hydrate each refreshed subscription with one feed-scoped state scan before rebuilding the All group.
- Give every Feeds-tab consumer one consistent filtered view so displayed positions and actions cannot diverge.
- Make filter transitions deterministic and keep storage failure outside the browsing and playback success paths.
- Prove the existing store, playback wiring, and new browsing behavior together across machines.

**Non-Goals:**

- Any new played-state write, periodic checkpoint, notification stream, or automatic state refresh.
- Roaming subscriptions; each machine must configure the same normalized feed URL independently.
- Changing feed-entry identity, shared-data protocol negotiation, queue duplicate rules, or lifecycle completion semantics.
- Showing resume position in feed rows; event-driven checkpoints are not current enough to warrant the added row density.

## Decisions

### Hydrate at the feed refresh result boundary

When a subscription fetch succeeds, scan the shared store once for that subscription's normalized URL and merge returned rows into parsed entries by entry GUID. Install the hydrated subscription vector only after the merge, then rebuild the All vector from those hydrated entries. Unknown stored GUIDs are ignored, and absent rows retain parser defaults.

This consumes the prefix-scan operation built for browsing and avoids one synchronous request per entry. Point hydration before play remains in place because it protects playback from a stale browsing snapshot. Scanning on every `w` press was rejected because the filter is a local view operation and must never become an implicit network action.

An unsupported capability, missing client, disconnection, timeout, or scan error yields the original parsed entries and a logged diagnostic only. The model has no third "unknown" played value, so unavailable state intentionally degrades to unplayed without an unavailable badge.

### Keep source entries canonical and filter through shared accessors

Add a three-state watched-filter value to `FeedTabState`, defaulting to All and cycling `All -> Watched -> Unwatched -> All`. Preserve the full per-subscription and All vectors; derive count, iteration, and indexed selection from the active group plus filter rather than destructively replacing either source vector.

All consumers must use this seam: cursor and page bounds, rendering and age-heading row maps, mouse selection, play, and enqueue. This prevents the common failure where a row rendered from a filtered list invokes the same numeric index in the unfiltered list. The exact iterator or index-cache representation can follow the smallest existing Rust pattern as long as it does not clone whole `FeedEntry` values for each lookup.

Changing filters resets cursor and scroll to zero while preserving the selected feed group. Resetting is preferred to retaining a raw index because the same index can identify a different entry after filtering, and it matches existing group-transition behavior.

### Bind only plain `w` in the existing Feeds handler

Handle unmodified `w` in the current Feeds-tab key path and consume it there. Modifier variants remain available to their existing meanings, including watched-state editing outside the Feeds tab. Although ADR 0002 describes convergence on semantic commands, migrating the existing imperative Feeds handler is broader than #494; this change follows the live code path rather than creating a parallel input architecture.

### Render filter state separately from played state

Use the available Feeds-tab header space for a stable label naming All, Watched, or Unwatched. Give played entries a compact marker using established row styling, including under the All filter. Do not show resume timestamps or storage-availability warnings. Add Feeds-specific help text for `w` so the binding is not confused with Emby watched editing.

### Refresh is the cross-machine consistency boundary

Feed-entry writes do not publish invalidations. A second machine observes position or played changes after its user presses `r`, which reloads both feed content and currently available entry state. The current machine follows the same browsing rule; playback still point-hydrates independently before starting. This explicit refresh model avoids polling and keeps the feature consistent with the Feeds tab's existing manual-refresh contract.

The roaming acceptance run must keep a client attached through each tested pause, stop, or EOF event because #493 persists lifecycle events in the App. Player-owner-resident persistence after the last client closes remains outside this design.

## Risks / Trade-offs

- **A synchronous feed-scoped scan can delay processing a completed refresh** -> Perform one scan per successful subscription fetch, retain existing operation timeouts, and make every failure non-fatal; do not add per-entry calls.
- **Unavailable state appears unwatched** -> Treat zero/unplayed as the documented stateless fallback and avoid UI that claims an availability result.
- **Filtered indices can drift from source indices** -> Centralize filtered iteration, count, and indexed lookup and use them in every render, input, and action path.
- **Another machine's state is stale until refresh** -> Make `r` the explicit consistency boundary and cover it in help and roaming verification.
- **An attached client can disappear before a lifecycle event is persisted** -> Keep the client attached during acceptance verification and leave owner-resident persistence to a separate architecture change.

## Migration Plan

No data or protocol migration is required. Existing parsed entries already default to zero position and unplayed, and existing stored rows become visible on the next successful refresh. Rollback removes the filter and bulk hydration while leaving feed-entry rows and #493's playback-time hydration intact. Update the FeedEntry domain description in `CONTEXT.md` with the implementation so documentation matches the shipped queue model.
