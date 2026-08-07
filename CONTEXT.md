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

**Playback run**:
The local mpv playback loop — one per mpv invocation, owned by a Player owner.
Distinct from Session (the Emby-tracked record that exists independently of
mbv). Each run holds the mpv instance, playlist, and per-item lifecycle state
(load pending, stop-report handshake, next-up arming, intro visibility,
startup-pause holdoff).
_Avoid_: session, playback session

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

**Daemon endpoint**:
The address form used to reach any Player owner's control socket: either
Local (this machine's own Local daemon) or a network address (Unix or TCP)
pointing at another Local daemon or an mbvd.
_Avoid_: connection string, remote address, socket path

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

## Queue

**Consume**:
Removal of an item from the queue once it finishes playing, as in ncmpcpp.
Purely a queue operation — it says nothing about where the queue came from and
never edits anything on the server. Applies the same way whether playback ran
in this process, on a Local daemon, or on another device's Session.
_Avoid_: auto-remove, playlist consume, consume-and-save

**Save on consume**:
The separate, opt-in behaviour of writing the shortened queue back to the Emby
playlist it was loaded from. Only meaningful for a queue that is a saved
playlist; Consume happens with or without it.
_Avoid_: autosave, consume persistence, playlist sync

## Remote sessions

A client can also reach *another* device's playback, discovered through Emby
rather than through this project's own local-daemon substrate. This is a
distinct relationship from Attach above, even though both involve one process
reaching a Player owner over a socket.

**Session**:
An Emby-tracked record that some device is playing something. Exists
independently of mbv — including for non-mbv devices — and is what the
Sessions panel lists.
_Avoid_: connection, stream, remote instance

**Session watch**:
A client observing another device's Session read-only — position and title
only, no queue control. The fallback when Direct remote control to that
device isn't available.
_Avoid_: attach, session attach, monitor, remote session (bare)

**Direct remote control**:
A client has its own control-socket connection to another device's Player
owner, giving the same queue management as a local session — reorder,
remove, play next, all of it. This is what the aqua queue-scope pill
indicates.
_Avoid_: green pill, remote takeover, queue management (alone)

**Queue scope**:
Local or Remote — which queue (this client's own Player owner, or the
Direct remote control target) is currently shown in the queue panel. Only
exists while Direct remote control is active.
_Avoid_: split view, pill state

**Library route**:
A persistent per-library assignment sending that library's playback to a
chosen device, set from the library-routing picker rather than the Sessions
panel. Independent of Session watch / Direct remote control — connecting via
the Sessions panel tears down an active route first.
_Avoid_: routing, daemon route

**Home daemon**:
The Local daemon a stay-alive client falls back to once Direct remote
control or a Library route ends. A bare-mode client has no home daemon;
ending remote control there resumes its own in-process Player directly.
_Avoid_: home base, origin daemon
