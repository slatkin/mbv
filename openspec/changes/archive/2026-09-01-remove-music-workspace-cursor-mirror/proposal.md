## Why

Issue #611's survey recorded the Music workspace as already moved to
event-scoped projection with no pin. It still carries one, and the component
announces it in its own field list:

```rust
pub struct MusicWorkspaceComponent {
    album_cursor: usize,
    album_scroll: usize,
    ...
    last_mirrored_cursor: usize,   // src/app/components/music_workspace.rs:28
    last_mirrored_scroll: usize,   //                                     :29
}
```

`last_mirrored_cursor` / `last_mirrored_scroll` hold the last value the shell
pushed, so `set_content` can ask "is my current cursor still the one I was
given?" and adopt the incoming value only if the answer is yes
(`music_workspace.rs:95-100`). That is echo suppression: a field whose only
purpose is telling the component's own writes apart from the shell's. A
surface with a single owner has nothing to compare against, so the pair is a
precise fingerprint of a live two-way mirror.

The consequence is order-dependent. Adoption is gated on an equality test, not
on an event, so whether a shell-side cursor change reaches the component
depends on whether the user happened to move first. Two identical `App` states
can project differently.

Unlike the Audiobookshelf surfaces, the *request* side here is already correct:
`MusicAlbumCursor` carries a resolved `target` and
`move_music_group_display_cursor` / `jump_music_group_display_cursor` /
`page_grouped_album_cursor` (`src/app/render/screens/album_cursor.rs`) already
apply it directly without recomputing. Only the return path is wrong.

## What Changes

- `MusicWorkspaceComponent` owns `album_cursor`, `album_scroll`, and
  `track_cursor` outright. `set_content` stops reading `context.list.cursor()`,
  `context.list.scroll()`, and `context.track_cursor` on every push.
- `last_mirrored_cursor` and `last_mirrored_scroll` are deleted. Their absence
  is the change's acceptance test.
- The genuine shell-owned re-anchors keep working, as explicit events rather
  than as a side effect of every push: group switch, recursive-album
  activation, and saved-position restore each re-anchor the component's cursor
  once, at the event (`music_grouping.rs:296-309` writes
  `level.cursor = catalog.entries[pos].album_index` at exactly these points).
- The existing selected-album-identity reset of `track_cursor`
  (`last_album_id`) is retained; it is an event-driven reset, not an echo test.

## Non-goals

- `BrowseLevel.cursor` and `.scroll` stay. The component→shell write-through
  and the ~37 `App`-side readers are `split-browse-state-interaction-fields`'s
  problem, exactly as `remove-browser-cursor-scroll-mirror` scoped it.
- No change to `MusicAlbumCursor`'s request shape, the three
  `album_cursor.rs` entry points, or their guards and effect tails.
- No change to group-pill cycling, album activation, or track playback.

## Capabilities

### Modified Capabilities
- `interactive-component-framework`: the presentation-authority requirement
  gains an explicit prohibition on echo-detection state — a component field
  that exists to distinguish its own writes from the shell's — and a scenario
  for re-anchoring at a navigation event rather than by equality test.

## Impact

- `src/app/components/music_workspace.rs`,
  `src/app/shell_music_workspace.rs`, `src/app/render/components/music_wide.rs`
  (if `MusicWideRenderCtx` sheds the three now-unread fields),
  `src/app/components/music_workspace_component_tests.rs`.
- Eleven `push_music_workspace_content()` call sites
  (`shell_messages.rs`, `shell_run.rs`, `shell_music_workspace.rs`) keep
  working unchanged; three of them may need an explicit re-anchor call.
- `docs/architecture/interactive-surface-ledger.md`: the Music workspace row.

## Sequencing

This change's spec delta includes the paragraphs added by
`split-audiobookshelf-cursor-ownership`, so apply it after that change. If it
lands first instead, drop the two Audiobookshelf-specific paragraphs from the
delta before syncing.
