---
status: accepted
---

# mpv Runs in a Child Process

## Context

The Player owner embedded the mpv library in its own process. A fault in mpv,
in libav, or in a decoder (a crash, a hang, or growth of memory) belonged to
the Owner process. It could end the queue, the Clients, and the Stay-alive
state together. The allocator also held the memory of a long session until the
process exited.

The change `mpv-ipc-event-probe` compared mpv as a child process on JSON IPC
against the embedded library. Its Findings are the evidence for this decision
(`openspec/changes/archive/2026-10-13-mpv-ipc-event-probe/design.md`).

## Decision

The Player owner runs mpv as a child process and controls it over mpv's JSON
IPC socket. The Owner process does not load the mpv library into its own
address space. One child process serves one Playback run. The child exits when
its run ends.

## Evidence

The probe ran 10 scenarios (S1 to S10) through both backends. It ran each
scenario 5 times per backend. The mpv binary and libmpv were both version
0.41.0.

- 9 of the 10 scenarios gave the same events on both backends, after
  documented normalizations. The normalizations hide IPC-only fields, the
  text-versus-code form of a failure, and the place of a property change among
  other events.
- Scenario S8 (a quit during a pending load) is the one difference. It is a
  shutdown behavior that the move handles (see below).
- The place of a property change among nearby events changes from run to run
  on both backends. It is a property of mpv, not of IPC. The run loop must not
  depend on that place.

No event gap blocked the move. The probe did not cover real audio devices,
video output, network streams, log messages, or the command path.

## S8: mpv sends no shutdown event on IPC

The embedded library delivered a `shutdown` event in all 5 runs of S8. The
IPC client never got `shutdown`: the socket closed. Before the close, the IPC
client got `start-file` and `end-file(reason=quit)` in 3 of 5 runs, and
nothing after `quit` in 2 of 5 runs.

The Player owner therefore treats the close of the IPC socket and the exit of
the child as shutdown. It does not wait for a shutdown event. A close after a
requested quit is a normal stop. A close with no requested quit is a fault,
and the run ends with an error and the last known position.

## Considered options

- **Keep the embedded library (rejected).** A fault still ends the Owner
  process, and mpv memory still stays until process exit.
- **A trait with two backends (rejected).** Nothing needs both at run time.
  The probe example already compares them.
- **`--input-ipc-client=fd://N` on a socket pair (rejected).** It needs a newer
  mpv. The server socket stays for user tools in any case.
- **A reader thread for each run (rejected).** It adds a second thread for
  each run and breaks the single-owner rule of the playback thread. The
  session polls the socket on the playback thread instead.

## Consequences

- A Debian install needs the `mpv` program, not the `libmpv2` library.
- A missing `mpv` program fails the run with an error that names the program.
  The Owner process stays up.
- A fault or an exit of the mpv child ends only its run. The Owner process,
  the queue, and the Clients stay up.
- Memory that mpv holds returns to the operating system at the end of each
  run.
- The child never outlives the Owner process and never serves two runs
  ([Invariant 22](../invariants/22-mpv-child-never-outlives-the-owner.md)).
- Related decisions: [ADR 0001](0001-core-playback-queue.md) (the queue
  authority stays with the owner, not with mpv) and
  [ADR 0030](0030-owner-process-is-the-only-local-player-owner.md).
