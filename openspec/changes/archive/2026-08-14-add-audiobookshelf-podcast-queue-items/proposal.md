## Why

Downloaded Audiobookshelf podcast episodes need a provider-native queue representation before playback code can handle them safely. Establishing identity, persistence, Service ownership, and owner admission first prevents later source and reporting work from embedding Audiobookshelf data in Emby or Feed paths.

## What Changes

- Require #513's provider-specific browse seam as the implementation baseline for milestone #515.
- Add an Audiobookshelf podcast QueueItem identified by Service kind, `libraryItemId`, and `episodeId`.
- Add typed Service-qualified content identity for matching and reconciliation while preserving independent QueueSlotId occurrence identity.
- Extend queue accessors, metadata projection, operations, persistence, restoration, and rendering for the new item kind.
- Extend owner admission to evaluate required Remote Service capability as well as media kind, without constraining Composed queue editing.
- Keep every current Player owner ineligible for Audiobookshelf items in this change; later source and reporting changes enable the in-process owner only when playback is complete.
- Preserve repairable staged and persisted items after credential rejection, but purge Audiobookshelf-owned queue state on confirmed Service replacement or removal.
- Keep playback-session APIs, stream resolution, mpv projection, progress writes, and episode activation out of scope.

## Capabilities

### New Capabilities

- `audiobookshelf-podcast-queueing`: Provider-native queued episode identity, Service ownership, persistence, and admission requirements before playback is enabled.

### Modified Capabilities

- `unified-playback-queue`: Admit a third QueueItem kind and extend owner binding from media-kind checks to media-kind plus required-Service capability.

## Impact

- Queue source-of-truth types, accessors, persistence, restoration, status metadata, and reconciliation in `mbv-core`.
- App queue submission/admission, Service lifecycle cleanup, and cold non-Emby queue construction.
- No Audiobookshelf API request, mpv source, Player reporting lifecycle, ctrl capability, protocol version, or credential transfer.
