## 1. Verification spikes

- [ ] 1.1 Confirm against the live Audiobookshelf server whether a media URL can carry its
      credential without a request header. Record the result in `design.md` under Risks. If
      it cannot, mark Audiobookshelf out of scope in `proposal.md` and drop task 4.4 only —
      the feed and Emby paths are unaffected.
- [ ] 1.2 With a throwaway binary against one Shield, confirm `rust_cast` can connect,
      launch the default media receiver, load an Emby direct-play URL, and read back status.
      Verify by observing playback on the TV and a status line in the spike's output.
- [ ] 1.3 With the same spike, confirm the receiver accepts a multi-entry queue load and
      advances between entries unattended, and record the status payload's shape for the
      fields task 6.1 parses. Verify by observing an unattended transition.
- [ ] 1.4 Record the Shield's reported codec and container support as the starting point for
      the device profile in task 4.1. Verify by capturing the values into `design.md`.

## 2. Cast client

- [ ] 2.1 Add `rust_cast` and `mdns-sd` to `crates/mbv-core/Cargo.toml`. Verify
      `rtk cargo check -p mbv-core` succeeds.
- [ ] 2.2 Add `crates/mbv-core/src/cast_client.rs` with connect, launch-receiver, and
      teardown against a device address. Verify with a unit test that a connection failure
      returns an error rather than panicking.
- [ ] 2.3 Add single-item and multi-item load to `cast_client.rs`. Verify with unit tests
      that each builds the expected protocol message for a given URL, content type, start
      position, and subtitle track list.
- [ ] 2.4 Add transport operations — play, pause, stop, seek, next, previous, volume, mute,
      subtitle-track selection. Verify with unit tests that each maps to the expected
      protocol message.
- [ ] 2.5 Add status retrieval parsing position, duration, playback rate, player state, and
      the identity of the playing entry. Verify with unit tests over the payloads recorded
      in task 1.3.
- [ ] 2.6 Split `cast_client.rs` if it approaches the 800-line cap. Verify
      `rtk make check-code-file-lines` passes.

## 3. Discovery

- [ ] 3.1 Add `crates/mbv-core/src/cast_discovery.rs` browsing `_googlecast._tcp` with a
      bounded timeout, returning each receiver's advertised identifier, friendly name, and
      current address. Verify with a unit test that a browse yielding nothing returns an
      empty list and no error.
- [ ] 3.2 Add resolve-by-identifier that re-runs discovery to find a known receiver's
      current address. Verify with a unit test that an absent identifier returns unavailable
      rather than a stale address.
- [ ] 3.3 Make browse failure non-fatal and logged. Verify with a unit test that a browse
      error returns an empty list and emits a diagnostic.

## 4. Media dispatch

- [ ] 4.1 Add a Chromecast device profile builder parameterised by whether an item's
      subtitles are text-based, image-based, or absent. Verify with unit tests over all
      three cases.
- [ ] 4.2 Extend `get_playback_info` in `crates/mbv-core/src/api_client_playlists.rs` to
      send a device profile and return the resulting direct or transcoding URL. Verify with
      unit tests over recorded responses for a direct-play result, a transcode result, and a
      failed request.
- [ ] 4.3 Resolve a feed entry's media URL for dispatch from its existing enclosure, and
      classify an entry with no retrievable URL as uncastable. Verify with unit tests for
      both cases.
- [ ] 4.4 Resolve an Audiobookshelf podcast episode's media URL for dispatch, carrying its
      credential in the URL. Verify with a unit test over the resolved URL's shape. Drop
      this task if task 1.1 shows the credential cannot be carried in a URL.
- [ ] 4.5 Classify multi-file Audiobookshelf books as uncastable without touching their
      stored position. Verify with unit tests that a book is classified uncastable and that
      classification performs no position write.
- [ ] 4.6 Build text subtitles into sidecar track descriptors from the URLs
      `get_playback_info` already parses, and request a burned-in rendition for image-based
      subtitles. Verify with unit tests over an item of each kind.
- [ ] 4.7 Produce a per-item castability result carrying either a dispatchable media
      description or a reason it is uncastable. Verify with a unit test that a mixed
      selection yields dispatchable entries and reasons in one pass.

## 5. Attachment and control

- [ ] 5.1 Add cast attachment state in `src/app` beside `connected_session_id`, holding the
      attached receiver's identifier. Verify with a unit test that attaching and detaching
      set and clear it without touching player state.
