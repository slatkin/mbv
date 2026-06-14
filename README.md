# mbv

A terminal UI client for [Emby](https://emby.media) media servers that uses an embedded mpv instance as its media player.

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

### What you can do

- **Browse your libraries** — navigate folders and series, jump to seasons and episodes, or fuzzy-search within any folder.
- **Build a queue** — add individual items or entire folders to the Queue from any screen. Folders are expanded automatically. Play from any position; the rest follows in order.
- **Pick up where you left off** — videos resume from their saved position. Watched status is synced back to the server automatically.
- **Full playback controls** — seek, pause, adjust volume, cycle audio tracks, enable subtitles, all from the keyboard.
- **Home screen** — see your Continue Watching items and what was recently added across your libraries.
- **Remote control** — any Emby app on your phone or browser can control mbv in real time: play, pause, seek, skip, adjust volume, queue items.
- **Control other sessions** — connect to another active Emby session and drive it from mbv's controls and keyboard. `F3` opens the session list.
- **Daemon mode** — run mbv as a background service with no terminal required. Register it with your server and drive it entirely from remote apps.
- **System media keys** — mbv exposes an MPRIS2 interface so desktop widgets, `playerctl`, and system media keys all work automatically.
- **Save queue as playlist** — `Ctrl+S` saves the current queue to Emby as a playlist.
- **Undo queue deletes** — `Ctrl+Z` restores the last item removed from the queue.
- **Desktop notifications** — with `system_notifications = true`, toasts and interactive prompts (Skip Intro, Next Up, Clear Playlist) are sent as desktop notifications with action buttons.

### Panels and navigation

| Key | Action |
|-----|--------|
| `F1` | Help / keybindings |
| `F2` | Settings |
| `F3` | Remote sessions |
| `F5` | Refresh |
| `h` | Hide / show the playback panel |
| `Tab` / `Shift+Tab` | Cycle tabs |

### Queue views

The Queue tab has three display modes, cycled with `v`:

- **List** — compact table of all items.
- **Filmstrip** — art strip above a details panel; wider terminals show a big card.
- **Presentation** — full-screen view with embedded seekbar, metadata panel, and scrollable queue list.

### Media playback details

- Media plays through an embedded mpv instance. Playback is synchronised between mpv, the mbv client, and Emby server.
- Audio plays headless by default (no mpv window). Set `show_audio_window = true` to change this.
- Your personal `~/.config/mpv/mpv.conf` can optionally be used (shaders, audio devices, renderer, keybindings) by setting `use_mpv_config = true`.
- The mpv IPC socket lives at `$XDG_RUNTIME_DIR/mbv-mpv.sock`, separate from the default mpv socket.
- Gapless audio playback is enabled by default.

### Audio and subtitle track selection

mbv applies opinionated defaults every time a new item starts playing:

- **English audio is always preferred.** If the default track selected by mpv is not English, mbv switches to the first English track it finds. If no English track exists, the default is kept.
- **Subtitles are always off at start.** Use `Alt+Z` or click the `≡` control to enable them manually.
- **Image-based subtitle tracks are hidden.** PGS, DVD, DVB, and XSUB tracks do not appear in the subtitle list since mpv cannot render them headless.

### Library views

- **Movies** and **TV Shows** show a rich active row with thumbnail, metadata, and overview when selected.
- **TV Shows** display a season grid before entering a season, with episode thumbnails and progress indicators.
- **YouTube / Channels** (`homevideos` type) show only unplayed video items, sorted oldest-first.
- All libraries support fuzzy search (shown in the border title). Instant search is available after the first load thanks to background prefetching.
- Libraries with many items are paginated; the scrollbar shows position within the current page.
- Per-library sort parameters are applied automatically (e.g. YouTube sorted by date ascending).

#### Music libraries

mbv browses music libraries in plain file/folder mode. To let mbv understand your folder structure, declare the nesting layout in `config.toml`:

```toml
[music]
levels = ["group", "album"]
```

Each entry names one level of folder depth. The track/file level at the bottom is always implied and should not be included. The following keywords are reserved:

| Keyword | Meaning |
|---------|---------|
| `"album"` | This level's folders are albums. Entering one activates album behaviour. |
| `"artist"` | Reserved for future use. |
| `"group"` | Reserved for future use (genre/organisational groupings). |

**Album behaviour** — when you navigate into an album folder, selecting any track auto-enqueues the entire album and starts playback from that track. Tracks are ordered by disc/track number, then by filename. Album folders show art thumbnails and release year in the list view.

**Common layouts:**

```toml
# Organisational groupings → albums → tracks
levels = ["group", "album"]

# Conventional artist → album → tracks
levels = ["artist", "album"]

# Albums at the library root → tracks
levels = ["album"]
```

If `[music]` is omitted, the music library behaves like any other folder library.

## Mouse

- Click a tab to switch to it.
- Click a row to move the cursor; double-click to play or open.
- Right-click an item to open its context menu.
- Double-click the seek bar to jump to that position; click and drag to scrub.
- Click the audio or subtitle label in the controls bar to cycle tracks.
- Scroll wheel moves the cursor in any list.
