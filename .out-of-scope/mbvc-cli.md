# mbvc: dedicated CLI binary for daemon control

mbv does not have (and, for now, does not need) a dedicated `mbvc` binary for
scriptable control of a running daemon.

## Why this is out of scope

The original pitch for `mbvc` (#108) was a fast, script-friendly control
surface analogous to `mpc` for MPD — baseline commands like `play`/`pause`/
`seek`/`volume`, plus queue-manipulation verbs (`queue`/`jump`/`clear`).

Working through it in triage surfaced that the common control surface is
already covered by existing tools that talk to mbv's own control paths:

- `mpvc` (or any mpv-JSON-IPC client) against mbv's mpv IPC socket already
  covers the stateless, single-shot commands: status/play/pause/toggle/stop/
  seek/volume/mute. These don't touch mbv's internal queue mirror, so
  driving them externally is safe.
- `playerctl` against mbv's own MPRIS server (`src/mpris.rs`) already covers
  next/previous/seek in a way that's authoritative — MPRIS calls route
  through `PlayerCommand`/`cmd_tx`, the same channel as TUI keybindings, not
  a bypass.

So the baseline `mbvc` command set doesn't add anything a combination of
`mpvc` + `playerctl` doesn't already provide.

## Dependency: this is conditional, not unconditional

This wontfix is conditional on #109's interoperability work landing (see
#109 and #110). Direct mpv IPC access to mbv's own socket currently *can*
desync mbv's internal queue mirror from mpv's real playlist state (#110);
`playerctl next`/`previous` after such a desync will act on stale data.
If #110 turns out to be infeasible to fix and can only be documented as a
limitation, this decision may need revisiting — the safety story for
"mpvc already does the job" depends on that gap being closed or narrowed.

## Scope not covered by mpvc/playerctl (deliberately not pursued here)

Two things #108 also raised are not addressed by mpvc/playerctl, but aren't
being pursued as a result of this decision either — just noted so a future
request for either of these isn't treated as a duplicate of this rejection:

- **Remote/network daemon targeting.** mpv's own IPC socket is a local unix
  socket only; mbv's own daemon protocol (`ctrl.rs`) was designed to reach
  local-or-explicitly-addressed remote daemons. mpvc/playerctl can't do
  that.
- **Agent-driven testing determinism.** A follow-up on #108 wanted
  machine-readable output, deterministic completion semantics, and
  structured exit codes for scripted test assertions. mpvc's output isn't
  shaped for that use case.

## Prior requests

- #108 — "mbvc: scriptable CLI binary for daemon control"
