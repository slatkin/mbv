# Handoff: grouped music images (resolved)

Date: 2026-09-13 · Worktree: `unify-screens-under-panel-components`

## Symptom (as reported)

Grouped Music (albums grouped by artist under the `LibraryPanel`'s
`MusicContent` owner) showed the shared placeholder for almost every album;
the few that loaded were painted **Portrait**. Album art did resolve in the
Queue column's playback panel, and still does.

## Root cause

Grouped Music's album rows are Emby **`Folder`** items — a folder-view music
library (`[library.music] levels = ["group", "album"]`) lists album folders as
plain `Folder`s, exactly as `library_browse_actions.rs`'s album-index comment
already documents. The panel migration replaced the music painters'
unconditional call

```text
fetch_card_image(inline_album_art_cache_key(&album.id), album.id, series_id,
                 MUSIC_ALBUM_IMAGE_TYPES)          // {id}:P, ["AudioChild"]
```

with the shared `emby_artwork_policy`, whose music arm is gated on
`item_type == "MusicAlbum" || is_audio()`. A `Folder` album matches neither, so
every album row fell to the tag-based arm:

- no declared image tags → `source: None` → **no fetch is issued at all**, the
  reserved box stays a placeholder (the vast majority);
- a declared `Primary` tag → `Portrait` + `{id}:Primary,Backdrop,Logo` (the
  handful that loaded, painted in the wrong shape).

Observed in `~/.local/state/mbv/mbv.log`:

```text
[img] policy id=525079 type=Folder media= collection= folder=true primary="" music=false
[img] policy id=525081 type=Folder media= collection= folder=true primary="db6906…" music=false
```

The Queue column was unaffected because its card path keys off the queued
**Audio track** (`is_audio()` is true there), so it never hit the arm.

## Fix

`hero.rs` factors the album chain into `album_source()` (one constructor, shared
by the `MusicAlbum` arm and the new one so the chain and `{id}:P` key cannot
drift), adds `music_album_artwork()` (always Square, always the album chain) and
`hero_content_music_album()`. `MusicContent` uses that producer for its rows:
the destination knows its rows are albums, so it asks for the music arm instead
of a type dispatch that cannot see them.

## Why the two earlier attempts could not work

Both changed the *chain* for items typed `MusicAlbum` (`d813ec61`,
`e1bec1e6`), which these rows are not — `music_source` was never reached, so
neither the chain nor the pre-warm could affect the outcome. The handoff's
"prime suspect" (a failing `AudioChild` probe) was wrong for the same reason.

## Server evidence (read-only probes against the user's Emby)

```text
GET /Items?ParentId={album}&IncludeItemTypes=Audio&Limit=1  → 200, returns the first track
GET /Items/{track}/Images/Primary                           → 200, image/jpeg
GET /Items/{album}/Images/Primary   (album without a Primary tag) → 404, text/plain, 45 bytes
GET /Items/{album}/Images/Primary   (album with a Primary tag)    → 200, image/jpeg
```

So the `AudioChild` probe is healthy, and album-`Primary`-first is the wrong
chain for this library — which is why the fix restores the pre-panel chain
rather than adding one.

## Remaining risks (not addressed here)

1. `spawn_image_fetch` writes **any** HTTP body to the disk cache when a
   candidate returns bytes, including the 45-byte `text/plain` 404 body. That
   poisons `{album_id}:P` permanently: every later run short-circuits on
   `read_image_disk_cache` and decodes nothing. A "cache only what decodes"
   guard would make chains safe to extend, and is the precondition for ever
   trying album-`Primary`-first.
2. A resolved-empty fetch is still final for the session (`project_hero_image`
   returns `None` and the fetch dedupes against the existing state).
