# Design

## Context

See proposal.md (Why) for the motivation.

The run loop (`crates/mbv-player/src/run/run_loop.rs`, `handle_mpv_event`) consumes `Result<libmpv2::Event, libmpv2::Error>` from `mpv.wait_event(0.0)`. It acts on these events only:

| Event | Consumer |
|---|---|
| `PropertyChange` for the 11 IDs in `runtime.rs` `observe_properties` | `on_property_change` |
| `PlaybackRestart` | `restart.rs` `on_playback_restart` |
| `EndFile(reason)` | `queue_advance.rs`, `termination.rs`, `event_classifiers::is_superseded_jump_end_file` |
| `LogMessage` | log forwarding with sensitive-source redaction |
| `ClientMessage` | `handle_client_message` (Lua OSC buttons) |
| `Shutdown` | `on_shutdown` |
| `Err(e)` | `termination.rs` `on_mpv_error` with `is_clocked_audio_error` |

The match ends with a `_ => false` wildcard. AGENTS.md says to replace a wildcard with explicit arms or a documented no-op.

libmpv2 6.0.0 (`src/mpv/events.rs`, `wait_event`) has two properties that the IPC path does not share:

- An `END_FILE` event with an error set comes back as `Err(Error::Raw(code))` only. The reason (`error`) is not delivered. Over IPC the same event is `{"event":"end-file","reason":"error","file_error":"<mpv_error_string>"}`.
- `EndFile` carries only the reason. The `playlist_entry_id`, `playlist_insert_id` and `playlist_insert_num_entries` fields are dropped. IPC delivers them.

mpv's IPC client thread (`input/ipc-unix.c`) creates an ordinary client handle per connection and writes each `mpv_wait_event` result as one JSON line, in order. It runs commands on the same thread, so a command reply line can arrive between event lines.

## Goals / Non-Goals

**Goals:**

- Show, per scenario, whether the IPC event sequence matches the libmpv sequence after a fixed set of normalizations.
- Leave behind real mpv event traces that unit tests can decode and feed to mbv's event classifiers with no live mpv.
- Remove libmpv2 types from the event-classification logic, which a later migration needs anyway, with no behavior change.

**Non-Goals:**

- No production IPC transport, child-process supervision or `MpvConn` type. Those belong to the migration change if the findings support it.
- No full replay of the run loop from fixtures. Run-loop handlers also issue mpv commands, so a full replay needs a command-side fake. That is migration work.
- No change to the 5 synchronous `get_property` reads. The Findings section records them as a migration input.

## Decisions

### D1. The probe is a Rust example in `mbv-core` that drives both backends

`crates/mbv-core/examples/mpv_ipc_probe/` runs each scenario through `libmpv2::Mpv` in-process, then through an `mpv` child process, and compares the two traces. `mbv-core/examples/audiobookshelf_contract_probe.rs` is the precedent. An example is a manual tool. It does not run in CI and is not a test, so the hermetic-test rule does not apply to it. The user runs it. Agents ask before they run it.

Rejected alternative: connect `socat` to the IPC socket that mbv already opens, next to a running mbv. That only shows that two clients of one core agree, which the mpv source already shows. mbv also has no raw event log to compare against.

The probe uses only the public `libmpv2` API and `serde_json`. Its normalizer lives in the example file. It does not use crate-private `MpvEvent`, so no item becomes `pub` for the example (M-INTEGRATION-TESTS forbids that).

Implementation note: the probe lives at `crates/mbv-core/examples/mpv_ipc_probe/` (`main.rs`, `backends.rs`, `compare.rs`, `scenarios.rs`), not in `mbv-player`. `mbv-player` inherits the workspace `print_stdout`/`print_stderr` deny, and a probe must print its report. `mbv-core` mirrors the lint block without those two rules for its CLI probes, and it already has `libmpv2` and `serde_json` as dev-dependencies. This change adds `libmpv2-sys` (already a workspace dependency) as an `mbv-core` dev-dependency for `mpv_error_str` and the `IDLE` event ID. The fixture header date comes from `time`, which `mbv-core` already depends on. The fixture output path is unchanged. Neither side requests log messages, because the libmpv side would need the unsafe `mpv_request_log_messages` call and rule 4 drops log messages anyway. For the 3 properties observed as `Format::String`, the IPC side uses `observe_property_string`, so both sides deliver the same string form.

