## Why

Feed playback state (resume position, watched flag) has to roam across machines the way Emby item state does, but it is per-entry and unbounded — the wrong shape for the revisioned single-document store that `shared-mbv-state` (#433) provides. This change adds the persistence layer alone, behind its own compiling checkpoint, so the daemon/redb piece that repeatedly derailed past RSS attempts (#461) lands and is proven before any playback code depends on it.

## What Changes

- New keyed redb table `feed_entry_state`, hosted by the daemon alongside the existing shared-documents table. Key `(user_id, feed_id, entry_guid)` → `{ position_ticks, played }`.
- New keyed operations on the **existing** shared store/worker/service/client/protocol modules: get one entry, put one entry, prefix-scan over `(user_id, feed_id, *)`. These **extend** `SharedDataCmd` / `SharedStoreRequest`; they do not fork a parallel subsystem.
- **Last-write-wins, no CAS.** Feed entries are independent rows, deliberately outside the revisioned-document path the existing `SharedDocumentKind`s use.
- Additive shared-data **capability string** (mirroring the `SHARED_DATA_CAP_*` mechanism), not a protocol bump.
- Reuses the existing shared-data transport, per-user identity, and local-fallback behavior unchanged — no new auth, transport, or endpoint surface.

No playback caller is added here. Player resume/completion wiring is D2 (#493); the feeds-tab watched filter is D3 (#494).

## Capabilities

### New Capabilities

- `feed-entry-state`: per-user, per-entry feed playback state (position + watched) stored in a keyed daemon-hosted redb table on the shared-data transport, with last-write-wins semantics and no cross-entry transaction.

### Modified Capabilities

None. The existing `shared-mbv-state` capability's transport, identity, and fallback requirements are reused unchanged; its document/CAS model is not altered.

## Impact

- `crates/mbv-core/src/shared_store.rs`, `shared_worker.rs`, `shared_service.rs`, `shared_client.rs`, `shared_protocol.rs` — new keyed table and get/put/prefix-scan ops, new capability string.
- No change to the ctrl protocol version, to any playback path, or to `FeedEntry`.
- Existing shared-store tests must stay green.
