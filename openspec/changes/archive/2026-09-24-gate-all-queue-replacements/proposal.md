## Why

The populated-queue confirmation (#753) guards only grouped-music-tree track plays. Album and artist track plays, playlist loads, shuffle-folder plays, and context-menu Play/Shuffle all replace a populated queue silently (issue #754). Confirming before a user-picked replacement is a queue-level rule, so it should apply to every user-initiated replacement, not to one screen.

## What Changes

- Every user-initiated queue replacement goes through the one existing populated-queue gate before it runs: album/artist track plays, playlist loads, shuffle-folder plays, context-menu Play and Shuffle, and the grouped tree (already gated).
- The gate records which of the two existing playback executors runs once the user confirms. Neither executor changes.
- Deliberately not gated: library autoplay, single-item play (a movie or an episode continuation), and session-switch / session-event replays. These are either not user picks of a queue, or they restore existing state.
- Prompt order when more than one applies: replace-queue confirmation first, then the "play locally instead?" deferral, then the unsaved-playlist save/discard prompt.
- Out of scope: merging the two executors into one (option (b) in #754). It was rejected as not worth the cost for a confirmation fix.

## Capabilities

### New Capabilities

### Modified Capabilities
- `unified-playback-queue`: adds the rule that user-initiated queue replacements confirm before replacing a populated queue.

## Impact

- `src/app/queue_actions.rs` (`request_queue_replacement`), `src/app/app_struct.rs` (`pending_queue_replacement` type), and `src/app/input_confirm_keys.rs` (`ReplacePopulatedQueue` arm).
- Callers: `actions_navigation.rs`, `shell_playlists.rs`, `shuffle_folder_actions.rs`, `context_menu_actions.rs`.
- `docs/invariants/10-deferred-queue-mutation-slot-ownership.md`: the slot payload now carries its executor.