### D2. The child uses `--input-ipc-server=<scratch path>`, not `--input-ipc-client=fd://`

Both modes run the same client thread in mpv, so the event semantics are the same. The server path needs no `pre_exec` or `dup2` unsafe code in the probe. The fd mode (with `quit_on_close`) is the likely production choice. The migration change decides it.

The probe starts the child with `--idle=yes --no-config --vo=null --ao=null --no-terminal`. Scenario S6 replaces the `ao` setting. The libmpv side sets the same options through the initializer. Both sides observe the same 11 properties with the same IDs as `observe_properties`.

### D3. Comparison happens at mpv's JSON event shape, after documented normalizations

The probe converts each libmpv2 event into mpv's JSON event field names (`event`, `reason`, `name`, `data`, `id`, `args`, `prefix`, `level`, `text`). `Err(Raw(code))` becomes `{"event":"end-file","reason":"error","error_code":code}`. The probe reduces each IPC line to the same field set. Normalizations, all printed in the probe output:

1. Drop what libmpv2 cannot deliver: the fields `playlist_entry_id`, `playlist_insert_id`, `playlist_insert_num_entries`, and `property-change` events with no `data` (libmpv2 `wait_event` returns `None` for a property that becomes unavailable). Record that IPC has them.
2. Map IPC `file_error` text to an error code with mpv's error-string table, through `libmpv2-sys` `mpv_error_string`.
3. Collapse consecutive `time-pos` changes into one entry and compare no `time-pos` values, because their number depends on timing.
4. Drop `log-message` events from the order comparison, because their count depends on timing.
5. Drop command reply lines (`request_id`) from the event sequence.
6. Add `"id": 0` to a `property-change` that has no `id`. mpv's JSON encoder leaves out the ID when it is 0 (`time-pos`).

Before the first step, each backend drains the initial values of its observed properties. Without this step, the first probe run showed IPC delivering those values before or after `start-file` from one run to the next. Their position depended on when the client first polls, not on the transport. The drain keeps only property changes, because the IPC client connects after mpv starts and can miss the startup `idle` event (second probe run).

The second run also showed that a property change can land before or after a nearby event from run to run on the same backend (for example `audio-codec-name` and `audio-reconfig`). So the probe also compares a split view: the order of the non-property events, plus each property's value sequence. When only that view agrees, the result is `MATCH-IGNORING-PROPERTY-INTERLEAVING`. That is a finding to judge: the run loop must not depend on where a property change falls relative to other events.

Each scenario result is `MATCH`, `MATCH-AFTER-NORMALIZATION` (it lists which rules applied) or `DIFF` (it prints the first differing index and both sides).

### D4. Scenarios come from the code paths that consume events

All media is synthetic: `av://lavfi:sine=frequency=440:duration=1` (audio) and `av://lavfi:testsrc=duration=1:size=64x64` (video). Missing files use a path under the scratch directory that does not exist.

| ID | Scenario | Code that relies on it |
|---|---|---|
| S1 | Two-item audio playlist plays to natural EOF with `gapless-audio=weak` | `queue_advance.rs` advance, `track_progress.rs` |
| S2 | Two `playlist-pos` writes in quick succession (superseded jump) | `event_classifiers::is_superseded_jump_end_file` |
| S3 | `playlist-next` from mpv's side (window key path) | `daemon-playback-intents` spec, mpv-side entry change |
| S4 | Load-then-play: append all items with `pause=yes`, then unpause | `submit.rs` load path (design D3), `restart.rs` |
| S5 | Playlist with a missing file between two good ones | `termination.rs` `on_mpv_error` generic path |
| S6 | `audio-device=alsa/mbv-probe-missing` | `is_clocked_audio_error` |
| S7 | `script-message mbv-next-up-play` sent by a command | `handle_client_message` |
| S8 | `quit` sent while a load is pending | `handle_stop_request`, `on_shutdown` |
| S9 | Absolute seek while paused, then unpause | `restart.rs` seek and restart handling |
| S10 | Video item: `video-params/h` and `current-tracks/video/image` changes | `restart.rs` output classification |

