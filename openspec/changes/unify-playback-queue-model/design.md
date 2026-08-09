## Context

The motivation and affected surfaces are in `proposal.md`. The important architectural facts are:

- ADR 0001 already makes `PlaybackQueue` the core queue authority with stable runtime slot identity; mpv reflects that queue rather than owning it.
- ADR 0017 distinguishes Composed queues owned by a client from Bound queues owned by a Player. This change preserves those stages and removes duplicate representations within each stage.
- `QueueItem` already represents `EmbyItem | FeedEntry`, but the TUI and daemon also retain `items` and `feed_items` collections and reconstruct mixed queues from them.
- Feed playback bypasses ordinary submission through `play_feed` and `LoadFeed`, duplicating cold-start, reuse, headless, reporting, consumption, status, and mpv-loading decisions.
- The current main specification deliberately requires an Emby prefix plus Feed tail and rejects mutations while the tail exists. This design supersedes that decision rather than adding more exceptions around it.

## Goals / Non-Goals

**Goals:**

- Establish one canonical `PlaybackQueue<QueueItem>` for every Composed or Bound queue.
- Use item-generic queue and playback operations locally and over ctrl.
- Keep queue slot coordinates, current-slot state, mpv playlist order, persistence, and UI rendering aligned.
- Preserve Composed/Bound ownership, queue scope, direct-control fall-through, and Emby reporting behavior.
- Make media-kind admission and playback lifecycle independent of `QueueItem` variant.
- Retire Feed-specific queue storage and playback orchestration after compatibility translation.

**Non-Goals:**

- Replacing `QueueItem`, `PlaybackQueue`, or the Composed/Bound model with a new domain vocabulary.
- Adding Feed resume, watched state, or Emby progress reporting.
- Making arbitrary non-mbv Emby Sessions play external URLs; unified control applies to mbv Player owners reached locally or by ctrl.
- Redesigning feed subscription storage, parsing, refresh, or management.
- Introducing distributed consensus, durable command journals, or a new dependency.
- Changing `CTRL_PROTOCOL_VERSION`.

## Decisions

### 1. `PlaybackQueue<QueueItem>` is the only queue truth

Each Composed queue and each Player-owned Bound queue stores one `PlaybackQueue<QueueItem>`. TUI presentation state may keep cursor, scroll, focus, requested scope, and cached render measurements, but it does not mirror queue contents in `Vec<EmbyItem>` or `Vec<FeedEntry>`. The daemon publishes its Player owner's queue instead of maintaining its own item-kind lists beside it.

This follows ADR 0001 and removes synchronization code rather than adding another abstraction. Rejected: retaining parallel lists behind a facade; their independently updated lengths and indices are the source of the current failures.

### 2. Preserve existing queue-slot identity as occurrence identity

`PlaybackQueue` already supplies stable runtime queue-slot identity and slot-addressed operations; this design builds on them rather than introducing another identity type. That identity remains the one used for remove, move, consume, and play-existing-slot behavior. Content identity remains variant-specific: Emby ID for an Emby item and Feed identity for a Feed entry. Two slots may contain the same content and remain independently addressable.

Queue persistence need not promise that runtime slot identifiers survive process replacement unless the existing queue serialization already provides it; it must preserve item variants and order. Rejected: targeting Feed operations by GUID, which removes every duplicate occurrence, and targeting all operations by current numeric index across asynchronously changing mirrors.

### 3. Queue mutation and playback selection are separate semantics

The shared boundary supports ordinary queue mutations—replace, append, remove, move, and clear—and selecting an existing slot for playback. Playing an existing slot never appends it. A user action that both establishes a queue and starts playback composes the same replace/append and start semantics used for any item kind; it does not introduce an item-specific command.

Feed-tab Play enters the same application submission path as library-item Play. Feed enqueue enters the same append path as library-item enqueue. Rejected: a generic `AppendAndPlayFeed` rename, which would preserve the conflation under a broader type.

### 4. One lifecycle-capable Player submission path handles every item

`Player`, `PlayerProxy`, and `RemotePlayer` expose item-generic submission and queue-control operations. The owner-side path validates the queue, starts a cold playback process when needed, and reuses or replaces an active process according to existing playback rules. Raw run-command forwarding remains appropriate only for an already running `PlaybackRun`; it is not a playback-start API.

The mpv adapter receives the canonical queue order. It does not receive a separately assembled Feed playlist or require a default/fake Emby client to satisfy Feed-only setup. Rejected: keeping `play_feed` as a convenience wrapper, because it preserves a second place for lifecycle policy to drift.

### 5. `QueueItem::media_kind()` becomes canonical before binding

All admission and headless decisions use one total media-kind accessor. An Emby item derives it from its media type. A queued Feed snapshot stores enough information to resolve classification once: enclosure MIME refines the value when recognized, otherwise the subscription's `FeedKind` is retained as fallback. Unknown or contradictory helper defaults are removed.

ADR 0017 then applies without variant checks: clients strip unplayable items before binding where required, owners enforce the same rule, and directly controlled explicit video actions may fall through locally. Rejected: treating all Feed entries as audio on a headless daemon, which confuses transport source with media capability.

