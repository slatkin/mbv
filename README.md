# mby

A terminal UI client for [Emby](https://emby.media) media servers. Browse your libraries, manage a playlist, and play media through [mpv](https://mpv.io) — all from the terminal. A WebSocket connection to the server lets any Emby remote control app (phone, web UI, etc.) control playback in real time. mby can also run as a headless daemon with no TUI, driven entirely by remote control.

This was built with Claude Code because I am lazy and I already have a job (for now).

## Requirements

- Rust toolchain (for building)
- [mpv](https://mpv.io) libraries (`libmpv`)

## Installation

```sh
make install
```

Installs the binary to `~/.local/bin/mby`.

## Usage

```sh
mby          # launch with terminal UI
mby --daemon # run headless, controlled via remote only
```

## Configuration

On first run a login screen prompts for server URL, username, and password. Credentials are cached after a successful login and won't be requested again unless the token is rejected.

Optionally create `~/.config/mby/config.toml`:

```toml
[emby]
# Override the server URL (normally set via the login screen)
url = "http://emby.local:8096"

# Hide libraries from the tab bar (case-insensitive)
hidden_libraries = ["Live TV", "Podcasts"]

[mpv]
# Show an mpv window for audio playback (default: false = headless)
show_audio_window = false

# Use your own ~/.config/mpv/ setup (scripts, OSC, mpv.conf) instead of
# mby's bundled OSC. Default false = mby manages its own mpv environment.
use_mpv_config = false

[daemon]
# Show a system tray icon when running in daemon mode (default: true)
show_systray_icon = true
```

## Features

### Terminal UI

- **Home** — Continue Watching and Latest Movies / Shows sections.
- **Library browser** — single-row table with wrapping titles; navigate folders and series with real-time fuzzy search. Press `p` on a folder to enqueue all its contents into the List (asks confirmation).
- **List** — queue items from any tab; table view shows title, duration, and a mini progress seekbar. Play from any position; progress is resumed for videos. The active item's seekbar updates live during playback.
- **Playback controls** — seek bar with time and volume display; audio and subtitle track cycling.
- **Keyboard shortcut help** — press `?` from anywhere to open a cheat-sheet overlay.
- **Log tab** — live log of server communication and player events (`Ctrl+L` to open).

### Playback

- Plays media via an embedded mpv window alongside the TUI.
- Seamless track switching: starting a new item while one is already playing loads it into the existing mpv window without closing and reopening.
- Videos resume from their stored position when played from the List; the stored position is updated when playback stops.
- Audio files play headless (no mpv window) by default; set `show_audio_window = true` to override.
- Mouse-over the mpv window shows the current item title as an OSD overlay. The OSC (on-screen controller) appears on mouse movement and hides after 5 seconds. Press `F8` to show the mpv playlist.
- Media titles are passed to mpv so the OSC and `F8` playlist show item names rather than stream URLs.
- By default mby uses its own bundled OSC ([mpv-osc-modern](https://github.com/maoiscat/mpv-osc-modern)) and suppresses user scripts to avoid conflicts. Set `use_mpv_config = true` to defer entirely to your own `~/.config/mpv/` setup.
- Auto-selects English audio track and disables subtitles on each new file.
- Reports playback progress and watched status back to the server continuously.
- Loads your personal `~/.config/mpv/mpv.conf` (shaders, renderer settings, audio devices, keybinds, etc.) so your mpv setup is respected.
- Volume ceiling is read from mpv's `volume-max` setting, so if you have raised or lowered it in `mpv.conf` mby's `+`/`-` keys will respect that limit.
- The mpv IPC socket is placed at `$XDG_RUNTIME_DIR/mby-mpv.sock` (separate from the default mpv socket) so running mby alongside standalone mpv does not cause conflicts.

### Remote control (WebSocket)

mby maintains a persistent WebSocket connection to the server. Any Emby remote (mobile app, web UI) can:

- Play a single item or a full shuffled playlist
- Stop, pause, resume
- Seek, skip to next/previous track
- Adjust volume
- Navigate to any item in the current playlist

Shuffling or queuing a folder from the remote loads into the existing mpv window when something is already playing.

### Daemon mode

`mby --daemon` runs without a TUI — no terminal required. It registers with the server and responds to all the same WebSocket remote commands. A PID file is written to `~/.local/share/mby/mby.pid`.

### MPRIS

mby exposes an MPRIS2 interface so system media keys, desktop widgets, and tools like `playerctl` work automatically.

## Key bindings

Press `?` at any time to open the built-in cheat-sheet overlay.

### Playback (all tabs, when player is active)

| Key | Action |
|-----|--------|
| `Space` | Pause / resume |
| `←` / `→` | Seek ±5 seconds |
| `Alt+←` / `Alt+→` | Previous / next item in List |
| `Ctrl+Enter` | Stop |
| `-` / `+` | Volume down / up |

### Global

| Key | Action |
|-----|--------|
| `?` | Keyboard shortcut help |
| `Tab` / `Shift+Tab` | Cycle tabs |
| `1`–`9` | Jump to tab by number |
| `Ctrl+L` | Open log tab |
| `q` | Quit |

### Home tab

| Key | Action |
|-----|--------|
| `↑` / `k`, `↓` / `j` | Move cursor |
| `Enter` | Play selected item |
| `p` | Add / remove from List |
| `w` | Toggle watched status |
| `r` | Refresh |

### Library browser tab

| Key | Action |
|-----|--------|
| `↑` / `k`, `↓` / `j` | Move cursor |
| `PgUp` / `PgDn` | Page scroll |
| `Enter` | Open folder / play item |
| `p` | Add item to List; on a folder, enqueue all contents |
| `w` | Toggle watched status |
| `r` | Refresh current view |
| `Ctrl+S` | Shuffle-play current folder |
| `Ctrl+O` | Context menu |
| `Esc` / `Backspace` | Go back |
| `/` | Open fuzzy search |

### List tab

| Key | Action |
|-----|--------|
| `↑` / `k`, `↓` / `j` | Move cursor |
| `PgUp` / `PgDn` | Page scroll |
| `Enter` | Play from selected item |
| `Delete` / `Alt+P` | Remove selected item |
| `Alt+O` | Context menu (also: Remove from Playlist) |

### Log tab

| Key | Action |
|-----|--------|
| `↑` / `↓` | Scroll |
| `c` | Toggle text-selection mode (for copying) |

## Mouse

- Click a tab to switch to it.
- Click a row to move the cursor; double-click to activate (play / open).
- Right-click an item to activate it directly.
- Double-click the seek bar to jump to that position; click and drag to scrub.
- Click the audio or subtitle label in the controls bar to cycle tracks.
- Scroll wheel moves the cursor in any list.
- Moving the mouse over the mpv window shows the current item title as an OSD.
