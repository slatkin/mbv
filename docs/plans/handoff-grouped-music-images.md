# Handoff: grouped music images not showing (unresolved)

Date: 2026-09-13 · Worktree: `unify-screens-under-panel-components` · Status: **not fixed, reverted**

## Symptom

In the grouped Music view (albums grouped by artist under the
`LibraryPanel`'s `MusicContent` owner), album artwork does not load for the
vast majority of albums: the reserved image box shows only the shared
placeholder. A minority of albums do load art.

## Investigation so far (evidence, not guesses)

1. **The projection pipeline is intact.** An end-to-end test was written
   (`completed_album_image_reaches_owner_hero_state_as_ready`, since reverted
   with the fix attempts) proving that a *completed* fetch under key
   `{album_id}:P` reaches `MusicContent`'s hero as `HeroImageState::Ready`
   through the real sync path (`sync_mounted_surfaces` →
   `sync_library_hero_images` → `project_hero_image` → `set_active_hero_image`)
   and would be painted by `paint_panel_hero_image`. Caveats for re-adding it:
   the test must do one real `Terminal::draw` first (the hero projection gates
   on `RootFrame.library` via `library_panel_content_area`, which is only
   populated once a frame is drawn), and it must pre-seed the completion
   **before** the first sync — otherwise the stub's spawned fetch thread sends
   its own `(key, None)` completion and overwrites the seeded image.
2. **The fetch chain is unchanged by the panel migration.** The `AudioChild`
   probe in `spawn_image_fetch` (`src/app/images.rs`) is byte-identical to the
   pre-migration code (compare with `49e3fa8c:src/app/images.rs`).
3. **Therefore the runtime failure is fetches resolving empty** — and a
   resolved-empty fetch (`entry.img == None`) is *final for the session*:
   `project_hero_image` returns `State::None` and `queue_card_image_fetch`
   dedupes against the existing state forever, so one failed probe means a
   permanent placeholder until the app restarts.

## Prime suspect

The album art chain (`MUSIC_ALBUM_IMAGE_TYPES = ["AudioChild"]` in
`src/app/render/components/widgets.rs`) probed **only** the first audio
child's Primary image:

```text
GET /Items?ParentId={album_id}&IncludeItemTypes=Audio&Limit=1&api_key=…
→ Items[0].Id
GET /Items/{child_id}/Images/Primary?maxHeight=400&quality=80&api_key=…
```

Failure modes that would produce "vast majority of albums":

- The first track has no embedded Primary image (404) → chain yields nothing,
  even when the album's own Primary image (Emby metadata art) exists. Most
  likely cause: the old widgets.rs comment claims album Primary images are
  "not reliable", but the chain never even *tried* them; metadata-filled
  libraries typically carry album-level art.
- Tracks nested under disc parents: the probe is not `Recursive=true`, so
  `/Items?ParentId` may return disc folders instead of Audio items.
- Disk-cache note: only successful fetches are written to
  `read_image_disk_cache`/`write_image_disk_cache`, so cache hits explain why
  *some* albums load (plus first-track-art albums).

## What was tried and reverted

- `e1bec1e6` — restored the deleted neighbour pre-warm
  (`prewarm_grouped_music_album_images`, lost in the 9.x panel migration).
  Reverted (`b80ad2e2`): pre-warm only speeds up art that the fetch chain can
  already resolve; it did not fix the placeholders.
- `d813ec61` — album `Primary`-first chain + `Recursive=true` child probe +
  shared chain constructor. Reverted (`937cb6f9`): **unverified at runtime** —
  the user reported no improvement, but it is *unknown whether they rebuilt
  and restarted*. Both commits are referenced here; restore them (or their
  ideas) only behind a confirmed reproduction.

## Critical unknown for the next agent

**Confirm the reproduction before changing any more code.** Specifically:

1. Was the user running a build that contained `d813ec61`? The resolved-empty
   marker is per-session, so testing requires a **fresh start of the new
   build** — existing sessions keep placeholders regardless.
2. Which presentation shows the placeholders (Wide hero header, Narrow inline
   hero, or both)? Does art ever appear for an album the user sits on for
   10+ seconds?
3. Do images still load elsewhere (movie posters via `fetch_nearby_movie_posters`
   under `:cmp_primary`, TV series art)? That discriminates "music-only chain
   fails" from "hero pipeline broken at runtime".

There is no logging framework initialised (`log::debug!` calls go nowhere);
if runtime evidence is needed, add a temporary `eprintln!` (or init
`env_logger`) around the `AudioChild` arm in `spawn_image_fetch` to log the
probe URL/child id/HTTP result for a few albums, and have the user run the
app against their server. Do not commit diagnostics.

## Candidate fixes (in order)

1. Album `Primary` first, then the (recursive) `AudioChild` probe — as in the
   reverted `d813ec61`, but **only after** confirming the user actually ran
   it; if their album art is track-embedded-only, this changes nothing.
2. If the `/Items?ParentId` probe itself fails on their server, log the raw
   response; older Emby versions may require `UserId`, or the library may
   organise albums differently.
3. Make the resolved-empty marker retryable for music keys (today one failed
   probe poisons the session), instead of/in addition to chain changes.

## Worktree notes

- Reverts are `937cb6f9` and `b80ad2e2`; the two reverted commits are kept in
  history (`d813ec61`, `e1bec1e6`) for reference.
- Unrelated dirty files in the worktree (`library_panel/wide.rs`,
  `render/components/widgets.rs`, `queue_playback_panel.rs`,
  `render/arrangements/chrome.rs`, `shell_chrome_panels.rs`) belong to other
  in-flight sessions — do not commit or revert them.
- `mini_view_panel_does_not_overlay_queue_on_mode_switch`
  (`tests_queue_regression.rs`) fails at HEAD independent of all of this.