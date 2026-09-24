# Tasks

## 1. Build hygiene: take test-support off the release path

- [x] 1.1 In the workspace-root `Cargo.toml`, drop `features = ["test-support"]` from
  the `[dependencies]` entry for `mbv-core` (line 42) and add
  `mbv-core = { path = "crates/mbv-core", features = ["test-support"] }` to
  `[dev-dependencies]`. Verify: `cargo build --release -p mbv` succeeds and
  `nm -C target/release/mbv | grep -E 'mock_http::|connect_stub_daemon_pair'` returns
  nothing (it currently returns 4 symbols, including a spawned thread closure).
- [x] 1.2 Verify the feature is still visible to the test build that needs it:
  `cargo nextest run -p mbv` compiles and passes, and
  `cargo tree -p mbv -e features` shows `test-support` on `mbv-core`. Do not touch
  `crates/mbv-core/Cargo.toml` (it keeps declaring the feature) or `crates/mbvd`
  (it never used it).

## 2. `single_instance` resolution

- [x] 2.1 Add a test module to `src/single_instance.rs` covering `Resolution::Fresh`
  and the PID round-trip: `resolve` against a lock path in a temp dir returns `Fresh`,
  and `write_pid` then `read_pid` returns the process id. Verify with
  `cargo nextest run -p mbv -E 'test(single_instance)'`.
- [x] 2.2 Cover the held-lock arms in the same module: with a guard from a first
  `resolve` still alive, a second `resolve` returns `Refuse` when the socket path has
  no listener, and `Attach` when a `UnixListener` is bound at that path. Verify the
  same nextest filter, and confirm `read_pid` on a missing/garbage lock file returns
  `None`.

## 3. Loop-step drains

- [x] 3.1 Cover `App::drain_notif_actions` in `src/app/shell_run_tests.rs`: `"clear:yes"`
  dismisses and routes the clear-queue action, `"__notif_failed__"` sets
  `notif_failed`, an unrecognised string and an empty channel both produce no action.
  Assert the returned `produced` value in each case.
- [x] 3.2 Cover `App::drain_session_events`: a queued `SessionEvent` is dispatched and
  `produced` is true; an empty channel leaves `produced` false.
- [x] 3.3 Cover `App::drain_audiobookshelf_events` receiver bookkeeping: an `Empty`
  startup/test receiver is put back in place, a `Disconnected` receiver drives
  `handle_audiobookshelf_worker_disconnect` and reports `produced`, and the setup and
  catalog receivers behave the same for their own disconnect handlers.
- [x] 3.4 Cover the catalog completion gate and failures: a completion whose generation
  is not accepted is dropped without touching browse state, an
  `AuthenticationRejected` completion moves the runtime to `NeedsAuthentication` and
  clears credentials, and a generic error leaves browse state untouched.
- [x] 3.5 Cover the catalog success path under the stub config (`Config::default()`, no
  Audiobookshelf setup, so the spawned fetchers do no I/O and exit): `catalog_ready`
  is set, `audiobookshelf_browse` / `audiobookshelf_book_browse` are built for every
  library, and podcast versus book progress lands in the matching map. Additionally
  read `lib_tx` to assert that a podcast library requested shows + shelves and a book
  library requested books — this is the per-kind dispatch decision.
  Verify 3.1-3.5 with `cargo nextest run -p mbv -E 'test(drain)'`.

## 4. `WsEvent` dispatch

- [x] 4.1 Add a `#[rstest]` `#[case]` table to `src/app/ws_event_actions.rs` covering
  the pure command variants — `Pause`, `Unpause`, `NextTrack`, `PreviousTrack`,
  `TogglePause`, `Seek`, `SeekRelative`, `SetVolume`, `VolumeUp`, `VolumeDown`,
  `SetAudio` — asserting the exact `PlayerCommand` observed through
  `PlayerProxy::spy_on_commands()` on a `make_app_stub()` app.
- [x] 4.2 Cover the variants with their own state: `Stop` resets bare transitions and
  stops the player; `SetMute` / `ToggleMute` update `mute_on` *and* send
  `SetMute`, with `save_prefs` landing in the test state dir (the `TestStateDirGuard`
  installed by `make_app_stub` keeps this off the developer's config).
- [x] 4.3 Cover `SetSub` on its own: it resolves the stream index through player status,
  so assert both the resolved-command case and the no-command case rather than forcing
  it into the 4.1 table.
- [x] 4.4 Cover the `Play` arm with the existing mock-Emby fixture: single-item and
  multi-item plays assert the queue replacement, the `Remote` queue source, the
  honoured `start_position_ticks`, and the persisted queue state.
- [x] 4.5 Cover `UserDataChanged`: a successful home fetch sends
  `LibEvent::HomeContentRefreshed` on `lib_tx`, and a failed fetch sends nothing.
  Verify 4.1-4.5 with `cargo nextest run -p mbv -E 'test(ws_event)'`.

## 5. Audiobookshelf book playback session

- [x] 5.1 Add a book playback-session fixture under
  `crates/mbv-core/tests/fixtures/audiobookshelf/`, modeled on the podcast
  `play-direct.json` / `play-transcode.json` but with `media_type: "book"` and a
  non-empty `audio_tracks` array. It must satisfy every `decode_book_playback_session`
  precondition: non-empty `id`, matching `library_item_id`, `duration` and
  `current_time` finite and non-negative.
- [x] 5.2 Cover the book success path with the existing `mock_client` / `fixture`
  helpers in `audiobookshelf_playback_tests.rs`:
  `create_book_playback_session_bounded` returns a session and the decoded fields
  (source method, tracks, duration, current time) match the fixture. This drives
  `create_book_playback_session` and `decode_book_playback_session`, which are
  call-count 0 today.
- [x] 5.3 Cover the book decode rejections: a response with the wrong `media_type`, an
  empty `audio_tracks`, a mismatched `library_item_id`, and a negative or non-finite
  `duration` / `current_time` each return a protocol error rather than a session.
  Verify 5.1-5.3 with `cargo nextest run -p mbv-core -E 'test(book_playback)'`.

## 6. Dead helper removal

- [ ] 6.1 Delete `set_tv_cursor_for_test` from `src/app/render/test_helpers.rs` (it has
  exactly one occurrence in the tree: its own definition). Its doc comment's reference
  to the already-deleted `set_browser_cursor_for_test` goes with it. Verify
  `rg -n "set_tv_cursor_for_test" --type rust` returns nothing, `tv_owner_key` is left
  intact (still used by `test_helpers_mounted.rs`), and `cargo clippy -p mbv
  --all-targets -- -D warnings` stays clean.

## 7. Verification and closure

- [ ] 7.1 Run the gates: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run -p mbv-core`, `cargo nextest run -p mbv`. All must pass.
- [ ] 7.2 Re-measure coverage with
  `cargo llvm-cov --workspace --json --output-path /tmp/mbv-cov.json -- --test-threads=4`
  and confirm `run_loop_drains.rs`, `ws_event_actions.rs`, `single_instance.rs` and the
  Audiobookshelf book path are all off zero coverage with no new gap introduced
  elsewhere. Record the before/after numbers (baseline: prod 65,186/82,966 = 78.5%,
  total 91,751/110,094 = 83.3%).
- [ ] 7.3 Update #771 with the outcome, and state there that #697's actions 1 and 2 are
  withdrawn rather than deferred (libmpv is out of test scope) and that its
  `Browser*`-dead-arm, `list_letter_groups.rs`, and "restore what `39b3fbf4` deleted"
  findings are already resolved.
