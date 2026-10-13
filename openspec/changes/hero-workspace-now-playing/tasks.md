# Tasks

## 1. Shell now-playing projection

- [x] 1.1 Add the `WorkspaceNowPlaying` projection type in `mbv-ui-model` (D1: `QueueItemContentId`
  identity, clamped live progress, and for Audiobookshelf the chapter id plus chapter-relative
  progress). Derive it in the shell from `displayed_playback_state()` and the playing queue's item at
  the active index. Contract: the Queue and every Workspace agree on which item is playing
  (design D1, D3, D6). Verify with one `#[case]` table whose cases have different outcomes: an Emby
  item match, a pending slot that moves the match, an Audiobookshelf position that resolves to a
  chapter with chapter-relative progress, a pending book that resolves to no chapter, and no active
  item or a different book that resolves to nothing.

## 2. TV episode Workspace

- [x] 2.1 Add `TvContent::set_now_playing`, apply the override at the `refresh_episode_rows` seam,
  and wire the projection into `push_tv_workspace_content`. Contract: the playing episode's row
  carries `NowPlaying { progress }`, other rows keep their derived state, and identical input skips
  `set_content` (existing `last_episode_rows` guard). Verify with one owner test, written red before
  green.

## 3. Music Workspaces

- [x] 3.1 Add `MusicContent::set_now_playing`, apply the override in `resolved_workspace` for the
  album and artist track rows, and wire the projection into `push_music_workspace_content`.
  Contract: the playing track's row carries `NowPlaying { progress }` in both the album and artist
  Workspaces, and other rows and headings are unchanged. Verify with one owner test that covers both
  variants (they share `track_row`), written red before green.

## 4. Audiobookshelf chapter Workspace

- [x] 4.1 Add `BookContent::set_now_playing` with its own dedup guard and apply it at the
  `project_chapter_rows` seam without a full `set_content` rebuild. Call it from the sync pass only
  on the Audiobookshelf Book tab and only when the owner exists (design D5). Contract: the named
  chapter row carries `NowPlaying` with the chapter-relative progress the projection supplies, and
  other chapters stay `Ordinary`. Verify with one owner test, written red before green.

## 5. Gate

- [x] 5.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo nextest run` for the touched packages; fix or split anything they surface.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result.
