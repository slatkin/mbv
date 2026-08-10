## 1. Store layer (`shared_store.rs`)

- [ ] 1.1 Add `feed_entry_state` table: `TableDefinition<(&str, &str, &str), &str>` keyed `(user_id, feed_id, entry_guid)`, opened on the existing `Database`.
- [ ] 1.2 Define the stored value type (`position_ticks`, `played`) with serde; serialize to/from the JSON string value the way documents do.
- [ ] 1.3 Implement `get_feed_entry(user_id, feed_id, guid) -> Option<value>` (point read).
- [ ] 1.4 Implement `put_feed_entry(user_id, feed_id, guid, value)` — unconditional last-write-wins insert, acknowledge only after commit.
- [ ] 1.5 Implement `scan_feed_entries(user_id, feed_id) -> Vec<(guid, value)>` via a bounded `range` over the `(user_id, feed_id, *)` prefix.
- [ ] 1.6 Unit tests: round-trip put/get; put overwrites (last-write-wins, no CAS); prefix scan returns only the target feed's entries with a feed-id-prefix collision case and a second user present; scan of an empty feed returns empty.

## 2. Protocol (`shared_protocol.rs`)

- [ ] 2.1 Add `SHARED_DATA_CAP_FEED_ENTRY_STATE_V1` and advertise it in `SharedDataHello::current()`; leave `SHARED_DATA_PROTOCOL_VERSION` unchanged.
- [ ] 2.2 Add `SharedDataCmd` variants for get-one, put-one, and scan-a-feed (each with `request_id` and the key parts; put carries the value).
- [ ] 2.3 Add matching `SharedDataEvent` response variants (entry value / entry absent, put acknowledged, scan result), plus request-scoped error reuse.

## 3. Service + worker routing (`shared_service.rs`, `shared_worker.rs`)

- [ ] 3.1 Route the new commands to the store ops, scoping every key to the connection's authenticated `user_id` (never trust a client-supplied user id).
- [ ] 3.2 Emit the response events; on store failure return a request-scoped error without affecting playback or other connections.
- [ ] 3.3 Confirm no notification/broadcast path is triggered for feed entry writes (they are not documents).

## 4. Client (`shared_client.rs`)

- [ ] 4.1 Add client methods for get / put / scan that issue the new commands and await their responses.
- [ ] 4.2 Gate the methods on the advertised feed-entry-state capability; when absent, report feed entry state as unavailable (no protocol error), consistent with local fallback.

## 5. Verify

- [ ] 5.1 `cargo test -p mbv-core` — new store tests pass, existing shared-store/CAS tests stay green.
- [ ] 5.2 `cargo clippy --workspace --all-targets` clean.
- [ ] 5.3 `make check-code-file-lines` passes (split any file that crosses 800 lines in the same change).
- [ ] 5.4 Confirm no playback path, ctrl protocol version, or `FeedEntry` type was touched.
