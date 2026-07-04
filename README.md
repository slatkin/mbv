# mbv

A terminal UI client for [Emby](https://emby.media) media servers that uses mpv (and controls/communicates it with via libmpv) as its media player.

This was made because I use Niri and the beta client does not officially support that. The old official Linux client is increasingly problematic. The browser is a bit slow and problematic as well for me, and I find launching things directly in mpv to be much snappier and able to play more without error. In addition, I'm super old and my eyes are inconsistent, and so I often just watch videos on my monitor while I work because the TV is so far away and require a completely different set of glasses (old). This workflow works really well for me. I'm not crazy.

This allows one to browse libraries, build a queue, and play media from their library in a way that syncs playback with the Emby server. A person can leverage the standard Emby remote control API to control playback from any Emby remote app on their phone or browser. It can also run headless as a background daemon to provide a purely remote-controlled mpv launcher lol.

This was built with Claude Code because I am lazy and I already have a job (for now). You can tell because there's a lot of unnecessary images in a TUI app.

## Requirements

- Rust toolchain (for building)
- [mpv](https://mpv.io) libraries (`libmpv`)
- `notify-send` (from `libnotify`) — only needed if `system_notifications = true`

## Installation

```sh
make install
```
Or if you are on Arch just use AUR.

```sh
paru -Syu mbv
```

Installs the binary to `~/.local/bin/mbv`.

## Usage

```sh
mbv          # launch with terminal UI
mbv --daemon # run headless, controlled via remote only
```

## Configuration

On first run a login screen prompts for server URL, username, and password. Credentials are saved after a successful login.

Optionally create `~/.config/mbv/config.toml`:

```toml
[server]
# Override the server URL (normally set via the login screen)
url = "http://emby.local:8096"

[general]
# Hide libraries from the tab bar (case-insensitive). Default: ["Live TV", "Podcasts"]
hidden_libraries = ["Live TV", "Podcasts"]

# Hide the Latest block for specific libraries on the Home tab (case-insensitive).
# Does not affect the library tab itself — use hidden_libraries for that. Default: []
hidden_latest = ["Music"]

# Show a Log tab in the tab bar after all library tabs (default: false).
show_log_tab = false

# Always skip intros automatically without prompting (default: false).
always_skip_intro = false

# Send desktop notifications (via notify-send) for toasts and interactive prompts.
# Interactive prompts (Skip Intro, Next Up, Clear Playlist) include action buttons.
# When enabled, toasts are suppressed in the TUI and sent to the desktop instead. Default: false.
system_notifications = false

# Keep mbv running as a daemon when you close the TUI window (default: false).
daemon_mode_on_exit = false

# Image rendering protocol for album art and card images.
# Options: "halfblocks", "sixel", "kitty", "iterm2", "auto". Default: disabled.
# image_protocol = "kitty"

[queue]
# Start on the Queue tab instead of Home on launch (default: false).
start_on_queue = false

# Always play the next queue item automatically, even for videos (default: false).
always_play_next = false

# Remove a video from the queue and mpv playlist when it finishes playing (default: false).
consume_videos = false

[mpv]
# Show an mpv window for audio playback (default: false = headless)
show_audio_window = false

# Use your own ~/.config/mpv/ setup (scripts, OSC, mpv.conf) instead of
# mbv's bundled OSC. Default false = mbv manages its own mpv environment.
use_mpv_config = false

# Enable mpv's autoload script to automatically load adjacent files (default: false).
autoload = false

[daemon]
# Show a system tray icon when running in daemon mode (default: true)
show_systray_icon = true

# [music]
# Describe the folder layout of your music library so mbv can identify albums.
# Each entry names one level of nesting. The track/file level is always implied
# and should not be included. See "Music libraries" under Features for details.
# levels = ["group", "album"]
```

Press `F1` at any time to open the help and keybindings reference.

Most settings can also be toggled live in the settings panel (`F2`).

## Features

### Emby-Parity Features

- **Library browsing and search** — navigate folders and series, jump to seasons and episodes, and fuzzy-search within libraries.
- **Resume and watched-state sync** — videos resume from saved position and watched status is reported back to Emby.
- **Standard remote control compatibility** — any normal Emby remote app on your phone or browser can control mbv in real time.
- **Session control from mbv** — connect to another active Emby session and drive it from mbv's controls and keyboard. `F3` opens the session list.
- **Playlist integration** — browse Emby playlists, enqueue them, and save the current queue back to Emby with `Ctrl+S`.
- **Normal playback controls** — seek, pause, adjust volume, cycle audio tracks, and enable subtitles from the keyboard.
- **Home / continue-watching views** — see Continue Watching items and recent additions across libraries.

### mbv-Only Features

- **Dedicated persistent queue** — mbv keeps its own queue model instead of relying on Emby's simpler play-next/play-later behavior. It supports queue-source tracking, undo delete, direct jump-to-library from queue items, and queue-first workflows.
- **mbv-to-mbv remote control** — mbv can connect directly to another mbv daemon over its own control protocol rather than only through standard Emby session control.
- **Headless daemon mode** — run mbv as a background playback service with no terminal required, then drive it remotely.
- **mpv-first playback model** — playback runs through embedded mpv, including headless audio playback and optional PCM pipe output.
  Configure PCM pipe compatibility with `[mpv].audio_pipe_samplerate` and `[mpv].audio_pipe_bitdepth` (`16`, `24`, or `32`). Snapserver's `sampleformat` must match both values exactly. The pipe itself is config-only; there is no live UI toggle.
- **Opinionated playback defaults** — mbv prefers English audio, starts with subtitles off, and hides image-based subtitle tracks that do not work in headless mpv playback.
- **Special music library handling** — mbv can understand folder-shaped music libraries via `[music].levels`, including grouped music browsing that standard Emby clients do not provide.
- **Feed-library defaults** — selected libraries can behave like feed views with unplayed/date-sorted defaults, which is useful for YouTube/channel-style libraries.
- **Extra local control surfaces** — MPRIS integration lets desktop widgets, `playerctl`, and media keys control mbv directly.
- **Desktop-integrated prompts** — with `system_notifications = true`, Skip Intro, Next Up, and queue prompts can be surfaced as actionable desktop notifications.
