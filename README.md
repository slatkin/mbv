# mbv

A terminal client for Emby, Audiobookshelf, and Feeds that browses catalogs and
plays media. Playback may run inside the terminal process itself, or be hosted
by a background process on the same machine so it survives the terminal closing.

You can browse Emby libraries, Audiobookshelf podcast libraries, or RSS/Atom
feed subscriptions (including YouTube channels normalized to RSS). Build a mixed
queue from any combination of Emby items, feed episodes, and downloaded
Audiobookshelf podcast episodes (bare-mode Audiobookshelf playback is active;
Local daemon and packaged mbvd admission is tracked in milestone #524). Queues
are canonical, ordered, and slot-addressable — duplicate content retains distinct
slot identities.

Playback stays in sync with Emby for Emby items, hydrates feed resume/played
state from an optional shared-data store, and for bare-mode Audiobookshelf
episodes reports position and monotonic listening time while finalizing the
provider playback session on every teardown path.

# Installation

## Arch Linux

The release package is built from the `PKGBUILD` in this repository and does not require AUR access. Install the build tools, download the pinned package recipe, and build it locally:

```sh
sudo pacman -S --needed base-devel curl
mkdir -p /tmp/mbv-package && cd /tmp/mbv-package
curl -fLO https://raw.githubusercontent.com/slatkin/mbv/main/PKGBUILD
makepkg -si
cd ~/ && rm -rf /tmp/mbv-package/
```

The recipe downloads the matching release binary directly from GitHub and verifies its SHA-256 checksum before creating the pacman package. Refresh the `PKGBUILD` before rebuilding to pick up a newer release.

# Features

## How it is structured

- **Singleton Services** — mbv holds at most one Emby, one Audiobookshelf, and one Feeds integration. Feeds is always present. Emby and Audiobookshelf are Remote Services with their own server URL and Service credential; mbv itself never requires Service setup before it can start (Service-independent startup, ADR 0018).
- **Service state** — NotConfigured, Connecting, Ready, NeedsAuthentication, Unavailable. Each Service has a monotonic setup generation guard so stale async completions are rejected.
- **Tab selection** — Home, N Emby libraries, M Audiobookshelf podcast libraries, Feeds last. Position mapping is count-aware so Emby and Audiobookshelf never share a numeric position (strengthen-service-browse-seam).
- **Service browse dispatch** — Exhaustive boundary: every left-panel key, mouse, refresh, render, help, and context-menu action routes to exactly one of Home/EmbyLibrary/AudiobookshelfLibrary/Feeds. Provider browse models stay separate and meet only at QueueItem construction and owner admission.
- **Home** — Continue Watching across Emby libraries plus per-library Latest sections.

## Emby-Parity Features

- **Library browsing and search** — navigate folders and series, jump to seasons and episodes, fuzzy-search within a library; per-library sticky browse position restored across restarts.
- **Resume and watched-state sync** — videos resume where you left off using a unified 6% threshold (unknown runtime always resumable when position > 0); watched status reports back to Emby.
- **Standard remote control compatibility** — any Emby remote app on phone or browser can drive mbv.
- **Session control from mbv** — connect to another active Emby session and control it from mbv. `F3` opens the session list. Emby Sessions may advertise private-LAN ctrl and shared-data ports in `supported_commands`.
- **Playlist integration** — browse Emby playlists, enqueue them, save the current queue back with `Ctrl+S`. Queue source (Playlist, Album, Series, Shuffle, Remote, Collection, Unknown) is preserved.
- **Normal playback controls** — seek, pause, adjust volume, cycle audio tracks, toggle subtitles, Next Up / Skip Intro prompts.
- **System-audio visualizer** — press `v` to switch the queue card between artwork and a PipeWire stereo vectorscope for the default system output. This intentionally may include audio from other applications; it does not reroute or change mpv playback. Requires PipeWire. Set `[display].visualizer_glyph` to change the point glyph; the default is `●`.

## mbv-Only Features

- **Dedicated persistent queue** — its own queue model, not Emby's play-next/play-later. Queue-source tracking, undo delete, jump-to-library from a queue item, queue-first workflows. The queue is mixed: Emby items, Feed entries, and Audiobookshelf podcast episodes can be interleaved with stable slot identity (`QueueSlotId`) independent of content identity.
- **Unified queue and slot identity** — one canonical ordered representation of `QueueItem` values. Append, replace, remove, move, clear, consume, and play-existing-slot operations are item-kind agnostic. Persistence round-trips tagged `QueueItem` values; legacy untagged Emby-only state remains readable.
- **Owner admission / Service eligibility** — a Player owner binds only items it can play: media kind (audio vs video), required Remote Service setup and credential loaded in that owner's process, and ctrl transport negotiation. Bare mode currently admits Emby, Feed, and Audiobookshelf when their Services are Ready; Local daemon and packaged mbvd admit Emby and Feed (audio-only owners keep only the audio subset of a mixed submission); Audiobookshelf daemon admission is tracked in issues #525-528.
- **Headless daemon mode** — `mbvd` runs the player with no terminal attached, as a background service or systemd unit; any `mbv` can connect to it as a thin client over its own protocol, not just standard Emby session control. Audio-only mbvd (`--audio-only`) never holds a video item; a mixed submission containing audio is accepted minus non-audio items. Client fall-through for explicitly requested video is tracked in issue #431 / ADR 0017.
- **Local daemon for stay-alive** — one user-owned Local daemon per user (`$XDG_RUNTIME_DIR/mbv-ctrl.sock`), started without authenticating any Remote Service, authenticated to Clients with a Control credential (not an Emby token). Any number of Clients may attach at once without evicting (multi-connection model, ADR 0014). Ordinary disconnect never stops the daemon; explicit lifecycle request does when stay-alive is off.
- **mpv-first playback model** — playback runs through embedded mpv, including headless audio. Packaged `mbvd` defaults to clocked ALSA output: mpv's `[mpv].audio_device` (`alsa` for the default endpoint, or `alsa/<device>` for an exact one) binds it to a real hardware-paced device instead of libmpv's untimed `ao=pcm` file writer. Bare mode and the Local daemon are unaffected. For Snapcast, the topology is `mbvd`/libmpv → ALSA playback endpoint → paired ALSA capture endpoint → Snapserver: provisioning the loopback (e.g. `snd-aloop`), exposing both endpoints (including LXC/container device mapping and permissions), pointing `audio_device` at the playback side, and matching Snapserver's capture/`sampleformat` to it are all operator-owned — mbv does not create, load, or manage that hardware, Snapserver, Snapclient, or their downstream buffering. Set `[mpv].audio_pipe_enabled = true` to use the legacy PCM pipe path instead; its `audio_pipe_samplerate`/`audio_pipe_bitdepth` (`16`, `24`, or `32`, must match Snapserver's `sampleformat` exactly) and `audio_pipe_playout_delay_ms` (a manually calibrated startup/duplicate-play estimate, not an audibility guarantee — mbv neither queries nor controls the downstream consumer) remain pipe-only and unaffected by `audio_device`. Both output selections are restart-required and config-only, with no live toggle; roll back from ALSA to the pipe (or the reverse) by flipping `audio_pipe_enabled` and restarting `mbvd`. When a lifecycle-backed source (Audiobookshelf) is active, mpv switches to active-file projection: canonical queue retains all slots, mpv contains only the active materialized file.

  ```toml
  [mpv]
  # audio_device = "alsa"                    # inherited default (packaged mbvd only)
  # audio_device = "alsa/hw:Loopback,0,0"     # explicit ALSA loopback endpoint
  # audio_pipe_enabled = true                 # legacy PCM pipe path instead of ALSA
  ```