The probe writes each IPC trace (raw lines, in arrival order) to `crates/mbv-player/src/mpv_event/fixtures/<id>.jsonl`.

### D5. `MpvEvent` domain type with two constructors

New `crates/mbv-player/src/mpv_event.rs`, crate-private:

```rust
pub(crate) enum MpvEvent {
    PropertyChange { name: String, value: PropertyValue },
    PlaybackRestart,
    EndFile(EndFileReason),             // domain enum: Eof, Stop, Quit, Redirect
    Failed(MpvFailure),                 // libmpv Err(e) / IPC end-file reason=error
    LogMessage { prefix: String, level: String, text: String },
    ClientMessage(Vec<String>),
    Shutdown,
    Ignored,                            // events the run loop does not act on
}
pub(crate) enum MpvFailure { AudioOutputInit, Other(String) }
```

- `MpvEvent::from_libmpv(Result<Event, libmpv2::Error>)` is the production path. `Err(Raw(AoInitFailed))` becomes `Failed(AudioOutputInit)`, and every other error becomes `Failed(Other(mpv_err_str(e)))`. This keeps today's behavior: an end-file error is a failure, not an `EndFile`.
- `MpvEvent::from_ipc_line(&str) -> Result<MpvEvent, DecodeError>` is used by tests only for now. It decodes `end-file` with `reason:"error"` to `Failed`. It maps `file_error` equal to mpv's `AoInitFailed` error string to `AudioOutputInit`. It returns `Ignored` for known events that the run loop does not act on (`start-file`, `file-loaded`, `seek`, `idle`, `audio-reconfig`, `video-reconfig`, command replies), and `DecodeError` for malformed lines. `DecodeError` is a domain error type (no `thiserror`).
- `PropertyValue` has the variants `Double`, `Flag`, `Int64`, `Str` and `None`. The run loop already matches on names, so the IPC decoder types each value by the property name, using the formats that `observe_properties` requests.
- The run loop matches `MpvEvent` exhaustively, so the `_ => false` wildcard goes away. `Ignored` is the documented no-op arm.
- `is_clocked_audio_error` takes `&MpvFailure`. `is_superseded_jump_end_file` takes the domain `EndFileReason`. `EndFileReason` changes from the libmpv2 constant type to the domain enum at its existing use sites (`termination.rs` `report_for_end_file` and the queue-advance paths).

- The domain `EndFileReason` has no `Error` variant. mpv always sets an error code with the `error` reason, so libmpv2 returns that end-file as `Err` and `EndFile(Error)` never arrives. The `reason == Error` branches were unreachable: the `player.end_file.playback_error` warning, `fail_active_file_start` (with its `active_file_starting` field), and the `Error` test cases. Group 1 deletes them. Issue #936 tracks the active-file start-failure stop, which never fired. A reason value without a domain variant becomes `Failed(Other(_))`.
- `PropertyValue::None` comes with the IPC decoder in group 4. Only the decoder constructs it.
- Nothing outside tests calls the decoder, and the lint rules forbid `allow` for dead code. So the decoder lives in `mpv_event/ipc.rs` behind `#[cfg(test)]`, and so does the `None` variant. The migration change removes both gates when it wires IPC into the run loop. That change must also give `None` an arm in `on_property_change`: today the `("video-params/h", change)` arm would log it as an unexpected type.
- The decoder returns `Ignored` for a line with no `event` key (command replies and the fixture header) and for any event name it does not know, so a new mpv version that adds an event does not break it.

Rejected alternative: decode IPC JSON straight into `libmpv2::Event`. Its borrowed `&str` fields and `PropertyData` cannot be built from owned JSON, and the change would tie fixtures to the binding that the migration wants to remove.

### D6. Tests own two contracts

Both live in `crates/mbv-player/src/mpv_event/tests.rs`, because they test crate-private items.

