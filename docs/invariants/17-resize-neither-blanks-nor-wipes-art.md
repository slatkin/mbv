# Invariant 17 — A terminal resize neither blanks the terminal nor wipes the image cache

**Scope:** the terminal-resize paths `src/app/shell.rs::apply_terminal_observer` and
`src/app/shell/run.rs::Model::sync_terminal_resize`, and the image cache they used to
touch (`crates/mbv-images/src/cache.rs::clear_images_and_loading`). Landed by the
`panel-expand-toggle` change.

## The invariant

1. Adopting a terminal resize — the `Resize` observer event, including the one that follows
   a `pinwin set-size` request — SHALL NOT set `App::force_clear` and SHALL NOT call
   `ImageCache::clear_images_and_loading`.
2. The new geometry reaches the screen through ratatui's own autoresize repaint, which
   repaints every cell. The cache keeps its decoded sources, its fetch dedup and the hero
   cover-fit state; a painter re-encodes through the per-key protocol resize path
   (`crates/mbv-images/src/protocol.rs`, `mbv-images/src/resize.rs`) at the new geometry.
3. `clear_images_and_loading` stays correct for the paths that change *which* images are
   wanted — logout, service switch, session teardown, LRU eviction. The invariant is about
   resizes, not about clearing in general.
4. `App::force_clear` stays correct for the paths that must repaint everything (`Ctrl+L`,
   library loads, playlist edits). A resize is not one of them.

## Why it matters

Measured on niri during `panel-expand-toggle`, with a dense queue and the 0.3.4
in-process width change: `terminal.clear()` blanked the panel for 68–99 ms in the middle of
the change, and the two cache wipes (one in each resize path) dropped the decoded sources
and the fetch dedup, so every visible card refetched over HTTP and re-encoded off-thread
while painters showed dim placeholders — the art appeared only ~350–550 ms after the change
had finished. The refetch burst is also several hundred KB written to the terminal in one
go. The 0.4 model delivers one new size per change instead of one per frame, so a wipe
would cost one such burst per change, but that burst is still avoidable and still delays
every visible card.

## How the code maintains it today

Both resize paths only adopt the new size (`terminal_width` / `terminal_height` and the
`pending_terminal_resize` marker) and leave the cache and `force_clear` alone. A pinned
width change is one `pinwin set-size` request; the new size reaches the `Resize` observer
as the terminal's own event, and `sync_terminal_resize` adopts it like any other resize.
`ImageCache::clear_images_and_loading` is reached only through session teardown
(`clear_session_image_work`) and the eviction paths; no resize path calls it. `force_clear`
is set by `Ctrl+L`, library loads, playlist edits and modal actions, and consumed by the
render cadence.

## Where it still fails

A resize still costs one re-encode per visible card on the single resize worker, so the
first art paint after a width change is ~400 ms late. That latency is the worker's, not a
cache wipe; shortening it would mean encoding inline or on more than one worker.
