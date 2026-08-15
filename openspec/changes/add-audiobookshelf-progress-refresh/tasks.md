## 1. Engine.IO / Socket.IO v4 framing

- [ ] 1.1 Add an Audiobookshelf socket module in `crates/mbv-core/src/` (e.g. `audiobookshelf_socket.rs`) parsing Engine.IO v4 packet types (open/ping/pong/message/close) and, inside message packets, Socket.IO v4 packet types (connect-ack, event), targeting `wss://<server>/socket.io/?EIO=4&transport=websocket` directly (no polling handshake).
- [ ] 1.2 Parse the Engine.IO `open` packet's `pingInterval`/`pingTimeout` and use them for heartbeat/staleness detection instead of hardcoding Emby's `ws.rs` constants.
- [ ] 1.3 Decode `42["user_item_progress_updated", {...}]` event packets into a typed progress-update value; ignore all other event names and all non-EVENT Socket.IO packet types.
- [ ] 1.4 Unit tests for the framing parser: open packet ping interval/timeout extraction, `user_item_progress_updated` decode, `stream_progress` and one unrelated event both decode to "ignored", and malformed/truncated packets return no event (mirrors `ws.rs`'s existing `parse`/test-module shape).

## 2. Connection lifecycle

- [ ] 2.1 Implement `start`/`WsSender`-equivalent background-thread connection with `mpsc` outbound channel and exponential backoff with jitter, mirroring `crates/mbv-core/src/ws.rs`'s shape.
- [ ] 2.2 Emit the `auth` client event with the installed API key immediately after the Socket.IO connect acknowledgement.
- [ ] 2.3 On `invalid_token`, surface the same Audiobookshelf Service authentication failure classification used elsewhere; do not clear the installed API key from this alone.
- [ ] 2.4 On unexpected disconnect, drop any buffered pre-disconnect state before reconnecting so no stale event replays after reconnect (mirror `ws.rs`'s `drop_stale_outbound`).
- [ ] 2.5 Wire connect/disconnect into the interactive process's Audiobookshelf Service Ready/replace/remove call sites (parallel to `mbv_core::ws::start`/`ws_send_tx` in `src/app/emby_service_actions.rs` and `app_struct.rs`), scoped to the current setup generation.
- [ ] 2.6 Do not add any Local daemon or packaged `mbvd` call site.

## 3. Progress merge

- [ ] 3.1 On a decoded `user_item_progress_updated`, resolve the target `(libraryItemId, episodeId)` and, when it matches cached browse progress or an inactive queue slot for the current setup generation, merge the event's progress data in place (no REST call).
- [ ] 3.2 Before merging, check the identity against the in-process Player owner's currently active Audiobookshelf slot at merge time; skip the merge if it matches the active slot.
- [ ] 3.3 Drop events whose connection generation is older than the current Audiobookshelf setup generation.
- [ ] 3.4 Tests: merge updates a matching inactive/browsed episode; merge is skipped for the active slot; merge is skipped for an unmatched episode; merge is skipped for a superseded generation.

## 4. Wrap-up

- [ ] 4.1 `rtk cargo check -p mbv-core` and `-p mbv` (interactive binary crate).
- [ ] 4.2 `rtk cargo nextest run -p mbv-core` and the interactive crate's test target.
- [ ] 4.3 `rtk cargo clippy --workspace --all-targets`.
- [ ] 4.4 `rtk make check-code-file-lines`; split any new file that exceeds 800 lines.
- [ ] 4.5 Call JCodeMunch `register_edit` for all touched/added files.