1. **Decoder contract:** a test helper `decode_fixture(name)` loads a fixture with `include_str!`, decodes every line, and panics on the first `DecodeError`. Every classifier test below goes through it, so real mpv output is decoded with no separate per-file test. One extra test asserts that a malformed line returns `DecodeError`.
2. **Classifier-on-real-data contract:** these are named tests with different expected outcomes, each citing its scenario:
   - S2 fixture: the decoded sequence contains a `Stop` `EndFile` that `is_superseded_jump_end_file(reason, None, false)` classifies as debris.
   - S6 fixture: the decoded sequence contains `Failed(AudioOutputInit)`, and `is_clocked_audio_error` returns `true` when a device is configured.
   - S5 fixture: the missing file decodes to `Failed(Other(_))`, and `is_clocked_audio_error` returns `false`.
   - S1 fixture: the natural end decodes to `EndFile(Eof)`.

No test asserts timing, counts of `time-pos` events, or log text.

## Risks / Trade-offs

- [lavfi media and `ao=null` behave differently from real streams and hardware audio] → The probe tests event *semantics*, not timing. The Findings section names any scenario whose outcome depends on real output. Live checks of those stay with the user.
- [Timing-sensitive scenarios (S2, S8) can produce different sequences from run to run] → The probe runs each scenario 5 times per backend and reports every distinct sequence. Findings record nondeterminism as a property of mpv, not as a backend difference, when both backends show it.
- [The `MpvEvent` refactor could change run-loop behavior] → Today's tests in `crates/mbv-player/src/tests/` must pass unchanged. `from_libmpv` maps error codes the same way `on_mpv_error` reads them today.
- [Fixtures go stale with new mpv versions] → Each fixture header line records `mpv --version` and the date. Rerun the probe when mpv changes in a way that matters.
- [The `AoInitFailed` error string differs between mpv versions] → The decoder compares against the string that `libmpv2-sys` `mpv_error_string` returns at runtime, not a literal.

## Findings

Run on 2026-10-13. The mpv binary and libmpv are both v0.41.0. Each scenario ran 5 times per backend. The full output is in `probe-output.txt`. The recorded traces are in `crates/mbv-player/src/mpv_event/fixtures/`.

### Results per scenario

| ID | Scenario | Result | Note |
|----|----------|--------|------|
| S1 | Two-item playlist plays to natural EOF | MATCH-IGNORING-PROPERTY-INTERLEAVING | Both items end with `end-file eof`. |
| S2 | Two `playlist-pos` writes in quick succession | MATCH-IGNORING-PROPERTY-INTERLEAVING | The `stop` end-file for the superseded target arrives on both backends. |
| S3 | `playlist-next` from mpv's side | MATCH-IGNORING-PROPERTY-INTERLEAVING | |
| S4 | Load with `pause=yes`, then unpause | MATCH-IGNORING-PROPERTY-INTERLEAVING | |
| S5 | Missing file between two good items | MATCH-IGNORING-PROPERTY-INTERLEAVING | Error code -13 (`loading failed`) on both. Playback goes on to the next item. |
| S6 | Audio output init failure | MATCH-IGNORING-PROPERTY-INTERLEAVING | Error code -14 (`audio output initialization failed`) on both. |
| S7 | `script-message mbv-next-up-play` | MATCH-IGNORING-PROPERTY-INTERLEAVING | `client-message` args are equal. |
| S8 | `quit` while a load is pending | **DIFF** | See below. |
| S9 | Absolute seek while paused, then unpause | MATCH-IGNORING-PROPERTY-INTERLEAVING | |
| S10 | Video item (`video-params/h`, image flag) | MATCH-AFTER-NORMALIZATION | Exact order, after the rules below. |

**S8 is the one real difference.** libmpv delivers `shutdown` in 5 of 5 runs. The IPC client never gets `shutdown`: the socket closes. Before the close, IPC delivered `start-file` and `end-file(reason=quit)` in 3 of 5 runs, and nothing after `quit` in 2 of 5 runs.

### Normalizations used

