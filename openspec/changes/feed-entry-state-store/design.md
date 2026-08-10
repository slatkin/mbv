## Context

See proposal.md — Why. The shared-data stack already exists and ships against redb: `shared_store.rs` owns a single `TableDefinition<(&str, &str), &str>` keyed `(kind, user_id)` → JSON string, serving four revisioned `SharedDocumentKind`s through `create_document` / `update_document` (CAS on a per-document revision). `shared_protocol.rs` carries `SharedDataCmd` / `SharedDataEvent`, a `SHARED_DATA_PROTOCOL_VERSION` (1), and additive `SHARED_DATA_CAP_*` strings advertised in `SharedDataHello::current()`. `shared_worker.rs`, `shared_service.rs`, and `shared_client.rs` route those messages. Feed entry state is unbounded and per-entry, so it cannot ride the single-document/CAS path (see specs/feed-entry-state/spec.md).

## Goals / Non-Goals

**Goals:**
- Add feed entry persistence and its get/put/prefix-scan wire operations as a strictly additive extension of the existing shared-data stack.
- Keep the new table and its ops isolated from the revisioned-document code path — no shared transaction, no shared revision.
- Land with round-trip and prefix-scan tests and no playback caller.

**Non-Goals:**
- Any `FeedEntry` field change, player resume/completion wiring, or feeds-tab UI — those are D2 (#493) and D3 (#494).
- Cross-entry atomicity, revisions, or change notifications for feed entries.

## Decisions

**Second redb table, not a reuse of the documents table.** A new `TableDefinition<(&str, &str, &str), &str>` keyed `(user_id, feed_id, entry_guid)` → JSON `{ position_ticks, played }`, opened on the same `Database` as the documents table. Alternative — encoding feed entries as another `SharedDocumentKind` — was rejected: that key space is `(kind, user_id)`, one row per user, and the whole path is CAS-revisioned; feed entries are many rows per user with last-write-wins. A distinct table keeps the two models from contaminating each other and lets the documents table stay exactly as specced in `shared-mbv-state`.

**Tuple key with user_id first, for prefix scan.** redb orders tuple keys component-wise, so `(user_id, feed_id, *)` rows are contiguous. Prefix scan uses a bounded `range` from `(user, feed, "")` up to the next feed boundary, filtering to the exact `(user, feed)` prefix. Putting `user_id` first also makes per-user isolation a key-prefix property rather than a filter applied after read.

**Last-write-wins put.** The put op inserts unconditionally, replacing any existing row. No revision is read or compared. This is the deliberate departure from `update_document`'s CAS and is why feed entries get their own ops rather than reusing `UpdateDocument`.

**New protocol variants + one capability string.** Add request variants (get one, put one, scan a feed) to `SharedDataCmd` and their responses to `SharedDataEvent`, plus `SHARED_DATA_CAP_FEED_ENTRY_STATE_V1` advertised in `SharedDataHello::current()`. `SHARED_DATA_PROTOCOL_VERSION` stays 1 — additive, per the shared-data convention mirroring ctrl. The client gates feed ops on the advertised capability and degrades to "unavailable" otherwise.

**Store failure fails the op only.** Feed entry commits use the same acknowledge-after-commit discipline as documents; a failed commit fails that operation and leaves daemon playback and prior rows untouched (spec: "Feed entry storage failure is isolated from playback").

## Risks / Trade-offs

- **Two tables on one database file** → both open under the existing `Database` handle and transaction model; no new file, lifecycle, or corruption surface beyond the existing store. Storage-failure isolation already required by the spec.
- **Prefix-scan range bounds on tuple keys** → get the upper bound wrong and a scan leaks an adjacent feed's rows or truncates. Mitigation: cover the boundary in a unit test (a feed whose id is a prefix of another feed's id, plus a second user, must not bleed into the scan).
- **Unbounded row growth** → feed entry rows accumulate with no eviction. Acceptable for this change; pruning (e.g. on unsubscribe) is future work, not required by #472's acceptance.

## Open Questions

None that affect the specs, approach, or task breakdown. Row pruning on unsubscribe is deferred and does not change this store's contract.
