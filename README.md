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

Shared state is disabled by default. On the canonical `mbvd`, enable the
dedicated host and configure either a Unix socket or a loopback/private TCP
listener. TLS is optional for TCP; WAN endpoints are rejected:

```toml
[shared_data]
enabled = true
listen = "/run/mbv/shared.sock"
# Or: listen = "192.168.1.20:47789"
# Optional TLS for TCP listeners:
# tls_cert_path = "/etc/mbv/shared-cert.pem"
# tls_key_path = "/etc/mbv/shared-key.pem"
```

On each participating client, opt in with an independent endpoint:

```toml
[shared_data]
endpoint = "tcp://192.168.1.20:47789"
```

For a `tls://` endpoint, the client validates the TLS certificate before
sending its Emby token. Shared queue, library-position, reconnect, and
roaming-settings documents are mirrored locally. If `mbvd` is unavailable,
browsing and playback continue from the local mirror and background reconnect
attempts retry with bounded exponential backoff. Existing shared documents win
when a client reconnects.

To inspect an existing host database locally without starting the daemon, run
`mbvd --export-shared-data`. Disabling either opt-in returns the client to
local-only behavior; the database is preserved for later re-enablement or
export.

### First-time setup

1. On the host that will run `mbvd`, run `mbv` once and authenticate to Emby so
   `mbvd` has cached credentials for its current-user validation.
2. Choose a private IP address on that host and allow the shared-data port only
   from the intended LAN in the host firewall. Do not use `0.0.0.0`, a public
   address, or a public DNS name; WAN endpoints are rejected.
3. Add the `[shared_data]` host configuration above and restart `mbvd`.
4. On each client, authenticate to the same Emby account normally, add the
   separate `shared_data.endpoint`, and restart `mbv`.
5. The first connected client initializes absent shared documents from its
   local state. Later clients adopt the existing shared documents; shared
   queue, library position, reconnect target, and roaming settings then become
   authoritative while the local mirrors remain available for fallback.
6. Confirm the setup by changing a queue or library position on one client and
   checking that another connected client receives it. Use
   `mbvd --export-shared-data` on the host if you need to inspect revisions or
   recover the stored values.
7. To roll back, remove `shared_data.endpoint` from clients and/or set host
   `shared_data.enabled = false`; clients continue using their local mirrors
   and the database is preserved.
