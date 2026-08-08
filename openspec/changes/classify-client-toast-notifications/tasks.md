## 1. Core type, field, rendering

- [ ] 1.1 Define `ToastSeverity { Neutral, Success, Warning, Error }` in `notify_actions.rs` with TTL mapping (Neutral/Success 2s, Warning/Error 5s)
- [ ] 1.2 Implement `flash(msg, severity)`: sets status/severity/expiry; never rings the bell; `notify-send` only when severity != Neutral
- [ ] 1.3 Add `status_severity: ToastSeverity` to `App` (default `Neutral`; init in `construct.rs` and test stubs)
- [ ] 1.4 Add green/yellow toast background tokens to `palette.rs` (red exists as `TOAST_BG`)
- [ ] 1.5 Render: `status_expires.is_none()` (prompts) and Neutral → status-bar styling; Success/Warning/Error → severity color; clear severity when the toast expires

## 2. One-pass call-site migration

Mapping: `flash_status` → Neutral (progress/info) or Success (completed action); `flash_status_high` → Warning (recovered via fallback) or Error (unrecovered failure). Rewrite `"Playing on remote: {label}"` → `"Requesting playback: {label}"` where it fires before submission.

- [ ] 2.1 `player_event.rs`
- [ ] 2.2 `session_connect.rs`, `remote_slot_state.rs`, `playback_target_local.rs`
- [ ] 2.3 `actions.rs` (incl. 2× "Playing on remote"), `action.rs` (1×), `queue_actions.rs` (1×), `queue_actions_playlist_mutation.rs`, `input_queue_keys.rs`, `input_confirm_keys.rs`, `queue_scope.rs`
- [ ] 2.4 `library_load_actions.rs`, `lib_event_actions.rs`, `run_loop_events_session.rs`
- [ ] 2.5 `ws_event_actions.rs`, `run_loop_drains.rs`, `mod.rs`, `render/mod.rs`
- [ ] 2.6 `shared_sync.rs`, `context_menu_actions.rs`, `artist_header_actions.rs`, `shuffle_folder_actions.rs`, `session_command_actions.rs`, `render/overlays/library_routes.rs`
- [ ] 2.7 `notify_actions.rs` (rescan + route-conflict call → Error), `audio_subtitle_actions.rs`, `feed_actions.rs`

## 3. Cleanup

- [ ] 3.1 Delete `flash_status` and `flash_status_high`
- [ ] 3.2 Update existing assertions to the new model (incl. the 3 `"Playing on remote"` assertions in `action_tests.rs`) and the stale bell comment in `actions_tests_routes.rs`; no new tests

## 4. Verification

- [ ] 4.1 `rtk cargo check` and `rtk cargo clippy --workspace --all-targets` clean
- [ ] 4.2 `rtk make check-code-file-lines` passes
- [ ] 4.3 Scoped search: no `flash_status`/`flash_status_high` callers remain; no bell call on the toast path