### 6. Source resolution and reporting are the only variant branches

At the play boundary, an Emby item resolves to an authenticated stream URL using the Player owner's Emby context; a Feed entry resolves to its enclosure URL or fallback link. Both become the same mpv playlist input.

At the reporting boundary, Emby items build their existing reporting context and Feed entries select no Emby reporting. Queue mutation, lifecycle, status, current-slot tracking, headless selection, and consumption do not branch by variant.

### 7. Capable ctrl peers exchange one queue shape

Add one capability string for the unified queue representation. For capable peers, ctrl state carries one ordered queue-slot sequence and one current-slot coordinate; queue commands carry item-generic values or slot references. `CtrlState.items + feed_items` is not used on this path.

The capability is additive and does not bump `CTRL_PROTOCOL_VERSION`. Existing Emby-only wire fields and commands remain as a boundary adapter for older peers. A legacy `LoadFeed` may be accepted decode-only and translated immediately into the generic submission path while mixed-version support is required; current code never emits it. Compatibility data never becomes a second internal queue.

When an older peer cannot represent a mixed queue, it receives only its existing representable view and cannot perform a mutation that would silently overwrite hidden canonical slots. This preserves safety without imposing the Feed-tail invariant on capable peers.

Rejected: bumping the protocol version, because the project requires additive capabilities for additive wire behavior. Also rejected: extending `feed_items`, because it retains split coordinates by design.

### 8. Submission failures use existing result and event channels

Local proxy calls return failure when submission cannot reach an owner. Remote capability absence and ctrl-channel failure are surfaced immediately rather than logged and ignored. Owner-side validation or playback-start failures flow through the existing Player/status notification path, and state snapshots never claim an item entered a Bound queue before owner admission.

This change does not add general request IDs, command acknowledgements, or revision-based concurrency unless implementation proves an existing operation cannot report through these channels. Rejected: introducing a transactional command protocol preemptively; one owner remains authoritative and the immediate defect is ignored failure, not multi-writer consensus.

### 9. Persistence and restoration use the tagged canonical queue

All save, restore, bootstrap, shared-state, and daemon-adoption boundaries operate on the tagged `QueueItem` sequence. Legacy untagged Emby queues remain readable. Feed entries retain owned playback snapshots independently of subscription deletion, but no Feed progress state is added.

There is one conversion from legacy persistence at deserialization. Rejected: calling `emby_items()` throughout restoration, because that advertises mixed persistence while silently discarding one variant.

### 10. Consumption removes a slot, not matching content

Natural completion and explicit consumption identify the canonical queue slot. The existing consume policy determines whether that slot is removed, regardless of item kind. Feed completion does not remove every slot sharing a GUID and does not require a Feed-specific tail event.

## Risks / Trade-offs

- **[Large cross-cutting replacement can leave two models half-active]** → Migrate by authority boundary: establish the core queue API, move local Player submission, then ctrl/daemon authority, then TUI/persistence; delete each old mirror as soon as its consumer moves.
- **[Mixed-version ctrl peers cannot represent arbitrary mixed queues]** → Keep translation at the wire edge, advertise the new capability, prevent legacy mutations that would overwrite hidden slots, and test both capability combinations.
- **[Changing slot addressing can expose stale-index assumptions]** → Centralize index-to-slot resolution in `PlaybackQueue` and keep UI cursors presentation-only.
- **[Canonical Feed media kind changes prior accidental behavior]** → Resolve MIME plus subscription fallback when constructing the owned Feed snapshot and test absent/unknown MIME against audio-only admission.
- **[Player and mpv queues can diverge during failed loading]** → Admit before publishing Bound state and apply existing failed-item/skip policy against canonical slots rather than maintaining an alternate playlist index.
- **[Compatibility code may become permanent]** → Mark old Feed wire handling as decode-only and isolate it in named adapters with no use from current call sites.
- **[File-size cap during consolidation]** → Prefer deleting split-model helpers and extracting cohesive adapters over growing already-large source files; run the repository line-cap check during implementation.

## Migration Plan

1. Make `QueueItem` media classification total and make `PlaybackQueue` expose all item-generic slot operations needed by callers; preserve legacy queue deserialization.
2. Replace local `Player` and `PlayerProxy` Feed-specific startup with the shared submission/lifecycle path, while temporarily translating old call sites if needed.
3. Add the unified ctrl capability, state, and commands; move daemon authority to one `PlaybackQueue<QueueItem>` and translate legacy wire messages at the boundary.
4. Move TUI queue state, Feed/library actions, player events, queue scope, save/restore, bootstrap, and shared sync to the canonical queue.
5. Remove current-code emission of `LoadFeed`, parallel `feed_items` storage, Feed-tail guards, Feed-specific consumption, and synchronization helpers.
6. Verify local bare playback, cold stay-alive playback, direct remote playback, reconnect, persistence, mixed mutation, duplicate slots, and audio-only admission for both item kinds.

Rollback is commit-level within the feature branch before merge. The wire additions are optional behind a capability, so an implementation rollback leaves older peers on their existing behavior; no irreversible persisted-state migration is introduced.
