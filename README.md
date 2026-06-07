# mby

A terminal UI client for [Emby](https://emby.media) media servers. Browse your libraries, build a queue, and play media — all from the terminal. Control playback from any Emby remote app on your phone or browser, or let mby run headless as a background daemon. The name 'mby' stands for "My Bloody Yalentine."

This was built with Claude Code because I am lazy and I already have a job (for now).

## Requirements

- Rust toolchain (for building)
- [mpv](https://mpv.io) libraries (`libmpv`)

## Installation

```sh
make install
```
Or if you are on Arch just use AUR.

```sh
paru -Syu mby
```

Installs the binary to `~/.local/bin/mby`.

## Usage

```sh
mby          # launch with terminal UI
mby --daemon # run headless, controlled via remote only
```

## Configuration

On first run a login screen prompts for server URL, username, and password. Credentials are saved after a successful login.

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

### What you can do

- **Browse your libraries** — navigate folders and series, jump straight to a show's seasons and episodes, or fuzzy-search within any folder.
- **Build a queue** — add individual items or entire folders to the Queue from any screen. Folders are expanded automatically. Play from any position in the queue; the rest follows in order.
- **Pick up where you left off** — videos resume from their saved position. Watched status is synced back to the server automatically.
- **Full playback controls** — seek, pause, adjust volume, cycle audio tracks, enable subtitles, all from the keyboard.
- **Home screen** — see your Continue Watching items and what was recently added across your libraries.
- **Remote control** — any Emby app on your phone or browser can control mby in real time: play, pause, seek, skip, adjust volume, queue items.
- **Daemon mode** — run mby as a background service with no terminal required. Register it with your server and drive it entirely from remote apps.
- **System media keys** — mby exposes an MPRIS2 interface so desktop widgets, `playerctl`, and system media keys all work automatically.

### Media Playback details

- Media plays through an embedded mpv instance. Playback is synchronised between mpv, the mby client, and Emby server.
- Audio plays headless by default (no mpv window). Set `show_audio_window = true` to change this.
- If you want, your personal `~/.config/mpv/mpv.conf` is respected (shaders, audio devices, renderer settings, keybindings).
- The mpv IPC socket lives at `$XDG_RUNTIME_DIR/mby-mpv.sock`, separate from the default mpv socket, so running mby alongside standalone mpv doesn't cause conflicts.
- By default mby uses its own bundled OSC ([mpv-osc-modern](https://github.com/maoiscat/mpv-osc-modern)). Set `use_mpv_config = true` to defer to your own `~/.config/mpv/` setup instead.

## Key bindings

Press `F1` at any time to open the built-in reference screen.

### Global

| Key | Action |
|-----|--------|
| `F1` | Keyboard shortcut help |
| `Tab` / `Shift+Tab` | Cycle tabs forward / backward |
| `1`–`9` | Jump to tab by number |
| `↑` / `↓` | Move cursor |
| `PgUp` / `PgDn` | Page scroll |
| `Home` / `End` | Jump to first / last item |
| `Enter` | Select / play / open |
| `Alt+Q` | Add selected item or folder to Queue |
| `Alt+O` | Context menu |
| `c` | Clear Queue (asks confirmation) |
| `q` | Quit |

### Playback (when player is active)

| Key | Action |
|-----|--------|
| `Space` | Pause / resume |
| `Alt+←` / `Alt+→` | Seek ±5 seconds |
| `Alt+Enter` | Stop |
| `-` / `+` | Volume down / up |
| `Alt+A` | Cycle audio track |
| `Alt+Z` | Enable subtitles |

### Queue tab

| Key | Action |
|-----|--------|
| `.` | Jump to currently playing item |
| `Delete` | Remove selected item from Queue |
| `Alt+V` | Toggle list / card view |

### Home tab

| Key | Action |
|-----|--------|
| `Alt+↑` / `Alt+↓` | Switch between sections |
| `Alt+W` | Toggle watched status |

### Library tab

| Key | Action |
|-----|--------|
| `Esc` / `Backspace` | Go back |
| `/` | Search within current folder |
| `Alt+W` | Toggle watched status |
| `Alt+S` | Shuffle and play current selection |

## Mouse

- Click a tab to switch to it.
- Click a row to move the cursor; double-click to play or open.
- Right-click an item to open its context menu.
- Double-click the seek bar to jump to that position; click and drag to scrub.
- Click the audio or subtitle label in the controls bar to cycle tracks.
- Scroll wheel moves the cursor in any list.
