# Proposal

## Why

The Emby homevideos feed view still carries its pre-New-pill hack: it hides every watched video (the All pill and every folder pill list unplayed videos only) and enforces that hiding through three separate removal paths — including vanishing a video the moment its playback completes. It also materializes the whole library eagerly: one or two large fetches plus one server round-trip per video just to learn which folder each video belongs to. The New pill now owns "what's new and unwatched", so this hack has no job left, and as the library grows the eager build becomes the view's permanent cost.

## What Changes

- The All pill shows every video under the library root regardless of watched state; the existing oldest-first ordering is unchanged.
- Folder pills are driven by the folder listing plus per-folder video counts: every folder holding at least one video gets a pill, watched or not; a folder with no videos gets no pill.
- Pill contents load lazily: selecting a folder pill fetches that folder's videos at selection time, and the All list is fetched when All is selected.
- Marking a video played — context menu, multi-selection, or playback completing — no longer removes it from the view.
- The eager aggregate machinery is deleted: the full-library fetch, the per-video folder-membership round-trips, and the in-memory grouping.
- The New pill is unchanged: same source, same content, same markers.

## Capabilities

### New Capabilities

- `feed-home-video-group-view`: the Emby homevideos feed view's group behavior — what the All pill and folder pills contain, how pill membership is decided from folder counts, lazy loading of pill contents, and played-state retention when videos are marked played.

### Modified Capabilities

<!-- none: the New pill's Latest behavior is owned by destination-latest-modes and does not change; context-menu row resolution and the canonical media-list composition are unaffected. -->

## Impact

- `src/app/dispatch/feeds/feed.rs` — aggregate spawn, its helpers, and the group-building logic are removed or reduced; the feed-view state machine loses the aggregated list.
- `src/app/dispatch/context_menu/actions.rs` and `src/app/dispatch/library/browse/tv.rs` — the three feed-view removal branches lose their feed-view paths (the unplayed-only branches for ordinary libraries stay).
- `crates/mbv-ui-model/src/feed.rs` — `FeedHomeVideoState` loses `all_items`/`groups` materialization and gains lazy per-selection content.
- `crates/mbv-components/src/emby_library_content/panel.rs` and `src/app/shell/emby_library_content.rs` — the pill row is fed from the folder listing and the per-folder video counts instead of the aggregate's group list.
- `crates/mbv-emby/src/client_library.rs` — the Emby client gains the per-folder videos-only count query (the DTO already parses count fields); the ranged listing fetch itself is unchanged.
- Count-field semantics come from public Emby documentation, recorded in the change's `api-findings.md` (D1); no live server probe is made.
- No configuration, ctrl-protocol, or daemon changes.
