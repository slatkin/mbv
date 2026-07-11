# Owned pty relay for stay-alive mode

## Decision

Stay-alive mode (mbv surviving its controlling terminal closing while still
playing) is implemented with an **owned pty relay**: a thin, persistent
"master" process that creates the pty, holds the master fd, serves a unix
socket, and relays **raw bytes** to an ephemeral **terminal-client**, with the
mbv app running under it on the pty **slave**. A small **out-of-band
`socketpair` control channel** between relay and app carries exactly two
messages: `client attached` (app re-runs its existing capability detection and
fires the reattach-refresh) and `detach now` (app-initiated detach on `q`).

We own the relay rather than shelling out to `abduco`/`dtach`/`tmux` because no
stock relay is both **graphics-transparent** and **program-controllable**:

| Relay | sixel/kitty passthrough | program-initiated detach |
|---|---|---|
| abduco / dtach | yes | **no** (detach only on the client's keyboard path; mbv is downstream of it) |
| tmux | **no** (mangles sixel/kitty) | yes (`detach-client`) |

mbv's album-art rendering rules out tmux, and the clean `q` = detach rule
requires program-initiated detach, which abduco/dtach cannot do. Owning the
relay is the only way to get both. abduco/dtach served only to **prototype**
(they proved SIGHUP survival, playback continuity, graphics passthrough, and
the reattach-refresh fix).

The relay and client stay **dumb byte pipes**: capabilities reach mbv by
**transparent forwarding** (mbv's existing DA1/XTGETTCAP detection round-trips
through the pipe to the real terminal, re-run on each attach), not by an in-band
control sentinel or a client-computed capability descriptor. This is also what
buys **crash isolation**: the terminal-owning process is the tiny dumb client
with none of mbv's crash-prone machinery, so an mbv/libmpv panic cannot wedge
the terminal.

## Considered options

- **Own the relay (chosen).** Transparent graphics + controlled detach + crash
  isolation; costs us a small pty/relay to write and maintain.
- **abduco/dtach as the shipping mechanism (rejected).** No control channel, so
  no program-initiated detach; `q` = detach is impossible.
- **tmux (rejected).** Mangles sixel/kitty graphics.
- **In-band APC control sentinel (rejected).** Forces the client/relay to scan
  and strip the byte stream — fragile, and breaks the dumb-pipe/crash-isolation
  property.
- **Client-computed capability descriptor over a framed control protocol
  (rejected).** Duplicates mbv's detection logic into the client and turns the
  socket into a multiplexer; buys nothing we need over transparent forwarding.

## Consequences

- The relay is a **SIGHUP firewall**: the app's controlling terminal is the
  relay's pty slave, so real-terminal death (window close, SSH drop, emulator
  crash, client `kill -9`) hits only the client; the app keeps playing. The
  relay must **not** forward client-loss as SIGHUP to the app.
- **Relay death is fatal to the app** in the first cut (the app loses its pty
  master and can't render). The inverse — a relay that restarts a crashed app —
  is parked as v2 "relay-as-supervisor".
- `kill -9` of the client can't restore the terminal (dtach has the same
  limit); reattach re-inits the terminal, so the next `mbv` self-heals, with
  `reset` as the escape hatch.
