## 1. Gate carries its executor

- [x] 1.1 Add `ReplacementExecutor { Routed(RoutedReplacementPrep), Pending }` (next to `PendingQueueAction`; prep variants `Album`/`MusicAlbums`/`Folder`/`ShuffleFolder`/`Selection`); change `pending_queue_replacement` to `Option<(PendingQueueAction, ReplacementExecutor)>` in `app_struct.rs` / `construct.rs`
- [x] 1.2 `request_queue_replacement(action, via)` in `queue_actions.rs`; add `run_replacement(action, via)` dispatching Routed → `run_routed_replacement` (replay the entry point's pre-play prep, then `play_items_routed`), Pending → `execute_queue_replacement`
- [x] 1.3 `ReplacePopulatedQueue` arm in `input_confirm_keys.rs` takes the tuple and calls `run_replacement`
- [x] 1.4 Update existing `request_queue_replacement` callers and tests (`play_grouped_track`, `input_confirm_keys_tests.rs`) to pass `Pending`

## 2. Route the entry points through the gate

- [x] 2.1 `replace_and_route_album_queue` → `request_queue_replacement(PlayItems{..,autostart:true}, Routed)`; move any pre-play queue mutation after the gate (design D4)
- [x] 2.2 The three `play_items_routed` sites in `shuffle_folder_actions.rs` → gate with `Routed` (D4 applies)
- [x] 2.3 `context_menu_actions.rs` PlaySelection / ShuffleSelection: move `rebuild_queue_for_selection` into the confirmed path; gate with `Routed`
- [x] 2.4 `shell_playlists.rs` playlist load: `replace_queue_or_prompt` → `request_queue_replacement(.., Pending)` (the `load_and_play_playlist` route in `library_load_actions.rs`, `PlaylistsActivate{open:false}`, was gated as an additional playlist-load site in 5382bf01)

## 3. Tests (mock-only, `cargo nextest run -p mbv`)

- [x] 3.1 Populated queue + album track: modal shown, queue unchanged; `y` plays via the routed path; Esc leaves the queue and `pending_queue_replacement` empty
- [x] 3.2 Populated queue + context-menu Play, then cancel: queue unchanged (covers D4)
- [x] 3.3 Populated queue + playlist load: modal shown; confirm then reaches the dirty-playlist prompt when the queue is a dirty saved playlist
- [x] 3.4 Empty queue: album track / shuffle / playlist run with no modal
- [x] 3.5 Populated queue + wholly-unplayable items: replace modal first; confirm then raises "play locally instead"
- [x] 3.6 Library autoplay and `play_item` with a populated queue: no replace modal

## 4. Docs

- [ ] 4.1 Update `docs/invariants/10-deferred-queue-mutation-slot-ownership.md`: slot payload now `(action, executor)`, writer and reader line refs
- [ ] 4.2 Gates: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`
