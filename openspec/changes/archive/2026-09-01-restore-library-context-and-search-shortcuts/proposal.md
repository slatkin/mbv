## Why

Issue #636. `restore-library-ctrl-shortcuts` (issue #633) restored the `Ctrl+P/A/W/S/R`
and bare `r` library-panel shortcuts on the Emby **Music** and **TV** panels,
but it left two siblings of the same legacy handler still dead there:

- **`.` (context menu)** — works on the generic Emby browser
  (`BrowserComponent`) and Home, and on a *focused inline Music track*, but
  does nothing at the Music album level or anywhere on the TV panel.
- **`/` (search library)** — works only on `BrowserComponent`; the Music and
  TV components drop it at `_ => None`.

Legacy resolved both keys generically for every library kind in the one
shared handler `input_lib_keys.rs::handle_lib_key` (`.` at line 142, `/` at
line 292). The TuiRealm migration split that handler into per-destination
components and only `BrowserComponent` got a faithful port; `de45cdb8`
("remove global keyboard endpoint", ADR 0023) then deleted the fallback that
had been bouncing the unmatched keys through. The help overlay still
advertises `.` "Context menu" (Global) and `/` "Search library" (Library) on
every panel.

This is the same defect class as #633, scoped to the two chords that change
did not cover. Doing it now closes the Music/TV library-shortcut gap
completely before the interactive-surface ledger rows are finalized.

## What Changes

- `MusicWorkspaceComponent::handle_key` gains an **album-level** `.` arm
  (guarded on `track_cursor.is_none()`, so the existing focused-track
  `MusicTrackContextMenu` arm keeps precedence) and a `/` arm.
- `TvWorkspaceComponent::handle_key` gains `.` and `/` arms, resolving the
  context-menu target from `selected_item()` (series-list selection stays
  authoritative even while the Episodes pane is focused, matching the
  `EmbyLibrary*` arms #633 added).
- New `ShellRequest::EmbyLibraryContextMenu { item }` variant, mirroring
  `BrowserContextMenu { item }`; the shell routes it to
  `App::open_context_menu_for(item)` — the same tail `BrowserContextMenu`
  uses (`shell_browser.rs:59`).
- `/` reuses the existing `ShellRequest::OpenInlineSearch` that
  `BrowserComponent` emits; no new variant. The shell already scopes inline
  search to the selected Emby library.
- Dispatch arms for the new variant in `shell_messages.rs` (an unrouted
  variant is silent — see `make-shell-dispatch-exhaustive`).
- Component tests for both new arms on both components; model tests that the
  key drives the `App` tail.
- Ledger rows for Music and TV workspace updated; `help.rs` `.`/`/` gating
  re-verified.

## Capabilities

### Modified Capabilities

- `service-browse-dispatch`: ADD a requirement that the library-panel
  context-menu (`.`) and search-library (`/`) shortcuts SHALL be handled for
  every Emby library kind — generic/Movies/HomeVideos, Music, and TV —
  through the destination's own interactive component, not only the generic
  browser, with scenarios pinning the Music album-level and TV cases and the
  inline-track-focus precedence. Parallel to the requirement
  `restore-library-ctrl-shortcuts` adds for the `Ctrl+*` chords.

## Impact

- `src/app/components/music_workspace.rs`,
  `src/app/components/tv_workspace.rs` (new `handle_key` arms).
- `src/app/components/msg/shell.rs` (`EmbyLibraryContextMenu` variant).
- `src/app/shell_browser.rs` / `src/app/shell_library.rs` (route the variant
  to `open_context_menu_for`; watch the 800-line cap).
- `src/app/shell_messages.rs` (dispatch arm).
- `src/app/components/music_workspace_component_tests.rs`,
  `src/app/components/tv_workspace_component_tests.rs`.
- `docs/architecture/interactive-surface-ledger.md` (Music, TV rows).
- `src/app/render/components/help.rs` (verify `.`/`/` gating only).

### Sequencing

Depends on `restore-library-ctrl-shortcuts` being in the tree (it adds the
`EmbyLibrary*` variants, the `shell_library.rs` split decision, and the
`selected_item()` target-resolution pattern this change extends). No
spec-delta overlap: that change ADDs the `Ctrl+*` requirement, this one ADDs
the `.`/`/` requirement. If `make-shell-dispatch-exhaustive` lands first, add
`EmbyLibraryContextMenu` to its exhaustive match.

### Non-goals

- No change to `BrowserComponent` or Home — their `.` and `/` already work.
- No `.`/`/` on Audiobookshelf, Feeds, or the Queue panel — legacy never
  bound these keys there (the Queue context menu is right-click only, both
  before and after the migration).
- No new router policy; `key_policy.rs` `Active` dispatch already delivers
  both keys to the focused component.
- No reintroduction of `GlobalViewKey` / `handle_legacy_key` (ADR 0023).
