## Context

See `proposal.md` for the motivation. Playback initialization currently sets a
50MiB forward demuxer limit and a 10MiB retained/back limit before loading the
queue. Feed entries are passed to mpv as direct network URLs, while Emby items
continue to use the Emby streaming URL path. The player run can contain both
item kinds.

## Goals / Non-Goals

**Goals:**

- Give network video feeds a 100MiB retained demuxer-cache budget.
- Preserve the existing 50MiB forward limit and mixed-queue behavior.
- Keep the adjustment local to playback initialization, with no new dependency,
  protocol field, or user configuration migration.

**Non-Goals:**

- Selecting a different yt-dlp format or changing YouTube URL resolution.
- Enabling or selecting hardware video decoding.
- Introducing adaptive buffering, per-feed configuration, or a new cache UI.

## Decisions

### Set the retained cache limit at playback initialization

Change the playback run's retained/back demuxer limit from 10MiB to 100MiB while
leaving the 50MiB forward limit intact. The options are configured once when
mpv is initialized, before the queue is loaded.

Configuring the policy at initialization keeps it stable when a mixed queue
transitions between Emby and feed items and avoids a per-item option transition
or restoration race. A per-feed runtime profile was considered, but would add
complexity without evidence that Emby playback needs a different limit.

### Keep cache activation and source resolution unchanged

Retain mpv's existing cache activation behavior and the existing direct source
URL construction. The observed improvement came from the retained-cache limit,
so forcing cache activation or changing yt-dlp format selection would broaden
the change unnecessarily.

### Use a fixed value rather than a new configuration key

Use 100MiB as the built-in value. It is the smallest tested value that remained
smooth for the high-quality Nextlander stream, while exposing a setting would
add configuration and documentation surface before there is evidence users
need different values.

## Risks / Trade-offs

- **Additional memory use:** The retained cache may use up to 100MiB in addition
  to other player buffers. → Keep the forward limit at 50MiB, avoid the much
  larger experimental values, and verify playback remains stable on the local
  daemon.
- **The back-buffer interaction may be stream-dependent:** A larger retained
  cache may not eliminate all buffering caused by CDN throughput or decoder
  pressure. → Validate against the reproduced high-quality video feed and keep
  hardware decoding and format selection as separate follow-up changes.
- **Global playback-run scope:** Emby playback also receives the larger retained
  limit. → This preserves one stable policy for mixed queues and does not change
  Emby URL or queue behavior; revisit per-feed profiles only if memory impact is
  observed.

## Migration Plan

No persisted data, protocol version, or user configuration changes are needed.
Deploying the new binary applies the larger limit to newly initialized playback
runs. Rollback is a one-line option-value revert to the previous 10MiB limit.
