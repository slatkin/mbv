# mbv

A terminal UI for Emby. It plays media through mpv.

I use Niri. The beta client doesn't support it. The old Linux client is falling apart. The browser is slow. mpv is fast and rarely errors. My eyes are old, so I watch on my monitor while I work instead of walking to the TV. This client fits that life.

You can browse libraries, build a queue, and play from your server. Playback stays in sync with Emby, so any Emby remote app can control it. It can also run headless, as a daemon, and take commands with nothing on screen.

AI wrote most of this. That's why the TUI has more pictures than it needs.

<img src="assets/screenshot-power.png" width="49%" alt="mbv power view: library browser, video preview, and queue" /> <img src="assets/screenshot-music.png" width="49%" alt="mbv music tab: grouped browsing by decade/artist with album art" />

## Requirements

- Rust toolchain (for building)
- [mpv](https://mpv.io) libraries (`libmpv`)
- OpenSSL (TLS to your Emby server)
- `notify-send` (from `libnotify`) — only if `system_notifications = true`

## Installation

On Arch, use the AUR. It installs `mbv`, the `mbvd` daemon, and the `mbvd.service` systemd unit.

```sh
paru -Syu mbv
```

Otherwise, build from source.

```sh
cargo build --release
```

The binaries land in `target/release/`. Copy `mbv` (and `mbvd`, if you want the daemon) onto your `$PATH` yourself — there's no `make install`.

## Usage

```sh
mbv                              # launch with terminal UI
mbv -a                           # stay-alive: keep playing after the terminal closes
mbv                              # reattach to a running stay-alive session
mbv -q                           # stop a running session (foreground or stay-alive)
mbv --connect-daemon <endpoint>  # connect as a thin client to a running mbvd
mbv -V, --version                # print the version and exit
mbv -h, --help                   # print this help
```

## Configuration

On first run, mbv asks for a server URL, username, and password. It saves your credentials after login. You won't need to touch a config file — almost everything lives in the in-app Settings panel (`F2`).

Press `F1` for help and keybindings.

### In-app settings (`F2`)

- **Stay alive** — keep playing in the background after the terminal closes. Same effect as `-a`, but takes hold from the next launch on.
- **Start on queue** — start on the Queue tab instead of Home.
- **Always play next** — auto-play the next queue item, even for videos.
- **Consume videos** / **Consume audio** — drop an item from the queue and mpv's playlist once it finishes.
- **Save playlist on consume** / **Save playlist on consume (audio)** — push the queue to its saved Emby playlist after each consume.
- **Save playlist on quit** — on quit, push a dirty queue's edits to Emby. Off discards them instead; the local queue state still saves either way.
- **Always skip intro** — skip intros without asking.
- **Image protocol** — how album art and card images render: halfblocks, sixel, kitty, iterm2, or auto.
- **Hidden libraries** — hide libraries from the tab bar (case-insensitive).
- **Hidden latest** — hide the Latest block for chosen libraries on Home (case-insensitive); the library tab itself is unaffected.
- **Show audio window** — show an mpv window for audio instead of running headless.
- **Use mpv config** — use your own `~/.config/mpv/` setup instead of mbv's bundled OSC.
- **No scripts** / **autoload** — disable mpv's default scripts, or enable its autoload script for adjacent files.
- **Show systray icon** — show a tray icon in stay-alive mode.
- **System notifications** — send desktop notifications (via `notify-send`) for toasts and prompts (Skip Intro, Next Up, queue) instead of in-TUI ones; prompts get action buttons.
- **My languages**, **Subtitle mode**, **Subtitle language**, **Audio language** — client-only language preferences.
- **Feed view** — libraries to treat as feed views (unplayed, date-sorted).
- **Log out**.

### File-only options (`~/.config/mbv/config.toml`)

A few knobs have no UI. Edit the file by hand.

```toml
[general]
# Reconnect at startup to whatever remote connection (a routed library, or
# a Sessions-panel direct-remote/attached session) was active when mbv
# last exited. Off by default. A failed or impossible reconnect (e.g. the
# other device is offline) falls back to local playback instead of
# blocking startup or erroring.
auto_reconnect = false

[server]
# Override the server URL. Rarely needed — the login screen sets and persists
# this after your first successful login.
url = "http://emby.local:8096"

[mpv]
# PCM pipe output for external consumers like Snapserver. No live toggle.
audio_pipe_enabled = false
audio_pipe_path = "/tmp/mbv-pipe"
# Snapserver's `sampleformat` must match these two exactly.
audio_pipe_samplerate = 192000
audio_pipe_bitdepth = 32

[music]
# Describe the folder layout of your music library so mbv can identify albums.
# Each entry names one level of nesting; the track/file level is always implied
# and should not be included. See "Special music library handling" under Features.
levels = ["group", "album"]

[daemon.client]
# Auto-connect to a running mbvd instead of owning a local player.
# Overridden by `mbv --connect-daemon <endpoint>`. Empty = don't connect.
endpoint = ""

[daemon.server]
# mbvd's own listen address (used when mbvd, not mbv, reads this file).
tcp_listen = ""
```

## mbvd — the headless daemon

`mbvd` is a second binary: mbv's player with no terminal attached. Point a `mbv` at it with `--connect-daemon <endpoint>` or `[daemon.client].endpoint`, and that mbv becomes a thin client — Emby session sync and remote control all still work.

```sh
mbvd                # run the daemon
mbvd --audio-only   # audio only, no video window
mbvd -q             # stop it
mbvd --version
```

The AUR package installs a systemd unit, `mbvd.service`, that runs it as a system service: config in `/etc/mbv/config.toml`, state in `/var/lib/mbv/`, sockets in `/run/mbv/`.

## Features

### Emby-Parity Features

- **Library browsing and search** — navigate folders and series, jump to seasons and episodes, fuzzy-search within a library.
- **Resume and watched-state sync** — videos resume where you left off; watched status reports back to Emby.
- **Standard remote control compatibility** — any Emby remote app on phone or browser can drive mbv.
- **Session control from mbv** — connect to another active Emby session and control it from mbv. `F3` opens the session list.
- **Playlist integration** — browse Emby playlists, enqueue them, save the current queue back with `Ctrl+S`.
- **Normal playback controls** — seek, pause, adjust volume, cycle audio tracks, toggle subtitles.
- **Home / continue-watching views** — Continue Watching and recent additions across libraries.

### mbv-Only Features

- **Dedicated persistent queue** — its own queue model, not Emby's play-next/play-later. Queue-source tracking, undo delete, jump-to-library from a queue item, queue-first workflows.
- **Headless daemon mode** — `mbvd` runs the player with no terminal attached, as a background service or systemd unit; any `mbv` can connect to it as a thin client over its own protocol, not just standard Emby session control. See "mbvd" above.
- **mpv-first playback model** — playback runs through embedded mpv, including headless audio and optional PCM pipe output. Configure pipe compatibility with `[mpv].audio_pipe_samplerate` and `[mpv].audio_pipe_bitdepth` (`16`, `24`, or `32`) — Snapserver's `sampleformat` must match both exactly. The pipe is config-only; there's no live toggle.
- **Opinionated playback defaults** — English audio preferred, subtitles start off, image-based subtitle tracks hidden because they don't work in headless mpv.
- **Special music library handling** — folder-shaped music libraries via `[music].levels`, with grouped browsing standard Emby clients don't offer.
- **Feed-library defaults** — chosen libraries behave like feeds, unplayed and date-sorted — good for YouTube-style libraries.
- **Extra local control surfaces** — MPRIS lets desktop widgets, `playerctl`, and media keys control mbv.
- **Desktop-integrated prompts** — with `system_notifications = true`, Skip Intro, Next Up, and queue prompts show as actionable desktop notifications.
