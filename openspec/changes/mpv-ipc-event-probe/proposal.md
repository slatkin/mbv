# Proposal

## Why

The Owner process embeds libmpv. A move to an external `mpv` child process, controlled over JSON IPC, would give crash isolation, memory that the kernel reclaims at the end of each run, and less FFI. The move depends on one open question: do the mpv events that the player run loop depends on arrive with the same types, payloads and order over IPC as they do through libmpv? We must answer that question with recorded evidence before we decide on the move.

mpv source (`input/ipc-unix.c`) shows that each IPC connection is an ordinary client handle that JSON-encodes `mpv_wait_event` output in order. So the semantics should be the same by construction. This change tests that claim against the scenarios that mbv's event logic depends on. It also finds the known differences: end-file failures come as an error code from libmpv but as a string over IPC, and synchronous property reads become round trips.

## What Changes

- Add a manual probe example to `mbv-player`. It runs each scenario two times, one time through in-process libmpv and one time through an `mpv` child process on an `--input-ipc-server=<scratch path>` socket (design D2). It compares the two event traces and writes the IPC trace to a fixture file. The scenarios use synthetic `av://lavfi:` media, so they need no files and no network.
- Add a crate-private `MpvEvent` domain type to `mbv-player`. It covers only the events that the run loop consumes. It has two constructors: one from a libmpv2 event, and one from an IPC JSON event line. The run loop matches on `MpvEvent` in place of `libmpv2::Event`, with no change in behavior.
- Change `event_classifiers` to take domain types in place of libmpv2 types.
- Check in the recorded IPC traces as fixtures. Add unit tests that decode them and run the classifiers on real mpv sequences.
- Record the per-scenario results and the move or stay recommendation in `design.md` under Findings.

No user-visible behavior changes. The move of mpv out of process is not part of this change. If the findings support the move, it gets its own change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This change is test tooling plus a behavior-preserving refactor of the event type, so `.openspec.yaml` sets `skip_specs: true`.

## Impact

- `crates/mbv-player/src/run/run_loop.rs`, `run/events/termination.rs`, `run/events/event_classifiers.rs`: these use `MpvEvent` in place of `libmpv2::Event` and `libmpv2::Error`.
- New `crates/mbv-player/src/mpv_event.rs` (domain type, conversions, IPC decoder) with its tests.
- New `crates/mbv-player/examples/mpv_ipc_probe.rs`. It is a manual tool that needs a real `mpv` binary and libmpv. It never runs in CI.
- New fixture files under `crates/mbv-player/src/mpv_event/fixtures/`.
- No new dependencies. `serde_json` and `libmpv2` are already dependencies of `mbv-player`.
