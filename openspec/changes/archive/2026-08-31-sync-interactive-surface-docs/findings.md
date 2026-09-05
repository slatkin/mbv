# Task 2.2 — Items not made true by editing documents alone

Recorded 2026-08-31. The docs now *disclose* each item truthfully; making the
underlying invariant literally hold requires the code change named below. Not
done in this change (per Non-goals).

## 1. Queue surface has two painters

Ledger row **Queue** is `all: F2`: legacy `render_queue` underpaints from the
deliberately-stale `App::queue_cursor` (`src/app/shell_run.rs:56`) beneath the
`QueueComponent` overlay (`shell_run.rs:72`). This contradicts the merged spec
requirement "Interactive surfaces have one owner and one painter" / "A
`migrated` surface SHALL have exactly one painter for each frame".

- **Disclosed:** ledger Queue row, deferred to `remove-queue-legacy-underpaint`
  (#629).
- **Code change needed:** delete the legacy `render_queue` underpaint call and
  the stale `App::queue_cursor` field so `QueueComponent` is the sole painter.
  Scope of #629.

## 2. `FeedHomeVideoState::video_cursor` is an App-held live cursor

Since #625 (3.5) the narrow feed/home-video group picker is painted by
`BrowserComponent`, but its cursor is still read off `App`
(`FeedHomeVideoState::video_cursor`, `src/app/types_feed.rs:17`, threaded via
`NarrowBrowseExtras::feed_video_cursor` in `list_narrow.rs`). Painter and cursor
owner are split, so #607's "component-local interaction state has one owner" is
literally false for this surface.

- **Disclosed:** ledger carries the D6 carve-out; `split-browse-state-interaction-fields`
  design D6 explicitly scopes it out.
- **Code change needed:** move `video_cursor` ownership into `BrowserComponent`
  for the feed-group view and thread it like the other browse cursors; retire
  the `FeedHomeVideoState::video_cursor` field and `feed_video_cursor` extras
  plumbing. Deferred by D6.

## Not a contradiction (listed for completeness)

`PanelMode::QueueOnly` playback chrome (ledger Playback row): legacy
`render_main` player panels paint because `player_area` is empty in queue-only
mode. The spec sanctions "a breakpoint with no component keeps a sole legacy
painter", so this is consistent. No code change required.
