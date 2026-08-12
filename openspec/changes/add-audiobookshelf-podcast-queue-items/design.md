## Context

See `proposal.md` and the delta specs. #513 must be applied first so Audiobookshelf selection has a provider-specific inert handler rather than an Emby fallback. The canonical queue currently contains `EmbyItem` and `FeedEntry`; slot identity is already separate from content identity, but several matching and refresh helpers still use an unqualified string `id()`.

This is the first child of #515. It must leave the repository shippable without any Audiobookshelf playback path: the new item can be staged and persisted, but no owner may bind it yet.

## Goals / Non-Goals

**Goals:**

- Establish one source-of-truth Audiobookshelf episode payload and Service-qualified matching identity.
- Make ordinary queue editing and persistence understand the third item kind.
- Establish Service-aware owner admission without enabling Audiobookshelf playback.
- Preserve repairable state on credential rejection and purge server-owned state on replacement/removal.

**Non-Goals:**

- Audiobookshelf playback API calls, source URLs, mpv changes, reporting, or episode activation.
- Audiobookshelf ctrl transport or daemon capability.
- A common provider-neutral browse model.

## Decisions

### 1. Add a concrete QueueItem payload and typed content key

Add a concrete downloaded-podcast payload beside `EmbyItem` and `FeedEntry`. It carries `libraryItemId`, `episodeId`, show/episode presentation fields, duration, progress, completion, and Service-scoped artwork identity, but no setup, credential, server URL, playback `sessionId`, resolved source, or headers.

Every QueueItem exposes a typed provider-qualified content/position key. Identity-sensitive matching, reconciliation, and cleanup use that key rather than extending `id()` with a formatted string. Existing `id()`-style access may remain for provider-local display or compatibility, but it is not a cross-Service identity boundary. Duplicate occurrences remain distinct through `QueueSlotId`.

Alternative rejected: reuse `FeedEntry` or format `abs:<library>:<episode>`. Both erase type-safe Service ownership.

### 2. Add Service capability to canonical admission but keep it disabled

Extend admission from media kind alone to media kind plus required Service capability. Composed queues remain unrestricted because no owner executes them. Explicit submission, Composed-to-Bound binding, restoration, and cold startup use the same admission operation.

Add a semantic in-process-owner query derived from `player_endpoint == None`, distinct from same-machine ownership in ADR 0016, but do not make that owner Audiobookshelf-capable in this change. Every owner strips or visibly rejects Audiobookshelf items until #518 enables the prepared and reportable source.

Alternative rejected: mark the in-process owner capable immediately. Queue representation alone cannot safely open, report, or close an Audiobookshelf session.

### 3. Keep persistence transport-ready but off ctrl

Persist the tagged third variant and preserve legacy untagged Emby state. Old binaries are not expected to read state written with the new variant. Admission runs before restored state binds.

The serialized variant does not imply ctrl support. Clients strip Audiobookshelf items before submission and owners discard any that arrive. No new ctrl capability is advertised; a future daemon milestone must add one before transport.

### 4. Separate credential rejection from Service identity invalidation

Explicit credential rejection preserves server setup and Service-owned repairable state under the existing Service-management contract. Clear the rejected secret and make Audiobookshelf items ineligible for Bound queues, while retaining Composed and persisted snapshots for repair.

Confirmed replacement/removal invalidates server-native IDs and purges Audiobookshelf items from Composed, Bound, and persisted state through one semantic cleanup operation. It does not change Emby or Feed items.

### 5. Remove Emby-only cold-start assumptions without adding sources

Make cold queue construction item-generic so Feed and future Audiobookshelf slots do not require an Emby snapshot merely to reach admission. Source preparation remains unchanged and Audiobookshelf items cannot pass admission in this child.

## Risks / Trade-offs

- **[Risk] Exhaustive QueueItem matches are spread across queue, status, render, and persistence code** -> Use the compiler plus a repository-wide variant audit and keep item-specific branching out of generic queue operations.
- **[Risk] Bound stripping could overwrite preserved staged state after credential rejection** -> Keep Service-owned persisted/Composed state reconciliation distinct from the admitted Bound projection.
- **[Risk] A serializable variant is mistaken for ctrl support** -> Assert that no Audiobookshelf item appears in ctrl state and advertise no transport capability.
- **[Trade-off] Users can stage an item that no owner can yet play** -> Accept this temporary first-child state; explicit submission fails visibly and #518 enables the eligible owner.

## Migration Plan

1. Complete #513 and verify the provider-specific inert episode handler.
2. Add the payload and typed key, then update generic accessors and metadata projection.
3. Extend persistence/restoration and Service cleanup.
4. Extend owner admission and cold item-generic construction while leaving all owners ineligible.
5. Verify staged mixed queues, restore, rejection/repair, replacement/removal, and absence from ctrl.

Rollback removes persisted Audiobookshelf entries before removing the variant. No protocol migration is required.
