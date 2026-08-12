## Why

High-quality YouTube feed entries, especially 1440p60 DASH video, repeatedly enter mpv buffering despite a fast local connection. Direct mpv testing reproduced the problem with mbv's current 10MiB demuxer back buffer and remained smooth with a 100MiB back buffer, so the player needs a larger retained cache window for network feed playback.

## Tracking

GitHub issue: [#497 — Video feeds stutter from an undersized mpv demuxer back buffer](https://github.com/slatkin/mbv/issues/497)

## What Changes

- Increase the mpv demuxer back-buffer limit used by playback runs from 10MiB to 100MiB.
- Preserve the existing 50MiB forward demuxer limit and direct yt-dlp-resolved feed URL playback.
- Keep the change compatible with mixed Emby/feed queues and local-daemon playback.
- Do not change yt-dlp format selection or enable hardware decoding as part of this change.

## Capabilities

### New Capabilities

- `video-feed-playback-buffering`: Provides a larger mpv demuxer cache window for smooth network-backed video feed playback.

### Modified Capabilities

<!-- No existing requirement set is modified; feed queue admission and URL resolution remain unchanged. -->

## Impact

- Affected playback initialization in `crates/mbv-core`, where mpv demuxer options are configured.
- Video feed playback gains a larger potential in-memory retained cache; no protocol, queue wire shape, or configuration-file migration is required.
- Emby URL construction, yt-dlp resolution, format selection, and hardware-decoding behavior remain unchanged.
