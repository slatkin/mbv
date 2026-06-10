# mbv

A terminal UI client for [Emby](https://emby.media) media servers that uses an external mpv instance as its main media player. 

It allows one to browse libraries, build a queue, and play media from your library in a way that syncs playback with the Emby server. You can leverage the standard Emby remote control API to control playback from any Emby remote app on your phone or browser. It can also headless as a background daemon as a purely remote-controlled solution.

This was built with Claude Code because I am lazy and I already have a job (for now). You can tell because there's a lot of unnecessary images in a TUI app.

## Requirements

- Rust toolchain (for building)
- [mpv](https://mpv.io) libraries (`libmpv`)

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

[mbv]
# Hide libraries from the tab bar (case-insensitive). Default: ["Live TV", "Podcasts"]
hidden_libraries = ["Live TV", "Podcasts"]

# Hide the Latest block for specific libraries on the Home tab (case-insensitive).
# Does not affect the library tab itself — use hidden_libraries for that. Default: []
hidden_latest = ["Music"]

# Show a Log tab in the tab bar after all library tabs (default: false).
# When false the log is not accessible. Always false in daemon-connected mode.
show_log_tab = false

# Open the Queue tab on launch instead of Home (default: false).
start_on_queue = false

[mpv]
# Show an mpv window for audio playback (default: false = headless)
show_audio_window = false

# Use your own ~/.config/mpv/ setup (scripts, OSC, mpv.conf) instead of
# mbv's bundled OSC. Default false = mbv manages its own mpv environment.
use_mpv_config = false

[daemon]
# Show a system tray icon when running in daemon mode (default: true)
show_systray_icon = true
```

Press `F1` at any time to open the reference screen.

## Features

### What you can do

- **Browse your libraries** — navigate folders and series, jump straight to a show's seasons and episodes, or fuzzy-search within any folder.
- **Build a queue** — add individual items or entire folders to the Queue from any screen. Folders are expanded automatically. Play from any position in the queue; the rest follows in order.
- **Pick up where you left off** — videos resume from their saved position. Watched status is synced back to the server automatically.
- **Full playback controls** — seek, pause, adjust volume, cycle audio tracks, enable subtitles, all from the keyboard.
- **Home screen** — see your Continue Watching items and what was recently added across your libraries.
- **Remote control** — any Emby app on your phone or browser can control mbv in real time: play, pause, seek, skip, adjust volume, queue items.
- **Daemon mode** — run mbv as a background service with no terminal required. Register it with your server and drive it entirely from remote apps.
- **System media keys** — mbv exposes an MPRIS2 interface so desktop widgets, `playerctl`, and system media keys all work automatically.

### Media Playback details

- Media plays through an embedded mpv instance. Playback is synchronised between mpv, the mbv client, and Emby server.
- Audio plays headless by default (no mpv window). Set `show_audio_window = true` to change this.
- If you want, your personal `~/.config/mpv/mpv.conf` is respected (shaders, audio devices, renderer settings, keybindings).
- The mpv IPC socket lives at `$XDG_RUNTIME_DIR/mbv-mpv.sock`, separate from the default mpv socket, so running mbv alongside standalone mpv doesn't cause conflicts.
- By default mbv uses its own bundled OSC ([mpv-osc-modern](https://github.com/maoiscat/mpv-osc-modern)). Set `use_mpv_config = true` to defer to your own `~/.config/mpv/` setup instead.

### Audio and subtitle track selection

mbv applies opinionated defaults every time a new item starts playing:

- **English audio is always preferred.** If the default track selected by mpv is not English (`en`, `eng`, `en-US`, `en-GB`, or anything starting with `english`), mbv switches to the first English track it finds. If no English track exists, the default track is kept.
- **Subtitles are always off at start.** Regardless of what mpv or the Emby server would default to, subtitles are disabled when playback begins. Use `Alt+Z` or click the `≡` control to enable them manually.
- **Image-based subtitle tracks are hidden.** PGS (`hdmv_pgs_subtitle`, `pgssub`), DVD (`dvd_subtitle`, `dvdsub`), DVB (`dvb_subtitle`), and XSUB tracks do not appear in the subtitle list at all, since mpv cannot render them in headless mode.

These defaults suit an English-language setup where subtitles are an opt-in rather than opt-out. There is currently no config option to change them.

#### YouTube / Channels libraries

Libraries of type `homevideos` get special treatment when first opened:

- **Video items only** — folders and playlists are filtered out; only playable `Video` items are fetched.
- **Unplayed only** — the item list is restricted to content you haven't watched yet, keeping the view uncluttered.

This is applied automatically based on the library's collection type as reported by the Emby server. No configuration is required.

## Mouse

- Click a tab to switch to it.
- Click a row to move the cursor; double-click to play or open.
- Right-click an item to open its context menu.
- Double-click the seek bar to jump to that position; click and drag to scrub.
- Click the audio or subtitle label in the controls bar to cycle tracks.
- Scroll wheel moves the cursor in any list.