- [ ] 5.2 Include cast attachment in the transport-key gate at
      `src/app/input_resolver.rs:119` so transport keys are live while attached. Verify with
      unit tests over the gate for attached and unattached states.
- [ ] 5.3 Route a played selection to the attached receiver instead of the local player,
      dispatching all castable items in one act. Verify with a unit test that dispatch
      occurs and no local playback command is issued.
- [ ] 5.4 Surface uncastable items to the user by name and reason at dispatch time. Verify
      with a unit test that a selection containing an uncastable item produces both the
      dispatch and the message.
- [ ] 5.5 Route transport key actions to the attached receiver. Verify with unit tests that
      each action produces the corresponding client call.
- [ ] 5.6 Leave the receiver as it is on detach, and return subsequent playback to the local
      player. Verify with a unit test that detaching issues no stop.

## 6. Status and reporting

- [ ] 6.1 Poll the attached receiver's status on an interval and store the reported
      position, duration, rate, state, and playing-entry identity. Verify with a unit test
      that a status response updates the stored state.
- [ ] 6.2 Present now-playing title, position, duration, and paused state from the stored
      receiver status while attached. Verify with a render test over an attached target with
      a playing status and with an idle status.
- [ ] 6.3 Extrapolate position from last reported position, elapsed wall-clock, and playback
      rate. Verify with unit tests including a rate other than 1.0.
- [ ] 6.4 Hold extrapolation while the reported state is paused, buffering, or stalled, and
      adopt the reported position when it disagrees. Verify with unit tests for a buffering
      hold and a drift correction.
- [ ] 6.5 Match a reported playing entry back to a dispatched item, yielding no match when it
      cannot be identified. Verify with unit tests for a match and a non-match.
- [ ] 6.6 Report progress to the matched item's provider from the stored status, and report
      nothing when there is no match. Verify with unit tests that a matched item produces a
      progress report and an unmatched one produces none.

## 7. Session lifecycle

- [ ] 7.1 Leave the receiver playing on mbv teardown while attached, issuing no stop and
      ending status polling. Verify with a unit test that teardown issues no stop for an
      attached cast target.
- [ ] 7.2 Persist the attached receiver's identifier at exit through
      `crates/mbv-core/src/config_state.rs`, gated on `Config.auto_reconnect`. Verify with a
      unit test that the record round-trips and is absent when the setting is off.
- [ ] 7.3 Reattach on launch to a persisted receiver, restoring control and displayed state
      from its reported status without dispatching. Verify with unit tests over a status
      showing a playing receiver and one showing an idle receiver, asserting no dispatch in
      either.
- [ ] 7.4 Skip reattachment when `auto_reconnect` is off. Verify with a unit test that
      launch attaches to nothing.
- [ ] 7.5 Present an unavailable persisted receiver as unavailable rather than connecting to
      a stored address. Verify with a unit test over a discovery result lacking the
      identifier.
- [ ] 7.6 Present the target as disconnected on connection loss, stop reporting for it, and
      leave mbv's queue intact. Verify with a unit test that the queue survives a dropped
      connection.

## 8. Panel and gating

- [ ] 8.1 Run cast discovery concurrently with the `/Sessions` fetch when the target panel
      opens, rendering Emby targets as soon as they arrive. Verify with a unit test that
      panel content is produced from the session list before the browse result arrives.
- [ ] 8.2 Present discovered receivers in the target panel labelled by kind, so a device
      appearing on both channels shows as two distinct targets. Verify with a render test
      over a mixed target list.
- [ ] 8.3 Attach to a cast target on selection from the panel. Verify with a unit test that
      selection sets attachment state and leaves the queue intact.
- [ ] 8.4 Add an attached-cast clause to `visualizer_should_run()` in
      `src/app/visualizer.rs:37`. Verify with unit tests that the gate is false while
      attached and unchanged otherwise.
- [ ] 8.5 Stop any running PipeWire capture and release its resources when a cast target
      becomes attached. Verify with a unit test that capture teardown runs on attach.

## 9. Closeout

- [ ] 9.1 Add any new domain terms this change introduces to `CONTEXT.md`. Verify every term
      used in the specs appears there.
- [ ] 9.2 Run `rtk cargo clippy --workspace --all-targets` and resolve warnings this change
      introduced. Verify the command is clean.
- [ ] 9.3 Run `rtk make check-code-file-lines`. Verify the command passes.
- [ ] 9.4 Cast one Emby item and one feed item to a Shield end to end — dispatch, seek,
      pause, unattended advance to the next dispatched item, quit mbv, relaunch and reattach
      — and confirm behaviour matches the specs.
