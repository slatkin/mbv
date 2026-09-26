# Tasks

Each group is one commit. The gate for a group is `cargo check -p mbv`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv`, all green, with `cargo fmt` applied. No behaviour edits inside a move. No lint suppressions.

## 1. Player event dispatch (design D4)

- [ ] 1.1 In `src/app/dispatch/session/player_event.rs`, add `pub(in crate::app) enum PlayerEventFlow { Proceed, RestartLoop }` and change `handle_player_event` to return it. `true` maps to `RestartLoop`, `false` to `Proceed`. Update the caller in `src/app/shell/run/drains.rs::drain_player_events` so it still returns `bool` to its own caller (`flow == PlayerEventFlow::RestartLoop`; derive `PartialEq, Eq, Debug`). Verify: gate green.
- [ ] 1.2 Replace the body of `handle_player_event` with one exhaustive `match ev` holding an arm for every `PlayerEvent` variant. Move each arm from `handle_player_event_playback` / `_notices` / `_queue_state` / `_progress` into it, and move bodies longer than ~10 lines into a named `fn handle_<variant>` helper in the same file. Delete the four `Result<bool, PlayerEvent>` stage functions. Wildcard arms are forbidden; intentionally ignored variants get an explicit arm plus a one-line comment. Verify: `rg "Result<bool, PlayerEvent>" src` is empty, and the gate is green.

## 2. Library event dispatch (design D5)

- [ ] 2.1 In `src/app/dispatch/library/event.rs`, rewrite `handle_lib_event` as one exhaustive `match ev` over all `LibEvent` variants, each arm calling one handler. Extract every inline arm body from `handle_browse_event`, `handle_music_event`, `handle_playlist_event` (this file) and `handle_audiobookshelf_event` (`event/audiobookshelf.rs`) into `fn handle_<variant_snake>` in the file where the body lives today. Turn early `return None` inside a moved body into plain `return`. Delete the four family dispatchers and their `unreachable!()` arms. Leave `dispatch/library/browse/tv.rs`'s `unreachable!` alone. Verify: `rg "Option<LibEvent>|unreachable!" src/app/dispatch/library/event.rs src/app/dispatch/library/event/` is empty, and the gate is green.

## 3. ImageCache seam (design D1–D3)

- [ ] 3.1 Create `src/app/infra/images/cache.rs` with `pub(in crate::app) struct ImageCache` holding these fields, moved verbatim with their doc comments from `App` in `src/app/state/app_struct.rs`: `card_image_states, image_lru, image_cache_size, card_image_loading, last_card_height, last_card_width, pending_image_fetches, image_fetches_active, card_image_tx, card_image_rx, resize_register_tx, resize_response_rx, image_picker, halfblock_picker, image_cache_size_total, image_protocol, image_protocol_enabled`, plus the `#[cfg(test)]` fields `card_image_fetch_calls, image_protocol_builds`. Add a constructor that takes whatever `App::build` in `src/app/state/construct.rs` currently computes for them. Declare the module from `src/app/infra/images.rs`. Verify: `cargo check -p mbv` (the struct is unused so far; if the compiler reports dead code, add the `App` field in the same step as 3.2).
- [ ] 3.2 Add the field `pub(in crate::app) images: ImageCache` to `App`, delete the moved fields from `App`, build `images` in `App::build` via the constructor, and fix every compile error by rewriting `self.<field>` / `app.<field>` to `.images.<field>`, tests included. Leave `queue_card_projection` and `dim_backdrop_active` on `App`. Verify: gate green.
- [ ] 3.3 Move each `impl App` method in `src/app/infra/images.rs`, `images/protocol.rs`, `images/fetch/card_images.rs` and `src/app/infra/resize.rs` whose body reads or writes only `self.images.*` into `impl ImageCache` (receiver `&mut self` / `&self` on the cache), and update callers to `self.images.method(..)`. Leave methods that also touch other `App` state where they are. Verify: gate green.

## 4. ServiceSetup seam

- [ ] 4.1 Create `src/app/state/service_setup.rs` with `ServiceSetup` holding these fields, moved from `App` with their types and docs: `emby_startup_rx, emby_startup_request, audiobookshelf_startup_rx, audiobookshelf_startup_request, audiobookshelf_catalog_rx, audiobookshelf_test_rx, audiobookshelf_setup_rx, emby_setup_form, audiobookshelf_setup_form, emby_setup_rx, pending_emby_replacement, pending_audiobookshelf_replacement`. Add a constructor. Add the field `setup: ServiceSetup` to `App`, delete the originals, and fix all access paths to `.setup.<field>`. Verify: gate green.
- [ ] 4.2 Move `impl App` methods that touch only `self.setup.*` into `impl ServiceSetup` (look under `src/app/dispatch/session/service_startup*` and `services_settings*`). Verify: gate green.

## 5. RemoteTracking seam

- [ ] 5.1 Create `src/app/state/remote_tracking.rs` with `RemoteTracking` holding these fields, moved from `App`: `remote_pos_s, remote_pos_at, remote_api_pos_advanced_at, remote_stalled_while_paused, remote_seek_pending_until, runtime_zero_since, session_miss_count, last_session_poll, direct_remote_connected, direct_remote_label, direct_remote_session_id`. Do NOT move `connected_session_id` / `connected_session_state`. Add a constructor with today's initial values from `construct.rs`. Add the field `remote: RemoteTracking` to `App`, delete the originals, and fix access paths to `.remote.<field>`. Keep every write site's values exactly as they are, including the unpaired direct-remote writes in `dispatch/session/switch.rs` and `connect.rs`. Verify: gate green.

## 6. RuntimeChannels seam

- [ ] 6.1 Create `src/app/state/runtime_channels.rs` with `RuntimeChannels` holding `lib_tx, lib_rx, search_tx, search_rx, sessions_tx, sessions_rx, cast_tx, cast_rx, notif_action_tx, notif_action_rx`, and a `new()` that creates the five `mpsc::channel()` pairs as `construct.rs` does today. Add the field `channels: RuntimeChannels` to `App`, delete the originals, and fix access paths to `.channels.<field>`. `player_rx`, `ws_rx` and the audiobookshelf socket fields stay on `App`, because they are swapped at runtime by player/remote binding. Verify: gate green.

## 7. Wrap-up

- [ ] 7.1 Run `rg -c "^\s+pub\(in crate::app\) \w+:" src/app/state/app_struct.rs` and record the before/after field count in the PR description. Run `make check-code-file-lines` before pushing and split any file over 800 lines along responsibility seams. Verify: the check passes.
