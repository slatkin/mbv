# Proposal

## Why

The Owner process embeds libmpv. A fault in mpv, libav, or a decoder (a crash, a hang, or growth of memory) belongs to the Owner process. It takes down the queue, the Clients, and the Stay-alive state with it, and the allocator keeps the memory of a long session until the process exits.

The change `mpv-ipc-event-probe` tested the alternative. It ran 10 scenarios through libmpv and through an `mpv` child process on JSON IPC. 9 of the 10 scenarios gave the same events on both backends, up to normalizations that the run loop does not depend on. The one difference (S8, quit during a load) is a known shutdown behavior that the move can handle. No event gap blocks the move (`openspec/changes/mpv-ipc-event-probe/design.md`, Findings).

## What Changes

- Replace the embedded `libmpv2::Mpv` handle with an `mpv` child process that mbv controls over its JSON IPC socket. mbv starts one child for each playback run, as it creates one libmpv handle for each run today. The end of the run stops the child.
- Add a crate-private mpv session type to `mbv-player`. It owns the child, the socket, and a queue of events that arrive while a request waits for its reply. It offers the calls that the player makes today (`command`, `set_property`, `get_property`, `observe_property`, next event). All other player code stops naming libmpv2 types.
- Decode socket lines with the `MpvEvent` decoder from `mpv-ipc-event-probe`. Remove its `#[cfg(test)]` gates. The run loop keeps its `MpvEvent` match. `PropertyValue::None` gets an arm.
- Turn the libmpv option set (`init_mpv`) into child command-line arguments. Keep the same values. Keep `--input-ipc-server=<mbv_ipc_path>`: it is also the socket that mbv uses, so user tools that talk to that socket keep working.
- Treat socket close or child exit as `Shutdown`. A child that exits before a stop was requested ends the run with an error. The Owner process, the queue, and the Clients stay up.
- Replace the synchronous `get_property` reads that run inside event handlers with the last observed value, and read the track list in one request instead of about 10 reads for each track.
- Replace the libmpv wakeup callback with a poll on the wakeup pipe and the socket together.
- Remove the `libmpv2` and `libmpv2-sys` dependencies from `mbv-player`. The Debian package depends on `mpv` in place of `libmpv2`.
- **BREAKING (packaging):** a Debian install now needs the `mpv` program, not the `libmpv2` library. The Arch package already depends on `mpv`.

Out of scope: a dual backend (the change replaces libmpv, it does not add a switch), changes to queue authority or to the player command set, and live-service or network-stream checks (the user does those by hand).

## Capabilities

### New Capabilities

- `mpv-process-isolation`: the Player owner runs mpv in a child process, so an mpv fault or an mpv exit does not end the Owner process, and memory that mpv holds returns to the operating system at the end of each run.

### Modified Capabilities

None. `headless-playback-memory-footprint` and `mpv-playback-policy` state behavior that the child process keeps. The new capability adds the isolation behavior.

## Impact

- `crates/mbv-player/`: `runtime.rs` (`init_mpv`, `observe_properties`, cache and audio-output setup), `controller.rs` and `submit.rs` (pre-warm), `run/run_loop.rs` (wakeup, `wait_event`, async quit), `run/commands.rs`, `run/queue_commands.rs`, `run/events/restart.rs`, `tracks.rs`, `lib.rs`, `error.rs` (`PlayerError::mpv_init` wraps a libmpv2 error), `mpv_event.rs` and `mpv_event/ipc.rs`.
- `Cargo.toml` and `crates/mbvd/Cargo.toml`: remove the libmpv2 dependencies. Update the Debian `depends` lines.
- `crates/mbv-core/examples/mpv_ipc_probe/` keeps its own `libmpv2` dev-dependency (it compares both backends). This change does not touch it.
- New runtime dependency on the `mpv` program on `PATH`. Missing `mpv` is a start error with a clear message.
- The tests that need a live mpv stay manual. Unit tests use recorded traces and a fake socket peer.
