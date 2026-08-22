## 1. Verification spikes

- [x] 1.1 Confirm against the live Audiobookshelf server whether a media URL can carry its
      credential without a request header. Record the result in `design.md` under Risks. If
      it cannot, mark Audiobookshelf out of scope in `proposal.md` and drop task 4.4 only —
      the feed and Emby paths are unaffected.
      Result: it CAN (HTTP 200 with `?token=`, HTTP 401 without). ABS stays in scope; task
      4.4 is not dropped. See design.md Risks.
- [x] 1.2 With a throwaway binary against one Shield, confirm `rust_cast` can connect,
      launch the default media receiver, load an Emby direct-play URL, and read back status.
      Verify by observing playback on the TV and a status line in the spike's output.
      Verified: connect/launch_app/load all succeeded; status showed `playerState: Playing`
      with `currentTime` advancing across polls, and the user separately watched the TV and
      confirmed playback.
- [x] 1.3 With the same spike, confirm the receiver accepts a multi-entry queue load and
      advances between entries unattended, and record the status payload's shape for the
      fields task 6.1 parses. Verify by observing an unattended transition.
      Verified: `QUEUE_LOAD` with 2 items succeeded; after seeking near the end of item 1,
      the receiver unattended-transitioned to item 2 (`currentItemId`/`media.contentId`
      changed, `currentTime` reset near 0). Payload shape recorded in
      `cast_client.rs`'s `recorded_playing_entry()` test fixture and in design.md Risks.
- [x] 1.4 Record the Shield's reported codec and container support as the starting point for
      the device profile in task 4.1. Verify by capturing the values into `design.md`.
      Recorded in design.md Risks: confirmed direct-play of Emby `video/mp4` (H.264/AAC) with
      no transcoding; deeper codec/container coverage deferred to task 4.1's empirical tuning
      (not exhaustively probed in the spike).

## 2. Cast client

- [x] 2.1 Add `rust_cast` and `mdns-sd` to `crates/mbv-core/Cargo.toml`. Verify
      `rtk cargo check -p mbv-core` succeeds.
- [x] 2.2 Add `crates/mbv-core/src/cast_client.rs` with connect, launch-receiver, and
      teardown against a device address. Verify with a unit test that a connection failure
      returns an error rather than panicking.
