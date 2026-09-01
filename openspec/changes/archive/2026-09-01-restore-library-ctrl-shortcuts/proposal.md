# Restore library Ctrl+P/A/W/S/R shortcuts on the Music and TV panels

## Why

Issue #633. `Ctrl+P` (play), `Ctrl+A` (enqueue), `Ctrl+W` (toggle watched),
`Ctrl+S` (shuffle), and `Ctrl+R` (rescan) do nothing on the **Music** and
**TV** library panels. Only the Emby generic/Movies/HomeVideos browser
(`BrowserComponent`) carries these chords after the TuiRealm migration.

Legacy resolved all five chords for every library kind in one shared handler,
`input_lib_keys.rs::handle_lib_key`. During the migration:

1. `d5682ee6` "fix(tui): forward media global keys through shell" made
   `MusicWorkspaceComponent` / `TvWorkspaceComponent` wrap unmatched keys as
   `ShellRequest::GlobalViewKey(KeyEvent)` and bounce them through
   `handle_legacy_key` → the legacy lib-key handler. The chords worked.
2. `de45cdb8` "remove global keyboard endpoint" (next day) deleted
   `App::handle_key`, `GlobalViewKey`, `handle_legacy_key`, and the
   per-context handlers per ADR 0023, and restored the `_ => None` fallback
   in the two components. The `Ctrl+*` actions were never ported into them —
   only `BrowserComponent` (`src/app/components/browser.rs:373-400`) got real
   handlers.

The Music album list still paints the hint
`^P: Play | ^A: Enqueue | ^S: Shuffle` (`album_rows.rs:310`) for actions it
silently ignores.

The migration for these is small because the hard parts already exist:

- Both components already expose `selected_item() -> Option<EmbyItem>`
  (`music_workspace.rs:142`, `tv_workspace.rs:141`) — the same
  component-local target resolution `BrowserComponent` uses.
- The `App` effect tails are Service-agnostic and take `(lib_idx, EmbyItem)`:
  `play_or_activate_lib_item`, `enqueue_lib_item`, `toggle_watched_item`,
  `shuffle_play_selected`, `refresh_lib`, and the `RescanLibrary` confirm
  modal. `handle_browser_request` (`shell_browser.rs:20-90`) already routes
  the video browser's requests to exactly these.
- `view_dispatch` already delivers unclaimed keys to the active component;
  no router change is needed.

## What Changes

- `MusicWorkspaceComponent::handle_key` and
  `TvWorkspaceComponent::handle_key` match `Ctrl+P`, `Ctrl+A`, `Ctrl+W`,
  `Ctrl+S` at the album/series level (i.e. when no inline track focus /
  Episodes pane owns the key), resolving the target from `selected_item()`,
  and match `Ctrl+R` / bare `r` for refresh and rescan. Each returns a typed
  `Msg::Shell(ShellRequest::…)`; no `to_crossterm_key_event`, no
  `GlobalViewKey`.
- New `ShellRequest` variants mirroring the `Browser*` set for these
  surfaces: `LibraryPlay`, `LibraryEnqueue`, `LibraryToggleWatched`,
  `LibraryShuffle`, `LibraryRefresh`, `LibraryRescan`, each carrying the
  component-resolved `EmbyItem` where the `Browser*` equivalent does. (Name
  `Library*` rather than `Music*`/`Tv*` because both surfaces route to the
  same Service-agnostic `App` tails — one handler serves both.)
- A shell handler (`handle_library_request` or an extension of
  `handle_browser_request`) routes the new variants to the existing `App`
  tails, deriving `lib_idx` from `self.app.tab.emby_library_index()` exactly
  as the browser handler does.
- The Music `Ctrl+P` / `Ctrl+A` arms that today only fire while an inline
  track is focused keep that behavior; the new album-level arms sit behind
  the `track_cursor.is_none()` guard, matching the legacy precedence where a
  track-level context took the key first.

## Non-goals

- No change to `BrowserComponent` — its five chords already work and route
  through `handle_browser_request`.
- No change to the Audiobookshelf book/podcast components (they own
  `Ctrl+A` locally already; their play/shuffle semantics are covered by
  their own specs).
- No new router policy layer, no change to `key_policy.rs` or the
  `view_dispatch` entry.
- No reintroduction of `GlobalViewKey`, `handle_legacy_key`, or any
  legacy-endpoint bridge (ADR 0023).
- The `Ctrl+PageUp`/`Ctrl+PageDown` artist-jump chords are out of scope —
  they are already covered by `artist-keyboard-navigation` and its own
  migration state.

## Capabilities

### Modified Capabilities

- `service-browse-dispatch`: the left-panel keyboard-dispatch requirement
  gains an explicit statement that the library playback/queue/library-admin
  shortcuts (`Ctrl+P/A/W/S/R`) SHALL be handled for every Emby library kind —
  video, Music, and TV — through the destination's own interactive component,
  not only the generic browser, and a scenario pinning the Music and TV cases.

## Impact

- `src/app/components/music_workspace.rs`,
  `src/app/components/tv_workspace.rs` (new `handle_key` arms).
- `src/app/components/msg/shell.rs` (new `Library*` `ShellRequest` variants).
- `src/app/shell_browser.rs` or a new `src/app/shell_library.rs` (route the
  new variants; watch the 800-line cap on `shell_browser.rs`).
- `src/app/shell_messages.rs` (dispatch arms for the new variants — see
  `make-shell-dispatch-exhaustive` for why an unrouted variant is silent).
- `src/app/components/music_workspace_component_tests.rs`,
  `src/app/components/tv_workspace_component_tests.rs`.
- `docs/architecture/interactive-surface-ledger.md`: the Music and TV
  workspace rows.
- Help overlay (`render/components/help.rs`) already lists these chords —
  no change, but verify the "Emby-only" gating still reads correctly once
  they work on Music/TV.

## Sequencing

Independent of the other open changes. If `make-shell-dispatch-exhaustive`
lands first, the new `Library*` variants must be added to its exhaustive
match; if this lands first, that change picks them up. No spec-delta overlap.
