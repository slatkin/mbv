# Design — hero-workspace-now-playing

## Context

The Queue projects its active row through `queue_media_rows`
(`crates/mbv-components/src/queue/rows.rs`): the active slot derives
`MediaSemanticState::NowPlaying { progress }` from `displayed_playback_state()` plus
`pending_playback_slot()`, and the shared `media_list` row painter renders it. Workspace row
builders (`tv_content/episode_rows.rs`, `music_content/workspace.rs`,
`book_content/chapters.rs`) derive state only from item facts
(`MediaSemanticState::from_emby`), and no shell push carries playback identity into a workspace
owner.

The Music and TV owners already re-project every sync pass
(`push_music_workspace_content`, `sync_tv_content` in `shell/run.rs`); the Audiobookshelf book owner
re-projects only on ABS event drains. Owner row refreshes dedup with the `last_*` rows pattern so an
identical re-projection skips `set_content` and keeps the painted frame claimable.

## Goals / Non-Goals

**Goals:**

- One shell-owned now-playing projection that the Queue and every Workspace read from, so the two
  surfaces cannot disagree about which row is playing.
- Live progress in the workspace row with no new tick path: reuse the sync pass.
- Row builders stay pure; owners apply the override.

**Non-Goals:**

- Remote Emby sessions (another device's now-playing item) never light a workspace row; only the
  local queue's active item matches.
- Grouped Music tree track items, Home rows, podcast rows, and feed rows are not workspaces and are
  out of scope.
- No paint-vocabulary change: the painter already renders the `NowPlaying` state.

## Decisions

**D1 — One projection type, computed in the shell.** A new shell-owned projection value
(`WorkspaceNowPlaying`) carries exactly what a workspace owner needs: for Emby constituents, the
queue item's own identity (service kind + Emby item id); for Audiobookshelf books, the book identity
plus the derived playing-chapter index; and the clamped live progress percentage. The shell computes
it once per sync pass from `displayed_playback_state()`, `pending_playback_slot()`, and the active
queue item — the same inputs the queue row projection reads. Alternative: push `PlaybackState` and
the queue into owners — rejected: components never receive queue or protocol objects; the
boundary-crossing value must be presentation data with a stable identity, like `latest_marker`.

**D2 — Identity matching, discriminant-aware.** The projection carries the queue item's variant
identity, not a bare string, so an Emby item id can never match an Audiobookshelf id. A workspace
row matches when its stable target equals the active item's identity within the same provider kind.
Alternative: match by queue lineage (only queues composed from this workspace) — rejected by user
direction: identity matching regardless of origin.

**D3 — Chapters derive from book position in the shell.** For a book workspace the queue item is the
book; the shell resolves the chapter that contains the playback position against the chapter list
the shell already holds in `AudiobookshelfBookBrowseState`, and pushes the chapter index. The owner
never reads playback state. Alternative: push raw position and let the owner resolve the chapter —
rejected: derivation is projection, and owners stay presentation-only.

**D4 — Owners apply the override at their row-refresh seam.** Each owner gains a
`set_now_playing(Option<...>)` that stores the projection and applies it when it rebuilds its rows:
the matching row's semantic state becomes `NowPlaying { progress }`, every other row keeps its
derived state. Builders (`build_episode_rows`, `track_row`, chapter projection) stay pure. The
existing `last_*` dedup guards now compare rows that include the override, so unchanged input skips
`set_content`.

**D5 — No new tick path.** Music and TV pushes already run every sync pass; they gain the
projection input. The book owner gains its now-playing refresh as a dedicated sync-pass call
(separate from `set_content`, which stays event-driven), so chapter progress follows ticks without
rebuilding the whole book surface every tick.

**D6 — Pending selection mirrors the Queue.** The projection resolves the pending slot the same way
`queue_media_row_at` does: a pending selection lights the selected row and drops the outgoing
active row before the owner confirms.

## Risks / Trade-offs

- [Per-tick row rebuild while playing] → The dedup guards rebuild rows only when the projected rows
  actually changed (progress moves at tick granularity, like the Queue column today). When playback
  is idle or the workspace is hidden, the input is stable and nothing rebuilds.
- [Frame invalidation churn] → The existing `last_*` pattern treats an identical re-projection as a
  no-op, keeping the painted frame claimable by pointer input; the override rides the same guard.
- [Chapter boundary drift between shell and owner] → The shell derives the chapter from the same
  chapter list the owner renders; a stale chapter list after a catalog refresh is bounded by the
  next ABS event push, which re-projects both.

## Migration Plan

Single slice, no persistence or protocol changes. Land shell projection + owner seams + row
overrides together; the feature is inert until the projection input is wired into all three push
paths.
