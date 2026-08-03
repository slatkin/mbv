# mbv

A terminal UI for Emby. It plays media through mpv.

You can browse libraries, build a queue, and play from your server. Playback stays in sync with Emby, so any Emby remote app can control it. It can also run headless as a daemon and launch videos/music via remote control.

# Features

## Emby-Parity Features

- **Library browsing and search** — navigate folders and series, jump to seasons and episodes, fuzzy-search within a library.
- **Resume and watched-state sync** — videos resume where you left off; watched status reports back to Emby.
- **Standard remote control compatibility** — any Emby remote app on phone or browser can drive mbv.
- **Session control from mbv** — connect to another active Emby session and control it from mbv. `F3` opens the session list.
- **Playlist integration** — browse Emby playlists, enqueue them, save the current queue back with `Ctrl+S`.
- **Normal playback controls** — seek, pause, adjust volume, cycle audio tracks, toggle subtitles.
- **System-audio visualizer** — press `v` to show CAVA's spectrum for the default system audio path. This intentionally may include audio from other applications; it does not reroute or change mpv playback. Requires [`cava`](https://github.com/karlstav/cava) as a runtime dependency.
- **Home / continue-watching views** — Continue Watching and recent additions across libraries.

## mbv-Only Features

- **Dedicated persistent queue** — its own queue model, not Emby's play-next/play-later. Queue-source tracking, undo delete, jump-to-library from a queue item, queue-first workflows.
- **Headless daemon mode** — `mbvd` runs the player with no terminal attached, as a background service or systemd unit; any `mbv` can connect to it as a thin client over its own protocol, not just standard Emby session control. See "mbvd" above.
- **mpv-first playback model** — playback runs through embedded mpv, including headless audio and optional PCM pipe output. Configure pipe compatibility with `[mpv].audio_pipe_samplerate` and `[mpv].audio_pipe_bitdepth` (`16`, `24`, or `32`) — Snapserver's `sampleformat` must match both exactly. For a direct `mbvd` pipe client, optional `[mpv].audio_pipe_playout_delay_ms` is a manually calibrated estimate used only for startup progress and duplicate-play guarding. It is not an audibility guarantee; it can drift, and mbv neither queries nor controls the downstream consumer. The pipe is config-only; there's no live toggle.
- **Opinionated playback defaults** — English audio preferred, subtitles start off, image-based subtitle tracks hidden because they don't work in headless mpv.
- **Special music library handling** — folder-shaped music libraries via `[music].levels`, with grouped browsing standard Emby clients don't offer. Includes recursive album search across all configured ancestor levels.
- **Sticky library position** — browse position within each library (including drill depth and focused item) is saved and restored across restarts.
- **Auto-reconnect for daemon routes** — when enabled, reconnects to the last active daemon-routed library at startup via the `[settings] auto_reconnect` option.
- **Feed-library defaults** — chosen libraries behave like feeds, unplayed and date-sorted — good for YouTube-style libraries.
- **Extra local control surfaces** — MPRIS lets desktop widgets, `playerctl`, and media keys control mbv.
- **Desktop-integrated prompts** — with `system_notifications = true`, Skip Intro, Next Up, and queue prompts show as actionable desktop notifications.

## Shared mbv State

Shared state is disabled by default. It requires one boolean in the system
daemon config and one boolean in each client config. The daemon advertises its
shared-data endpoint through Emby; clients discover it automatically.

The packaged `mbvd` service reads `/etc/mbv/config.toml`. Set:

```toml
[shared_data]
enabled = true
```

Each `mbv` client reads `$XDG_CONFIG_HOME/mbv/config.toml`, normally
`~/.config/mbv/config.toml`. Set the same boolean there:

```toml
[shared_data]
enabled = true
```

The normal setup uses private-LAN TCP on port `47789`; clients reject a daemon
advertised at a public address. Advanced deployments may still set `listen`,
`endpoint`, and the TLS certificate/key fields explicitly. For `tls://`, the
client validates the certificate before sending its Emby token.

Shared queue, library-position, reconnect, and roaming-settings documents are
mirrored locally. If `mbvd` is unavailable, browsing and playback continue from
the local mirror and background reconnect attempts use bounded exponential
backoff. Existing shared documents win when a client reconnects.

To inspect an existing host database locally without starting the daemon, run
`sudo MBV_SYSTEM=1 mbvd --export-shared-data`. Setting either boolean to false
returns the client to
local-only behavior; the database is preserved for later re-enablement or
export.

### First-time setup

1. Edit `/etc/mbv/config.toml`: set the Emby `[server].url` and
   `[shared_data].enabled = true`.
2. Run `sudo MBV_SYSTEM=1 mbv` once and log in. This writes the system daemon's
   credentials under `/var/lib/mbv`; quit after login.
3. Allow TCP port `47789` from the intended private LAN, then restart `mbvd`.
4. In each client's `~/.config/mbv/config.toml`, set
   `[shared_data].enabled = true` and restart `mbv`.

The first client initializes empty shared documents from its local state. To
roll back, set `enabled = false` on the clients or daemon. Local mirrors and the
daemon database under `/var/lib/mbv` are preserved.
