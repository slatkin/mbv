# Local daemon for stay-alive

**Supersedes ADR 0005** (Owned pty relay for stay-alive mode).

## Decision

Stay-alive is **ensure the local daemon exists, then attach**, not a relay.
There is one local daemon per user, serving `$XDG_RUNTIME_DIR/mbv-ctrl.sock`.
The launching `mbv` authenticates, self-spawns a hidden local-daemon
subcommand if nothing is listening on that socket, and then runs its own
full TUI as an ordinary client of `DaemonEndpoint::Local` — the same client
path `--connect-daemon` already uses. There is no pty, no byte-pipe terminal
client, and no eviction: any number of terminals may attach at once.

- **A client exiting never stops the local daemon.** Stopping it is only ever
  explicit — `mbv -q` or tray Quit — regardless of which invocation spawned
  it, or whether any invocation did (a user may have started it by hand).
  There is no reference counting and no "last client out" race.
- **Session continuity is deliberately not offered.** Only *playback*
  continuity is guaranteed: what is playing, the queue, and position survive
  a client closing. On-screen state — cursor, scroll, open overlays,
  in-flight search, the queue undo stack — does not, and is not meant to.
  This is the whole reason the pty relay is gone: preserving a terminal's
  byte stream is what forced single-slot attach, pty ownership, and eviction
  in ADR 0005. Per-library browse position, `prefs.json` scalars, and
  `queue_state.json` continue to persist as before, independent of this.
- **Bare mode is unchanged.** With `stay_alive` off, `mbv` is one process
  owning an in-process Player, exactly as today. `mbv -d` is the ad-hoc
  escape hatch for wanting several terminals; it is a no-op when
  `stay_alive = true` is already configured.

## Considered options

- **Ensure-daemon-then-attach over the existing local daemon (chosen).** The
  substrate — a daemon owning the Player behind a ctrl socket, with
  non-evicting multi-client attach already specified at the protocol level —
  already existed and is strictly better than the relay for this job. Stay-
  alive becomes a thin bootstrap over it rather than a second mechanism.
- **Keep the pty relay (rejected).** ADR 0005's design. It costs ~900 lines
  of pty, winsize, signal, and single-slot-attach code to preserve a property
  — byte-level session continuity — that is being deliberately dropped, and
  it forces eviction on attach: a second terminal always kicks off the first.
- **Make everything daemon-mode by default (rejected).** Uniform, but it
  leaves a resident mpv process running after every casual `mbv` launch, and
  it contradicts the rule that a client exiting must never stop the daemon:
  if daemon-mode is always on, "always on" means "always leaves something
  running" with no bare mode to fall back to for a quick, self-contained
  session.
- **tmux / abduco / dtach as the shipping mechanism (rejected).** ADR 0005
  rejected this category because no stock relay was both graphics-transparent
  and program-controllable — tmux mangles sixel/kitty, and abduco/dtach have
  no way for mbv to trigger a detach itself. That objection **evaporates**
  under this design, not just for the program-controllable half but for the
  graphics half too: there is no relay at all. Each client is a full `mbv`
  process on its own real terminal, running its own DA1/XTGETTCAP graphics-
  capability detection directly against that terminal. There is no byte-level
  pipe standing between a client and its terminal for image or graphics
  escape sequences to pass through, so a multiplexer's transparency to those
  sequences is no longer a property this design needs from anything.

## Consequences

- **Crash isolation is preserved, not dropped — it moves.** ADR 0005 credited
  the dumb-pipe terminal client with crash isolation: the terminal-owning
  process carried none of mbv's crash-prone machinery, so an mbv/libmpv panic
  couldn't wedge the terminal. Under this design libmpv — the actual player,
  and the component most prone to crashing on bad media or codecs — moves
  *out* of the terminal-owning process entirely and into the daemon, which
  owns no terminal at all, so a daemon crash cannot corrupt anyone's screen.
  The terminal-owning client is no longer a trivial byte pipe, but it no
  longer hosts the crash-prone component either: `src/main.rs` installs a
  global panic hook (`install_panic_hook`) as the first thing `main()` does,
  which writes a crash log and restores the terminal on a client-side panic —
  exactly as it already does in bare mode today, relay or no relay. A client
  dying now also costs the user nothing beyond that terminal, since the
  daemon keeps playing regardless.
- **No more single-slot attach.** Any number of terminals may be attached to
  the local daemon at once; a new client no longer evicts an existing one.
- **The tray moves into the local daemon**, via the existing
  `DaemonRuntimeHooks::on_tray_ready` hook, giving it a home that outlives
  every terminal rather than living in `App::run()`. Packaged `mbvd` keeps
  its no-op tray stub — it runs as root under systemd with no user session.
- **Users of today's relay-backed stay-alive lose on-screen continuity.**
  This is unavoidable and intended, not an unfinished corner: it is the cost
  of deleting the pty. Called out as a breaking change in release notes.
- `src/relay.rs`, `src/terminal_client.rs`, the `--__relay` hidden
  subcommand, and `src/app/stay_alive.rs` are deleted along with it.
