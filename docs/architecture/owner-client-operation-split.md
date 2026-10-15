# What the Owner Runs, What the TUI Runs

**Status:** current as of 2026-10-15 (`main`, 9eb825ec9).

Every local launch is a Client of the per-user Owner process. The Owner
process is the only local Player owner. The two processes split the work of
the app. This table records which side runs each operation. The domain terms
are defined in `CONTEXT.md`. The ownership decisions are ADR 0029 and 0030.

The rule behind the table: the Owner holds playback and queue authority. The
TUI holds presentation, Service reads, and desktop surfaces. Requests cross
ctrl. Data never does. No Service credential crosses ctrl.

| Domain | Owner process runs | TUI (Client) runs |
| --- | --- | --- |
| Playback run | Spawns and ends the mpv child process per Playback run; holds the mpv IPC socket (`mbv-player`, ADR 0032) | Nothing. It never touches mpv |
| Transport | Executes pause, seek, volume, mute, stop. Resolves `Step(Direction)` targets (`relative_step_target`) | Sends `PlaybackIntent` / `PlayerCmd` / `Step` over ctrl. Shows optimistic state until the Owner confirms |
| Run lifecycle | Decides resume versus restart at entry start (`mbv-player` start-position rule). Ends the run when nothing can play (`stop_for_unrecoverable_error`, `docs/invariants/24-run-ends-when-nothing-can-play.md`) | Shows run state from Owner snapshots |
| Progress | Reports watch position to Emby and Audiobookshelf from the run (`mbv-player` report worker) | Shows progress. Sends `ProgressUpdate` for remote-sourced changes |
| Queue | Holds the only canonical Bound queue: slots, order, lineage, consume. Applies every edit. Persists it across restarts. Owns play order | Builds and submits queues (`UnifiedQueueReplace`, `UnifiedQueueLoadIdle`, append, remove, move, clear over ctrl). Displays adopt-only snapshots |
| Owner admission | Tests media kind, loaded Service setup, and transport for each item before it enters the queue | Strips Unplayable items before submission. Shows the structured rejection |
| Service setup | Rereads committed owner-local setup on `ApplyServiceSetup`. Setup values and credentials never cross ctrl | Validates the server and Service credential against the remote. Commits setup owner-locally. Sends the apply request |
| Service reads | Resolves playback streams for the run | All browsing: library fetches, pagination, search, Home sections, feed refresh, artwork cache |
| Emby websocket | Runs the Emby websocket and obeys remote play, stop, and pause commands from other Emby clients (`mbv-daemon` ws) | Nothing |
| F3 Sessions and Cast | Nothing | Direct remote control of Emby Sessions and cast receivers: discovery, dispatch, cast heartbeat (`src/app/dispatch/cast*`) |
| Playlists | Nothing | Save-on-consume write-back to the Emby playlist (`consume_quit.rs`) |
| Ctrl protocol | Listens, admits Clients, role-gates commands, broadcasts queue and playback events | Attaches with the Hello handshake. Receives and renders snapshots |
| Lifetime | Stay-alive policy. Outlives Clients. Coordinated shutdown with durable queue save on `RequestShutdown` | Sends `RequestShutdown`. Spawns the Owner (`mbv --__local-daemon`), holds the single-instance flock, waits for socket ready, attaches |
| Owner actions | Runs Pin swap and Open terminal on request | Nothing |
| Tray | Shows the desktop Tray (local Owner process only; mbvd has none) | Nothing |
| Pinned panel | Nothing | Owns the pinwin layer-shell window and its pty. A pinned launch is a Client |
| Config | Keeps owner settings (`owner_settings.rs`) | Edits `config.toml` (F2). Runs UI-state reset. Keeps the TUI launch state exit snapshot |
| UI | Nothing | Everything on screen: panels, Interactive Components, the keyboard router, search, the visualizer capture and render, idle feed display, toasts |

mbvd is a separate product surface. It is not part of this table.