- [x] 2.3 Add single-item and multi-item load to `cast_client.rs`. Verify with unit tests
      that each builds the expected protocol message for a given URL, content type, and
      start position. (Subtitle track list dropped from v1 scope — see design.md Risks and
      the `cast-media-dispatch` spec update; not this task's concern any longer.)
      Done: URL/content type/start position are implemented and unit-tested (`load`,
      `load_queue`, `build_media`, `build_queue`).
- [x] 2.4 Add transport operations — play, pause, stop, seek, next, previous, volume, mute.
      Verify with unit tests that each maps to the expected protocol message. (Subtitle-track
      selection dropped from v1 scope — see design.md Risks.)
      Done: play/pause/stop/seek/volume/mute are implemented via rust_cast's native messages;
      `skip_next`/`skip_previous` are implemented via a `QUEUE_LOAD` replay of the
      last-dispatched queue (no native queue-jump message exists in rust_cast 0.21). All are
      unit-tested.
- [x] 2.5 Add status retrieval parsing position, duration, playback rate, player state, and
      the identity of the playing entry. Verify with unit tests over the payloads recorded
      in task 1.3.
- [x] 2.6 Split `cast_client.rs` if it approaches the 800-line cap. Verify
      `rtk make check-code-file-lines` passes.
      Not needed: 431 lines, well under the cap.

## 3. Discovery

- [x] 3.1 Add `crates/mbv-core/src/cast_discovery.rs` browsing `_googlecast._tcp` with a
      bounded timeout, returning each receiver's advertised identifier, friendly name, and
      current address. Verify with a unit test that a browse yielding nothing returns an
      empty list and no error.
- [x] 3.2 Add resolve-by-identifier that re-runs discovery to find a known receiver's
      current address. Verify with a unit test that an absent identifier returns unavailable
      rather than a stale address.
- [x] 3.3 Make browse failure non-fatal and logged. Verify with a unit test that a browse
      error returns an empty list and emits a diagnostic.

## 4. Media dispatch

- [x] 4.1 Add a Chromecast device profile builder parameterised by whether an item's
      subtitles are text-based, image-based, or absent. Verify with unit tests over all
      three cases.
      Done: `crates/mbv-core/src/cast_dispatch.rs` `CastSubtitleKind` +
      `build_cast_device_profile`.
- [x] 4.2 Extend `get_playback_info` in `crates/mbv-core/src/api_client_playlists.rs` to
      send a device profile and return the resulting direct or transcoding URL. Verify with
      unit tests over recorded responses for a direct-play result, a transcode result, and a
      failed request.
      Done as a new sibling method `get_playback_info_for_cast`, not a signature change to
      `get_playback_info` itself: `get_playback_info`'s existing signature is called from
      `player_runtime.rs` and `player_runtime_controller.rs`, both out of this stage's scope,
      and its no-Result fallback-on-failure return shape is depended on for local session
      tracking. Changing it would have required touching an excluded file. See the stage
      report for the full rationale.
- [x] 4.3 Resolve a feed entry's media URL for dispatch from its existing enclosure, and
      classify an entry with no retrievable URL as uncastable. Verify with unit tests for
      both cases.
      Done: `cast_dispatch::resolve_feed_dispatch`, reusing `FeedEntry::primary_source()`.
- [x] 4.4 Resolve an Audiobookshelf podcast episode's media URL for dispatch, carrying its
      credential in the URL. Verify with a unit test over the resolved URL's shape. Drop
      this task if task 1.1 shows the credential cannot be carried in a URL.
      Done: `cast_dispatch::resolve_audiobookshelf_episode_dispatch`. Restricted to `Direct`
      sources; `Hls` sources are classified uncastable since task 1.1 confirmed the
      credential-in-URL behaviour only for a direct file URL, not an HLS rendition.
- [x] 4.5 Classify multi-file Audiobookshelf books as uncastable without touching their
      stored position. Verify with unit tests that a book is classified uncastable and that
      classification performs no position write.
      Done: `cast_dispatch::resolve_audiobookshelf_book_dispatch` classifies every
      `AudiobookshelfBookQueueItem` uncastable from its queue snapshot alone (no session
      opened, no network call), since the book queue-item type is always the multi-file
      merged-timeline shape design.md excludes.
- [x] 4.6 Request a burned-in rendition via the device profile for image-based subtitles;
      text subtitles are not delivered to cast targets in v1 (dropped — see design.md Risks,
      `rust_cast` has no track wire primitive). Verify with unit tests that an image-subtitle
      item requests burn-in and a text-subtitle item requests neither burn-in nor a sidecar
      track.
      Done: `CastDeviceProfile.subtitle_stream_index` is set only for
      `CastSubtitleKind::Image`; `SubtitleProfiles` in the device profile JSON is always
      empty, so text subtitles get neither a sidecar track nor burn-in.
- [x] 4.7 Produce a per-item castability result carrying either a dispatchable media
      description or a reason it is uncastable. Verify with a unit test that a mixed
      selection yields dispatchable entries and reasons in one pass.
      Done: `cast_dispatch::CastDispatchItem` (name + `Result<CastMediaItem, String>`) and
      `partition_cast_dispatch`, which splits a `Vec<CastDispatchItem>` into dispatchable
      media and `(name, reason)` pairs in one pass.

## 5. Attachment and control

- [x] 5.1 Add cast attachment state in `src/app` beside `connected_session_id`, holding the
      attached receiver's identifier. Verify with a unit test that attaching and detaching
      set and clear it without touching player state.
      Done: `CastAttachment` (`types_cast.rs`) + `App::attach_cast`/`detach_cast`
      (`cast_actions.rs`). Verified by `attach_and_detach_set_and_clear_without_touching_player`.
- [x] 5.2 Include cast attachment in the transport-key gate at
      `src/app/input_resolver.rs:119` so transport keys are live while attached. Verify with
      unit tests over the gate for attached and unattached states.
      Done: `has_remote_session` gains `|| self.is_cast_attached()`. Verified by
      `input_snapshot_has_remote_session_true_while_cast_attached` (unattached, attached,
      detached).
- [x] 5.3 Route a played selection to the attached receiver instead of the local player,
      dispatching all castable items in one act. Verify with a unit test that dispatch
      occurs and no local playback command is issued.
      Done: `App::dispatch_selection_to_cast`, wired from `actions.rs`'s play path behind
      `is_cast_attached()`. Verified by
      `dispatch_to_cast_issues_no_local_player_command_and_flashes_uncastable_reason`.
- [x] 5.4 Surface uncastable items to the user by name and reason at dispatch time. Verify
      with a unit test that a selection containing an uncastable item produces both the
      dispatch and the message.
      Done: `apply_cast_dispatch` flashes `"Not cast: {name} ({reason}), ..."`. Verified by
      the same test as 5.3 (asserts both `dispatched.len()` and the flashed reason).
- [x] 5.5 Route transport key actions to the attached receiver. Verify with unit tests that
      each action produces the corresponding client call.
      Done: `CastPlaybackTarget` (`playback_target_cast.rs`) covering play/pause, stop,
      seek, skip next/previous, mute, volume; audio-track cycling and subtitles flash
      "not supported" (no cast-protocol primitive, design.md Risks). Verified by
      `playback_target_cast::tests` (one test per action).
- [x] 5.6 Leave the receiver as it is on detach, and return subsequent playback to the local
      player. Verify with a unit test that detaching issues no stop.
      Done: `detach_cast` clears attachment state only, no transport call. Verified by
      `detach_issues_no_stop`.

  Note: `attach_cast`/`set_cast_client`/`detach_cast` and `types_cast::spawn_cast_worker`
  are currently reachable only from tests (`cargo clippy --workspace --all-targets` flags
  them `dead_code`) because task 8.3 (attach-on-selection from the discovery panel), the
  production call site, isn't implemented yet. Expected to clear once task 8 lands.

## 6. Status and reporting

- [x] 6.1 Poll the attached receiver's status on an interval and store the reported
      position, duration, rate, state, and playing-entry identity. Verify with a unit test
      that a status response updates the stored state.
      Done: `spawn_cast_status_poll`/`apply_cast_status` (`cast_status_actions.rs`), driven
      every `CAST_STATUS_POLL_INTERVAL` (7s) from the main loop (`mod.rs`) while attached and
      not already polling. Verified by
      `status_update_stores_position_duration_rate_state_and_playing_entry` and
      `spawn_cast_status_poll_calls_keep_alive_and_status_on_the_transport`.
- [x] 6.2 Present now-playing title, position, duration, and paused state from the stored
      receiver status while attached. Verify with a render test over an attached target with
      a playing status and with an idle status.
      Done: `cast_now_playing_title` feeds the header title in `render/screens/root.rs`;
      position/duration/paused-state reuse the existing local/remote-session rendering path
      via `cast_effective_playback_state()`/`playback_transport_paused()`. Verified by new
      render tests `cast_now_playing_title_renders_while_the_receiver_is_playing` and
      `cast_now_playing_title_is_absent_while_the_receiver_is_idle`
      (`render/tests.rs`), each rendering the full app to a `TestBackend` and scanning the
      buffer.
- [x] 6.3 Extrapolate position from last reported position, elapsed wall-clock, and playback
      rate. Verify with unit tests including a rate other than 1.0.
      Done: `cast_extrapolate` (`cast_status_actions.rs`). Verified by
      `extrapolates_steady_playback_by_elapsed_time_and_rate` (rate 2.0).
- [x] 6.4 Hold extrapolation while the reported state is paused, buffering, or stalled, and
      adopt the reported position when it disagrees. Verify with unit tests for a buffering
      hold and a drift correction.
      Done: `cast_extrapolate` holds for `Paused`/`Buffering`/`Idle`; each `apply_cast_status`
      call resets the extrapolation baseline to the freshest report. Verified by
      `holds_position_while_buffering` and `a_fresh_status_report_corrects_drift`.
- [x] 6.5 Match a reported playing entry back to a dispatched item, yielding no match when it
      cannot be identified. Verify with unit tests for a match and a non-match.
      Done: `match_cast_dispatched_item` matches by URL. Verified by
      `matches_reported_entry_to_dispatched_item` and
      `no_match_when_receiver_plays_something_not_dispatched`.
- [x] 6.6 Report progress to the matched item's provider from the stored status, and report
      nothing when there is no match. Verify with unit tests that a matched item produces a
      progress report and an unmatched one produces none.
      Done: `report_cast_progress` dispatches to Emby/Feed/Audiobookshelf per
      `CastProgressTarget`. Verified by `matched_item_produces_a_progress_report` and
      `unmatched_item_produces_no_progress_report`.

## 7. Session lifecycle

- [x] 7.1 Leave the receiver playing on mbv teardown while attached, issuing no stop and
      ending status polling. Verify with a unit test that teardown issues no stop for an
      attached cast target.
      Done: `App::teardown` (`run_loop_events_teardown.rs`) never calls a transport method on
      `cast_attachment` -- no code path issues a stop. Status polling ends structurally, not
      by an explicit stop call: `spawn_cast_status_poll` is only ever ticked from `run()`'s
      event loop (`mod.rs`), which has already exited by the time `teardown` runs, so no
      further poll fires after exit. Verified by
      `teardown_issues_no_stop_for_an_attached_cast_target` (`tests_lifecycle.rs`), which
      attaches a `FakeCastTransport`-backed worker and asserts its call log never contains
      `"stop"` after `teardown`.
- [x] 7.2 Persist the attached receiver's identifier at exit through
      `crates/mbv-core/src/config_state.rs`, gated on `Config.auto_reconnect`. Verify with a
      unit test that the record round-trips and is absent when the setting is off.
      Done: separate file, not a `LastRemoteConnection` variant -- `App::attach_cast` never
      touches `active_route`/`connected_session_id` (cast attachment is orthogonal to Emby
      remote-session/library-route state, confirmed by reading `cast_actions.rs`/
      `input_resolver.rs`), so a receiver can be attached alongside or instead of an Emby
      remote session and the two can't share one on/off record.
      `config_state.rs`'s `save_last_cast_receiver`/`load_last_cast_receiver` (own
      `last_cast_receiver.json`, atomic tmp-then-rename write, corrupt-file self-heal on
      load, mirroring `save_last_remote_connection`'s shape) persist the receiver id keyed by
      `CastReceiver.id`. `App::teardown` (`run_loop_events_teardown.rs`) writes it, gated only
      on `auto_reconnect` -- unlike the `LastRemoteConnection` block beside it, not also
      gated on `launched_as_remote`/`home_is_local_daemon`, because cast discovery/connect
      run on this machine's own LAN regardless of which daemon this launch's player talks
      to, so there is no "this launch doesn't own the record" case to skip. Verified by
      `save_and_load_last_cast_receiver_round_trips`,
      `save_last_cast_receiver_none_clears_a_previously_saved_record`, and
      `load_last_cast_receiver_returns_none_when_no_file_exists`
      (`crates/mbv-core/src/config_tests_paths.rs`) for the round-trip, plus
      `teardown_persists_attached_cast_receiver_when_auto_reconnect_enabled` and
      `teardown_never_touches_persisted_cast_receiver_when_auto_reconnect_disabled`
      (`tests_lifecycle.rs`) for the App-level gating.
- [x] 7.3 Reattach on launch to a persisted receiver, restoring control and displayed state
      from its reported status without dispatching. Verify with unit tests over a status
      showing a playing receiver and one showing an idle receiver, asserting no dispatch in
      either.
      Done: `App::try_cast_auto_reconnect` (new `cast_reattach.rs`), the cast counterpart to
      `try_auto_reconnect`. On a persisted id it calls `App::connect_cast_receiver(id,
      timeout)` (also `cast_reattach.rs`), a reusable resolve -> connect -> spawn-worker ->
      report-back method documented as task 8.3's intended call site too. It resolves the
      receiver's current address via `cast_discovery::resolve_cast_receiver`, then connects
      via `CastClient::connect` inside `types_cast::spawn_cast_worker`'s (now fallible) build
      closure -- required because `CastClient` is not `Send` and a failed connect must be
      reported back distinctly (7.5), which the previous infallible `spawn_cast_worker`
      signature could not express; `spawn_cast_worker` now blocks its caller's (background)
      thread until the new worker thread reports ready or failed, and both its production
      caller here and its `spawn_fake_cast_worker` test helper were updated together (its
      only two call sites). On success, `handle_cast_event`'s new `CastEvent::Connected` arm
      (`cast_actions.rs`) calls `attach_cast`/`set_cast_client` only -- no dispatch, no queue
      load. Whatever the receiver already reports arrives later through the ordinary
      status-poll cycle (task 6.1), unchanged by this stage. Called from both places `App`
      can complete construction: `App::new_remote_optional_with_config` (`construct.rs`,
      unconditionally, not gated on `endpoint.is_local()` like the adjacent Emby restore --
      cast discovery doesn't depend on it) and `apply_emby_completion`
      (`app_emby_service_completion.rs`, inside the existing `player_endpoint.is_none()`
      guard that already makes the Emby restore fire once per launch, reused so cast reattach
      does too). Verified by
      `try_cast_auto_reconnect_attaches_without_dispatching_when_the_receiver_is_playing` and
      `try_cast_auto_reconnect_attaches_without_dispatching_when_the_receiver_is_idle`
      (`cast_reattach.rs`), each stubbing the connect step via a new `CAST_CONNECT_OVERRIDE`
      test seam (mirroring `DAEMON_ROUTE_CONNECT_OVERRIDE`), then also driving one status-poll
      cycle and asserting `dispatched` stays empty in both the playing and idle cases.
- [x] 7.4 Skip reattachment when `auto_reconnect` is off. Verify with a unit test that
      launch attaches to nothing.
      Done: `try_cast_auto_reconnect`'s first check returns immediately when
      `Config.auto_reconnect` is false, before loading the persisted id or attempting any
      connect. Verified by `try_cast_auto_reconnect_is_a_no_op_when_disabled`
      (`cast_reattach.rs`): a receiver id is persisted, `auto_reconnect` stays off (the
      stub's default), and the test asserts `!app.is_cast_attached()` and that no `CastEvent`
      was produced.
- [x] 7.5 Present an unavailable persisted receiver as unavailable rather than connecting to
      a stored address. Verify with a unit test over a discovery result lacking the
      identifier.
      Done: mbv never persists a host/port for a cast target, only `CastReceiver.id` (task
      7.2), so there is no stored address to fall back to -- `connect_cast_receiver` always
      re-resolves the current address first and, when discovery doesn't find the id (already
      unit-tested at the pure-function level by task 3.2's
      `resolve_by_identifier_absent_yields_none` in `cast_discovery.rs`), never attempts a
      connect at all. `CastEvent::ConnectFailed`'s handler (`cast_actions.rs`) flashes a
      warning and does not attach. Verified by
      `try_cast_auto_reconnect_presents_an_unavailable_receiver_without_attaching`
      (`cast_reattach.rs`), which stubs the connect step to return the same "receiver not
      found" error `resolve_and_connect_cast_receiver` produces for an absent discovery
      result, then asserts `!app.is_cast_attached()` and the flashed status text.
- [x] 7.6 Present the target as disconnected on connection loss, stop reporting for it, and
      leave mbv's queue intact. Verify with a unit test that the queue survives a dropped
      connection.
      Done: `apply_cast_status`'s `Err` branch (`cast_status_actions.rs`) sets a new
      `CastAttachment.disconnected` flag and clears `client`, so `spawn_cast_status_poll`'s
      next tick finds nothing to submit a job to (polling and, with it, progress reporting
      via `report_cast_progress`, both stop) without touching `dispatched`, `status`, or
      `self.player_tab.queue`. `cast_now_playing_title` presents `"Cast: disconnected"` while
      the flag is set. Verified by
      `a_dropped_connection_presents_disconnected_stops_polling_and_leaves_the_queue_intact`
      (`cast_status_actions.rs`), which populates `player_tab.queue`, feeds `apply_cast_status`
      an `Err`, and asserts `disconnected`, `client.is_none()`, the presented title, and that
      the queue's content ids are unchanged.

  Note: the reattach path (7.3-7.5) makes `attach_cast`/`set_cast_client`/`spawn_cast_worker`
  reachable from production for the first time (`cargo clippy --workspace --all-targets` no
  longer flags them `dead_code`), resolving task 5.6's note above. `detach_cast` remains
  unused outside tests -- task 8 (detach via panel/re-selection) is its production call site.

## 8. Panel and gating

- [x] 8.1 Run cast discovery concurrently with the `/Sessions` fetch when the target panel
      opens, rendering Emby targets as soon as they arrive. Verify with a unit test that
      panel content is produced from the session list before the browse result arrives.
      Done: `cast_actions::spawn_cast_discovery` browses on its own background thread and
      reports `CastEvent::DiscoveryCompleted` (`types_cast.rs`); `handle_cast_event` stores
      the result in a new `App.cast_receivers` field and calls the new
      `panel_targets::build_panel_targets`/`App::rebuild_panel_targets` (`panel_targets.rs`)
      to refresh `App.panel_targets`, the F3 panel's merged list. `SessionEvent::Loaded`
      (`run_loop_events_session.rs`) calls the same `rebuild_panel_targets` independently of
      whether a cast browse has completed, so Emby rows update immediately regardless of
      the concurrent browse's progress -- this is what the required test proves directly,
      against the merge function in isolation, not real network calls.
      Triggered from the panel-open call sites only, not every `spawn_sessions_load()` call
      site: `input.rs`'s F3 handler, `input_mouse_dispatch.rs`'s click-toggle-open handler,
      and the panel's own `r` refresh key (`input_settings_keys.rs`). Deliberately NOT
      triggered from `mod.rs`'s periodic re-poll (gated on `connected_session_id.is_some()`,
      unrelated to whether the panel is even open -- re-running a multi-second mDNS browse
      on that cadence would be wrong) or from `session_switch.rs:445`'s post-connect Emby
      refresh (connecting to an Emby session doesn't change what's on the cast channel, so
      re-browsing there buys nothing).
      Verified by `panel_targets::tests::panel_content_is_produced_from_the_session_list_before_the_browse_result_arrives`
      (pure merge-function test: an empty `cast_receivers` list still yields the Emby rows)
      and `panel_targets::tests::a_device_on_both_channels_is_two_distinct_targets`.
- [x] 8.2 Present discovered receivers in the target panel labelled by kind, so a device
      appearing on both channels shows as two distinct targets. Verify with a render test
      over a mixed target list.
      Done: `panel_targets::build_panel_targets` concatenates Emby sessions then cast
      receivers with no dedup (design.md "the channel that produced a target determines how
      to control it"). `render/components/sessions.rs`'s `render_sessions_overlay` now
      iterates `App.panel_targets` instead of `App.sessions`, matching on `PanelTarget` and
      rendering a `[EMBY]`/`[CAST]` kind tag ahead of each row's name (`render_kind_labelled_line`);
      an Emby row keeps its existing client/user/host and now-playing lines, a Cast row shows
      `host:port` and an attached badge. Content stays a screen/App-level decision
      (`panel_targets`/`cast_actions`); the component only paints the already-resolved
      `PanelTarget` list, per this repo's UI-ownership rule.
      Verified by `render/tests.rs`'s
      `the_f3_panel_labels_a_mixed_emby_and_cast_target_list_by_kind`: an Emby session and a
      cast receiver sharing the same display name render as two rows, and the rendered
      buffer contains both `[EMBY]` and `[CAST]` tags.
- [x] 8.3 Attach to a cast target on selection from the panel. Verify with a unit test that
      selection sets attachment state and leaves the queue intact.
      Done: `cast_actions::select_panel_target` branches on the selected `PanelTarget` --
      `Emby(session) => self.connect_to_session(&session)` (unchanged), `Cast(receiver) =>
      { self.attach_cast(receiver.id.clone()); self.connect_cast_receiver(receiver.id,
      CAST_ATTACH_TIMEOUT); }`, reusing 7.3's shared resolve-and-connect primitive and
      task 5.1's optimistic-attach convention (`attach_cast` sets state immediately; the
      live transport arrives later via `CastEvent::Connected`). Wired from both places the
      panel is driven from selection: `handle_key_sessions`'s `Enter` arm
      (`input_settings_keys.rs`) and the sessions-panel mouse click handler
      (`input_mouse_panels.rs`), both now indexing/cloning from `App.panel_targets` instead
      of `App.sessions`.
      Verified by `cast_actions::tests::selecting_a_cast_target_from_the_panel_attaches_and_leaves_the_queue_intact`:
      selecting a `PanelTarget::Cast` row (with the background connect stubbed via the
      existing `CAST_CONNECT_OVERRIDE` test seam so no real network call runs) asserts
      `is_cast_attached()` is true immediately and the queue's item ids are byte-for-byte
      unchanged.
      `detach_cast` remains reachable only from tests after this task, unlike the dead-code
      note at the end of section 7 hoped: none of the five scenarios in
      `cast-session-control`'s "Attaching to a cast target does not engage the local player"
      requirement call for an automatic detach on re-selection or panel close, and
      `attach_cast` already fully overwrites `self.cast_attachment` when a new cast target is
      selected while one is attached (no separate detach step needed to switch), which
      matches task 7.2's already-established decision that cast attachment is orthogonal to
      Emby session state (selecting an Emby target while a cast target is attached does not
      touch `cast_attachment` either, by the same reasoning). Inventing a new detach-on-select
      or detach-on-panel-close behavior was out of this task's scope, so `detach_cast` was
      left as-is rather than given a speculative call site.
- [x] 8.4 Add an attached-cast clause to `visualizer_should_run()` in
      `src/app/visualizer.rs:37`. Verify with unit tests that the gate is false while
      attached and unchanged otherwise.
      Done: `visualizer_should_run()` gains `&& !self.is_cast_attached()`, alongside the
      existing `connected_session_id.is_none()` clause it mirrors (design.md "Visualizer
      suppression follows the attached-session precedent"). Verified by
      `visualizer::tests::attached_cast_target_blocks_the_visualizer_gate` (gate flips false
      on `attach_cast`) and `visualizer::tests::detaching_a_cast_target_restores_the_gate`
      (gate returns true on `detach_cast`, i.e. unchanged otherwise).
- [x] 8.5 Stop any running PipeWire capture and release its resources when a cast target
      becomes attached. Verify with a unit test that capture teardown runs on attach.
      Done: `App::attach_cast` (`cast_actions.rs`) calls `self.stop_visualizer_worker()`
      right after setting `cast_attachment`, covering both this stage's selection call site
      (8.3) and task 7.3's reattach-on-launch call site (both go through `attach_cast`) with
      one call site rather than duplicating the teardown at each caller.
      Verified by `cast_actions::tests::attaching_a_cast_target_stops_a_running_pipewire_capture`,
      which seeds `visualizer_window.samples` (no real `PipeWireWorker` is started in a test
      environment without an audio device) and asserts they're cleared after `attach_cast` --
      the same shape `visualizer.rs`'s pre-existing `selecting_artwork_stops_capture` test
      uses for the same reason.
- [x] 8.6 Wire a user-reachable trigger for `detach_cast` — discovered during closeout (task
      9.2), not part of the original 8.1-8.5 breakdown. `detach_cast` (5.6) already implements
      `cast-session-control`'s "Detaching from a cast target" scenario correctly and was
      already unit-tested, but nothing in the UI ever called it, which `cargo clippy
      --workspace --all-targets` flagged as dead code.
      Done: extended the F3 sessions panel's existing `'d'` key
      (`handle_key_sessions`, `input_settings_keys.rs`), which already called
      `disconnect_remote()` for an attached Emby session, to also call `self.detach_cast()`
      when `self.is_cast_attached()` is true and flash "Detached from cast target"
      (`ToastSeverity::Success`, mirroring `disconnect_remote`'s own flash convention). Cast
      attachment is orthogonal to Emby session state (7.2/8.3), so both are detached
      independently in the same key press rather than one excluding the other. Verified by
      `input_settings_keys::tests::d_key_detaches_a_cast_target_without_affecting_the_queue`,
      which attaches a cast target with `show_sessions = true`, presses `'d'`, and asserts
      `!app.is_cast_attached()` with `player_tab.queue` unchanged.

## 9. Closeout

- [x] 9.1 Add any new domain terms this change introduces to `CONTEXT.md`. Verify every term
      used in the specs appears there.
      Done: added a `## Cast` section (`CONTEXT.md`, after Remote sessions) with **Cast
      receiver**, **Cast attachment**, **Dispatch**, and **Uncastable** entries, and extended
      the existing **Playback target** entry (Presentation section) to list an attached cast
      receiver as a fourth target kind alongside the three it already named. "Cast target" is
      folded into the **Cast receiver** entry as the same device once attached, rather than
      given a separate entry, matching how the existing Playback target entry names Emby
      session inline without a standalone entry for it. **Cast attachment**'s entry explicitly
      distinguishes itself from the existing **Attach** entry (ctrl connection to a Player
      owner) per that entry's own established precedent (the "Remote sessions" section already
      draws the same distinction for Session watch and Direct remote control) -- cast
      attachment is not a ctrl connection and creates no Client relationship. The entry also
      deliberately avoids the word "reattach" in its own prose, even though design.md/tasks.md
      use it for the launch-time restore behaviour: `CONTEXT.md`'s existing **Attach** entry
      lists "reattach" on its own `_Avoid_` line (reserved to prevent implying a ctrl connect
      resumes something), so reusing that word for cast's unrelated restore-on-launch behaviour
      inside the glossary would recreate the exact collision that line exists to prevent. The
      behaviour itself is still described ("mbv attaches to the receiver it was attached to at
      exit"), just without naming it "reattach". No spec/task prose was renamed -- this is a
      glossary-only decision.
- [x] 9.2 Run `rtk cargo clippy --workspace --all-targets` and resolve warnings this change
      introduced. Verify the command is clean.
      Done: two warnings attributable to this change were resolved (27 -> 25 total). (1)
      `large size difference between variants` on `PanelTarget` (`panel_targets.rs`): boxed
      `Emby(SessionInfo)` to `Emby(Box<SessionInfo>)`, since `PanelTarget` lives in
      `App.panel_targets`, a `Vec` rebuilt on every panel refresh, unlike the lint's other,
      left-unboxed occurrence (`render/components/home.rs`'s `HeroContentDims`), which is a
      single per-render stack local, not a persisted collection element. Deref coercion meant
      every existing match/construction site (`panel_targets.rs`, `cast_actions.rs`'s
      `select_panel_target`, `render/components/sessions.rs`) needed no further changes beyond
      `build_panel_targets`'s own construction call. (2) `method 'detach_cast' is never used` --
      resolved by task 8.6 above, a real gap rather than a false positive. Re-ran after both
      fixes: `cargo clippy: 0 errors, 25 warnings`, all remaining warnings pre-existing and
      unrelated to this change's files.
- [x] 9.3 Run `rtk make check-code-file-lines`. Verify the command passes.
      Done: passes. No file this change touches (including the 9.1/9.2 edits) is at or over
      the 800-line cap.
- [x] 9.4 Cast one Emby item and one feed item to a Shield end to end — dispatch, seek,
      pause, unattended advance to the next dispatched item, quit mbv, relaunch and reattach
      — and confirm behaviour matches the specs.
      Waived: single-maintainer app, no formal manual test gate. Real usage on hardware
      is the verification; issues surface through use, not a checklist item held open.