- **Opinionated playback defaults** — English audio preferred, subtitles start off, image-based subtitle tracks hidden because they don't work in headless mpv.
- **Special music library handling** — folder-shaped music libraries via `[music].levels`, with grouped browsing standard Emby clients don't offer. Includes recursive album search across all configured ancestor levels.
- **Sticky library position** — browse position within each library (including drill depth, focused item, letter-range pill, sort, and for feed-view libraries selected group) is saved and restored across restarts.
- **Auto-reconnect for daemon routes** — when enabled, reconnects to the last active daemon-routed library at startup via the `[general] auto_reconnect` option.
- **Library routes** — persistent per-library `lowercased name -> tcp://host:port` endpoint, device name transient in the F2 picker, immediately resolved before persisting. Connecting via Sessions panel tears down an active route. Malformed non-`tcp://` values logged and skipped.
- **Feed-library defaults** — chosen libraries behave like feeds, unplayed and date-sorted — good for YouTube-style libraries.
- **Feeds (RSS/Atom/YouTube)** — per-user `[[feeds]]` subscriptions in `config.toml` with Audio|Video classification (`FeedKind`). Fetched entries not persisted; per-entry resume (`position_ticks`) and `played` state roams via shared-data `feed_entry_state` table `(user_id, feed_id, entry_guid)` with last-write-wins semantics and prefix scan. Degrades to zero/unplayed without shared daemon. YouTube channel URLs normalized to RSS on subscribe. All/Watched/Unwatched filter (`w`). Optional idle feed RSS URL rotates in playback panel.
- **Audiobookshelf podcasts (bare-mode active, daemon in #524)** — singleton Audiobookshelf Remote Service with per-Service 0600 secret, setup generation guard, paged podcast show listing, downloaded episode inline browsing, TV-identical hero + column layout, All/Played/Unplayed episode filter, provider-qualified queue items `libraryItemId+episodeId`, typed content identity, just-in-time direct/HLS source preparation with scoped Bearer header, bounded HLS readiness, active-file mpv projection, periodic position + monotonic wall-clock listening sync, bounded session close before next session, generation-safe progress reconciliation to queue and browse state. Daemon transport/admission/continuity tracked in issues #525-528.
- **Extra local control surfaces** — MPRIS lets desktop widgets, `playerctl`, and media keys control mbv, including rebind handle.
- **Desktop-integrated prompts** — with `system_notifications = true`, Skip Intro, Next Up, and queue prompts show as actionable desktop notifications.
- **Search sidebar** — global cross-library search for navigable media types (Series/Episode/Season/Movie/Audio/MusicAlbum/MusicArtist) with type-filter pill and stale-query guards. Panel focus/mode cycle `x` (Both/LibraryOnly/QueueOnly).

## Shared mbv State

Shared state is disabled by default. It requires one boolean in the system
daemon config and one boolean in each client config. The daemon advertises its
shared-data endpoint through Emby; clients discover it automatically.

On `main`, packaged `mbvd` still requires cached Emby credentials and uses
legacy Emby-token ctrl authentication (`crates/mbvd/src/main.rs`). Service-
independent startup (zero Services, optional Emby runtime), filesystem/trusted-
LAN authorized ctrl, and `mbvd --connect emby` administration are implemented in
open PR #529 tracking issue #523 and are not described as landed.

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
client validates the certificate before sending its Emby token. WAN endpoints
are rejected before any credential is sent.

Roaming documents (four revisioned per user, CAS with stale toast): queue state,
library position state, last remote connection, roaming settings (exactly
`auto_reconnect` and `library_routes`, mirrored via `roaming_settings.json` not
`config.toml`). Plus keyed feed entry state table `(user_id, feed_id, entry_guid)`
holding `position_ticks`/`played` with last-write-wins and prefix scan, negotiated
as additive `shared-mbv-feed-entry-state-v1` capability.

Shared queue, library-position, reconnect, and roaming-settings documents are
mirrored locally. If `mbvd` is unavailable, browsing and playback continue from
the local mirror and background reconnect attempts use bounded exponential
backoff. Existing shared documents win when a client reconnects. Database
open/corruption/serialization/disk-full failures fail hosting or that operation
without stopping playback or corrupting committed data.

To inspect an existing host database locally without starting the daemon, run
`sudo MBV_SYSTEM=1 mbvd --export-shared-data`. Setting either boolean to false
returns the client to local-only behavior; the database is preserved for later
re-enablement or export.

### First-time setup

1. Edit `/etc/mbv/config.toml`: set the Emby `[server].url` and
   `[shared_data].enabled = true`.
2. Currently (main) run `sudo MBV_SYSTEM=1 mbv` once and log in. This writes the
   system daemon's credentials under `/var/lib/mbv`; quit after login. After PR
   #529 lands, the supported administration is `sudo MBV_SYSTEM=1 mbvd --connect emby`
   (prompts for server URL, username, password; validates before commit; preserves
   working setup on failure; reconciles a running daemon when possible).
3. Allow TCP port `47789` from the intended private LAN, then restart `mbvd`.
4. In each client's `~/.config/mbv/config.toml`, set
   `[shared_data].enabled = true` and restart `mbv`.

The first client initializes empty shared documents from its local state. To
roll back, set `enabled = false` on the clients or daemon. Local mirrors and the
daemon database under `/var/lib/mbv` are preserved.