| Rule | What it hides | Scenarios |
|------|---------------|-----------|
| 1 | IPC-only fields (`playlist_entry_id`, `playlist_insert_id`, `playlist_insert_num_entries`), and property changes with no data (libmpv2 never delivers them) | all |
| 2 | `file_error` text mapped to an error code (libmpv gives the code) | S5, S6 |
| 3 | Consecutive `time-pos` changes collapsed, values dropped | all but S8 |
| 5 | Command replies (`request_id` lines) | all |
| 6 | `"id":0` that mpv's JSON omits | all but S8 |

Rule 4 (log messages) never applied: the probe does not request them.

Two comparison choices are not rules. The probe drops non-property events that arrive before the first step (the IPC client connects after mpv starts, so it misses the startup `idle`). The split comparison ignores where a property change lands among other events, and drops repeated equal values.

### Nondeterminism

- Where a property change lands among nearby events changes from run to run on **both** backends (for example `audio-codec-name` against `audio-reconfig`). It is a property of mpv, not of IPC. The run loop cannot depend on that position today, and a move must not add the dependency.
- S8 gave 3 different IPC sequences in 5 runs. libmpv gave 1 in the final run. An earlier run showed that libmpv also sometimes delivers `end-file(quit)` before `shutdown`.

### IPC-only and libmpv-only behavior

- IPC events carry `playlist_entry_id` (also `playlist_insert_id`). libmpv2 does not expose them. They can tie an `end-file` to the playlist entry that caused it.
- IPC sends a property change with no `data` when a property is unavailable. libmpv2's `wait_event` returns `None` for it, and that ends mbv's drain loop early. The IPC decoder maps it to `PropertyValue::None`.
- IPC numbers come as JSON (`100.000000`). The decoder types each value by property name.
- Failure reasons are text over IPC and codes over libmpv. The decoder compares `file_error` with the runtime text of `AoInitFailed`. The test on S6 proves this on v0.41.0. Rerun the probe when mpv changes.

### Synchronous `get_property` reads

Over IPC, each read is a round trip. The design counted 5. The code has these sites:

| Site | Reads | Migration input |
|------|-------|-----------------|
| `lib.rs` `mpv_position_ticks` | `time-pos` | Called inside event handlers. The value is the value at read time, so the round trip makes the gap larger. |
| `lib.rs` `reassert_queue_layout` | `playlist-count`, `playlist-pos`, `idle-active` | One `get_property` per read. |
| `run/queue.rs` | `playlist-pos` | Already observed (id 9). Use the last observed value. |
| `run/events/restart.rs` | `video-params/h`, `current-tracks/video/image`, `audio-codec-name` | All three are already observed. Use the last observed values. |
| `run/events/track_progress.rs` | `duration` | One read. |
| `runtime.rs` | `volume-max` | One read at startup. |
| `tracks.rs` | `track-list/count`, then about 10 reads per track | N × 10 round trips. Replace with one read of `track-list`. |

### Recommendation

**Move mpv out of process. No event gap blocks it.** 9 of 10 scenarios give the same events over IPC as through libmpv, up to the normalizations above. The one difference (S8) is a known shutdown behavior that the move can handle. Start a separate change with `/opsx:propose`. It must carry these inputs:

1. Treat socket close or child exit as `Shutdown`. Do not rely on `end-file(quit)` at stop.
2. Give `PropertyValue::None` an arm in `on_property_change` (today `("video-params/h", change)` would log it as an unexpected type), and remove the `#[cfg(test)]` gates on `mpv_event/ipc.rs` and on `None`.
3. Replace the synchronous reads in the table above with observed values or one batched read. Check each one that runs inside an event handler for a stale-value hazard.
4. Connect to the socket before the first `loadfile`, and read start state (`idle-active`) with a request, because the client can miss the startup `idle`.
5. Keep the run loop free of dependence on property-change position (nondeterminism above).

Not covered by this probe: real audio devices and video output (`ao=null`, `vo=null`), network streams, log messages, and the command path (`loadfile`, `seek`, `set_property` over IPC). Those stay with the migration change and with live checks by the user.
