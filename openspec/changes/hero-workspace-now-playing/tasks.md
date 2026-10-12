# Tasks

## 1. Shell now-playing projection

- [ ] 1.1 Add the shell-owned `WorkspaceNowPlaying` projection (discriminant-aware item identity for
  Emby constituents; book identity plus derived chapter index for Audiobookshelf; clamped live
  progress) and derive it in the shell from `displayed_playback_state()`, `pending_playback_slot()`,
  and the active queue item, mirroring `queue_media_row_at`'s active/pending rule. Verify with unit
  tests: Emby episode match, music track match, Audiobookshelf book position-to-chapter derivation,
  pending-slot selection, and no-active-item absence.

## 2. TV episode Workspace

- [ ] 2.1 Add a red-before-green buffer test: with the projection set, the playing episode's row in
  `refresh_episode_rows` carries the `NowPlaying` state with live progress and other rows keep their
  derived state; identical input skips `set_content` (existing `last_episode_rows` guard).
- [ ] 2.2 Add `TvContent::set_now_playing`, apply the override at the `refresh_episode_rows` seam,
  and wire the projection into `push_tv_workspace_content`; verify the shell test drives the row
  state from a playing queue item.

## 3. Music Workspaces

- [ ] 3.1 Add a red-before-green test: with the projection set, the playing track's row in both the
  album-track and artist-track Workspace projections carries `NowPlaying { progress }`; other rows
  and headings are unchanged.
- [ ] 3.2 Add `MusicContent::set_now_playing`, apply the override where the album and artist track
  rows are projected, and wire the projection into `push_music_workspace_content`; verify with a
  shell test.

## 4. Audiobookshelf chapter Workspace

- [ ] 4.1 Add a red-before-green test: with the projection naming the playing chapter, that chapter
  row carries `NowPlaying { progress }` and other chapters stay `Ordinary`; a projection naming a
  different book lights nothing.
- [ ] 4.2 Add `BookContent::set_now_playing` with its own dedup guard and apply it at the chapter
  row projection seam without a full `set_content` rebuild; call it from the sync pass and verify a
  shell test drives chapter state from a playing book's position.

## 5. Overlay parity and integration

- [ ] 5.1 Add a Library-panel paint test proving the Library Hero overlay renders the same
  now-playing row the Wide Workspace paints (one owner, one unified look), per the
  `library-hero-overlay` workspace-parity contract.
- [ ] 5.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo nextest run` for the touched packages; fix or split anything they surface.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result.
