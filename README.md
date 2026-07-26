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
- **Home / continue-watching views** — Continue Watching and recent additions across libraries.

## mbv-Only Features

- **Dedicated persistent queue** — its own queue model, not Emby's play-next/play-later. Queue-source tracking, undo delete, jump-to-library from a queue item, queue-first workflows.
- **Headless daemon mode** — `mbvd` runs the player with no terminal attached, as a background service or systemd unit; any `mbv` can connect to it as a thin client over its own protocol, not just standard Emby session control. See "mbvd" above.
- **mpv-first playback model** — playback runs through embedded mpv, including headless audio and optional PCM pipe output. Configure pipe compatibility with `[mpv].audio_pipe_samplerate` and `[mpv].audio_pipe_bitdepth` (`16`, `24`, or `32`) — Snapserver's `sampleformat` must match both exactly. The pipe is config-only; there's no live toggle.
- **Opinionated playback defaults** — English audio preferred, subtitles start off, image-based subtitle tracks hidden because they don't work in headless mpv.
- **Special music library handling** — folder-shaped music libraries via `[music].levels`, with grouped browsing standard Emby clients don't offer.
- **Feed-library defaults** — chosen libraries behave like feeds, unplayed and date-sorted — good for YouTube-style libraries.
- **Extra local control surfaces** — MPRIS lets desktop widgets, `playerctl`, and media keys control mbv.
- **Desktop-integrated prompts** — with `system_notifications = true`, Skip Intro, Next Up, and queue prompts show as actionable desktop notifications.
