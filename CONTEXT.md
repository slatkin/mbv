# mbv

A terminal client for Emby that browses a library and plays media. Playback may
run inside the terminal process itself, or be hosted by a background process on
the same machine so it survives the terminal closing.

## Playback ownership

**Player owner**:
The single process on a machine that holds the audio device and the Emby
playback session. Exactly one exists per user at a time.
_Avoid_: instance, master, host

**Bare mode**:
The default presentation, where one process is both the terminal UI and the
Player owner. Closing it stops playback.
_Avoid_: foreground mode, standalone, normal mode

**Stay-alive**:
The mode in which playback is hosted by a local daemon rather than the terminal
UI, so playback continues after every terminal window closes.
_Avoid_: daemon mode, background mode, alive mode, persistent mode

## Processes

**Local daemon**:
The Player owner in stay-alive mode: a user-owned background process on the same
machine as its clients, holding no terminal. One exists per user.
_Avoid_: relay, backend, server, session host

**mbvd**:
The separately packaged daemon, run as a system service, with its own
configuration, state, and socket. A different product surface from the local
daemon, never started by a terminal UI.
_Avoid_: system daemon, the daemon

**Client**:
A terminal UI that owns no Player and reaches one over the control socket. Any
number may run at once, and each is disposable.
_Avoid_: thin client, terminal client, viewer, attachment

**Tray**:
The desktop status icon belonging to the Player owner, giving playback controls
and a stop action while no client is on screen. Only present in stay-alive mode.
_Avoid_: systray, status icon, indicator

## Continuity

**Playback continuity**:
The guarantee stay-alive makes: what is playing, the queue, and position survive
every client closing and reopening.
_Avoid_: persistence, session continuity

**Session continuity**:
Preservation of a client's on-screen state — cursor, scroll, open overlays,
search — across a close and reopen. Deliberately *not* offered; only playback
continuity is.
_Avoid_: terminal continuity, UI state

**Attach**:
A client connecting to an existing Player owner. Never displaces another client;
several may be attached at once.
_Avoid_: reattach, connect, resume, take over
