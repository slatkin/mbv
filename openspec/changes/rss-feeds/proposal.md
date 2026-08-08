# RSS Feeds

## Prerequisite

Requires centralized storage (mbvd-hosted redb). When mbvd is unreachable, the
Feeds feature is unavailable. Feed subscriptions and playback state live in the
shared store.

## Problem

mbv can only browse and play content from Emby libraries. Users who want to
follow RSS/Atom feeds — podcasts, audiobookshelf feeds, YouTube channels — must
use a separate app.

## Solution

Add a Feeds feature: users subscribe to feed URLs, browse entries, and play
enclosure URLs through mpv. Playback state (position, watched) persists in the
shared store and roams across machines.

- **Tab**: Appears after libraries when feeds are defined.
- **Layout**: Reuses the existing feed-view structure. Pillbar selects feed;
  "All" pill shows entries from all feeds.
- **Metadata**: Comes from the feed (title, description, duration, pub date).
- **Management**: Sidebar panel for adding/removing/editing subscriptions.
- **Refresh**: Async on app launch and via manual keybinding.
- **Playback**: Entries queue and play like Emby items. Progress reports write
  to the shared store instead of Emby.

## Non-goals

- Downloading or importing media into Emby.
- Daemon-side polling (clients poll).
- Config-file feed definitions (definitions live in shared store).
- Mixed audio/video within a single feed (per-feed kind).
