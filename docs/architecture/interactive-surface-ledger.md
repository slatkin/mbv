# Interactive Surface Migration Ledger

This is the stable inventory for issue #603. It tracks interactive ownership,
not the completed render-system migration. The target architecture is accepted in
ADR 0022 and the migration policy in ADR 0023.

ADRs 0022 and 0023, this ledger, and the architecture map were reviewed on
2026-08-23. Every row is currently `legacy`; Search implementation still requires
the contracts listed in the architecture map to be decided in an OpenSpec change.

## States

- `legacy`: local state, input, update, rendering, or render-derived interaction
  data still depends on global `App` ownership.
- `migrated`: one component owns its approved local boundary; old `App` fields and
  handlers are removed rather than mirrored; static and behavior checks pass.

No other committed state is valid. Temporary adapters may exist inside a migration
change but do not justify marking a row migrated.

## Update Rules

- New independently interactive surfaces receive a row before implementation and
  must enter as `migrated`.
- Existing rows change to `migrated` only in the PR that completes and verifies the
  ownership move.
- Splitting or combining rows requires an update to the architecture map and a
  rationale in issue #603.
- An exception to the legacy no-new-debt rule requires maintainer approval recorded
  in issue #603 and linked from the row's Notes cell. Inline lint ignores are not an
  exception record.
- Rows are never deleted merely because a surface is hidden or temporarily unused.

## Ledger

| Parent | Interactive surface | Current state | Primary current ownership | Notes |
| --- | --- | --- | --- | --- |
| Shell | Root UI and overlay routing | legacy | `App`, `render/screens/root.rs`, `CONTEXT_STACK` | Owns Panel focus/mode, destination and overlay priority |
| Root | Playback chrome and global controls | legacy | `App`, `action.rs`, `input_mouse_dispatch.rs`, `render/components/chrome*` | Player authority remains outside UI |
| Root | Queue | legacy | `App`, `input_queue_keys.rs`, `render/screens/queue.rs` | Cursor/scroll/scope may move; canonical queue may not |
| Root | Library parent | legacy | `App`, `input_browse_dispatch.rs`, destination dispatcher | Parent for destination children |
| Library | Home | legacy | `App.home`, `home_actions.rs`, `render/components/home*` | Cross-Service display |
| Library | Emby generic/Movies/home-video browser | legacy | `App.libs`, library actions, shared list/hero renderers | Instances remain destination-qualified |
| Emby browser | Inline library Search | legacy | `LibSearch` in `LibraryTab`, `input_lib_keys.rs` | Distinct from global Search sidebar |
| Library | TV workspace | legacy | `LibraryTab` series state, TV actions/renderers | Two interactive panes and child targets |
| Library | Grouped Music workspace | legacy | album/music state and actions/renderers | Album/track focus coupling |
| Library | Audiobookshelf podcast browser | legacy | ABS browse state/actions/renderers | Show/episode workspace |
| Library | Audiobookshelf book browser | legacy | ABS book state/actions/renderers | Browser/chapter workspace |
| Library | Feeds | legacy | feed state/actions/renderers | Grouping, selector and inline hero |
| Root | Overlay stack | legacy | `App` options/flags and `CONTEXT_STACK` | Parent owns overlay presence and priority |
| Overlay stack | Global Search sidebar | legacy | `SearchSidebar` plus `App` input/debounce/worker/render paths | First proof; excludes inline Search |
| Overlay stack | Settings sidebar and setup forms | legacy | `App` settings/forms and settings input/render paths | Service effects remain shell-owned |
| Settings | Multiselect popup | legacy | `App.multiselect_popup` and modal handlers | Nested Settings child |
| Settings | Library-routes popup | legacy | `App.library_routes_popup` and modal handlers | Nested Settings child |
| Settings | Feed-management popup | legacy | `App.feeds_manage_popup` and handlers | Nested Settings child |
| Overlay stack | Sessions sidebar | legacy | `App` sessions/targets and handlers | Merged Emby/Cast targets |
| Overlay stack | Playlists sidebar | legacy | `App` playlist state and handlers | Variable-row geometry is duplicated in mouse path |
| Playlists | Save-playlist dialog | legacy | `App.save_playlist_dialog` and handlers | Child of Playlists workflow |
| Overlay stack | Help sidebar | legacy | `App.show_help/help_scroll` and handlers | Local scroll; destination-derived content |
| Overlay stack | Context menu | legacy | `App.context_menu`, top-priority input and renderer | Exclusive overlay with anchor geometry |
| Overlay stack | Selection modal | legacy | `App.selection_modal`, source actions/input/render | Explicit row and selector targets |
| Overlay stack | Confirm modal | legacy | `App.confirm_modal` and handlers | Shared yes/no workflow |
| Overlay stack | Daemon-lost modal | legacy | `App.daemon_lost_modal` and handlers | Process-lifecycle effects remain shell-owned |
| Overlay stack | Remote-reanchor popup | legacy | `App.remote_reanchor_popup` and handlers | Remote reconciliation stays shell-owned |
| Root | Playback prompts (skip-intro/next-up) | legacy | `App` prompt state, notification/input handlers | Player effects remain shell-owned |
| Library | Inline album-track interaction | legacy | `LibraryTab.album_track_focus`, resolver and album handlers | Child state machine, not global Search |

## Verification Record

Each migrated row must link its OpenSpec change, narrow verification command, direct
component tests, shell-routing test, and final static check in Notes. Existing
render characterization remains required but does not by itself prove migration.
