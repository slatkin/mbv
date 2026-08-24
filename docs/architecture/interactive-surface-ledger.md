# Interactive Surface Migration Ledger

This is the stable inventory for issue #603. It tracks interactive ownership,
not the completed render-system migration. The TuiRealm target and complete-
conversion policy are accepted in ADR 0022.

The architecture map and ledger were reviewed on 2026-08-23. The earlier bespoke
component and incremental-migration ADRs were replaced when the project was
reframed as migration of the existing TUI framework to TuiRealm. Rows move from
`legacy` to `component` during the mirror-first implementation stage, then to
`migrated` after teardown.

## States

- `legacy`: local state, input, update, rendering, or render-derived interaction
  data still depends on global `App` ownership.
- `component`: the component has landed and paints the surface; the shell still
  mirrors `App` state and/or legacy input still forwards; `App` teardown is
  pending group 5.
- `migrated`: one component owns its approved local boundary; old `App` fields and
  handlers are removed rather than mirrored; static and behavior checks pass.

These are the only committed states. Temporary adapters may exist inside the
complete-conversion change, but a mixed TuiRealm/legacy framework is not a completed
or mergeable endpoint even when individual rows are marked `component`.

## Update Rules

- New independently interactive surfaces receive a row before implementation and
  enter as `legacy`, then change to `component` when their component lands.
- Existing rows change to `component` when their component lands and to
  `migrated` only in the PR that completes and verifies the ownership move.
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
| Library | Feeds | component (2026-08-24) | `FeedsComponent` (`src/app/components/feeds.rs`); shell mirror in `shell_feeds.rs`; legacy feed actions/input remain in place | Grouping, selector, list, and inline hero paint through the App-free Feeds render seam; `rtk cargo nextest run -p mbv feeds` (35 passed), component TestBackend and shell-sync tests; `rtk ast-grep scan` still reports the repository's existing 71 screen-boundary diagnostics, with no Feeds diagnostic; teardown pending group 5 |
| Root | Overlay stack | legacy | `App` options/flags and `CONTEXT_STACK` | Parent owns overlay presence and priority |
| Overlay stack | Global Search sidebar | component (2026-08-23) | `SearchSidebarComponent` (`src/app/components/search_sidebar.rs`); shell Model mounts/umounts on `App::search_sidebar_open` transitions, component owns sidebar state (query/cursor/scroll/type_filter/loading/results) + 300 ms debounce driven by `UserEvent::Clock`, renders via `application.view()`, emits `Msg::Shell(DismissSearch/SearchActivate)` and `Msg::Service(SearchQuery)` for cross-boundary work | Component-owned debounce; 25 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Settings sidebar and setup forms | legacy | `App` settings/forms and settings input/render paths | Service effects remain shell-owned |
| Settings | Multiselect popup | component (2026-08-24) | `MultiselectComponent` (`src/app/components/multiselect.rs`); shell mounts `PopupId::Multiselect`, mirrors `App.multiselect_popup`, and preserves the legacy close/persistence handler | Local cursor/choices; App-free render seam; component and shell-routing tests; teardown pending group 5 |
| Settings | Library-routes popup | component (2026-08-24) | `LibraryRoutesComponent` (`src/app/components/library_routes.rs`); shell mounts `PopupId::LibraryRoutes`, mirrors `App.library_routes_popup`, and preserves legacy route actions | Two-stage local cursor; App-free render seam; component and shell-routing tests; teardown pending group 5 |
| Settings | Feed-management popup | component (2026-08-24) | `FeedsManageComponent` (`src/app/components/feeds_manage.rs`); shell mounts `PopupId::FeedManage`, mirrors `App.feeds_manage_popup`, and forwards typed keys to legacy handlers | Local list/form draft; App-free render seam; component and shell-routing tests; `App.feed_tab` reset remains in `feeds_manage_actions.rs`; teardown pending group 5 |
| Overlay stack | Sessions sidebar | component | `SessionsComponent` with shell-owned session/cast snapshots | OpenSpec `migrate-tui-to-tuirealm`, `rtk cargo nextest run -p mbv sessions`, component and render characterization tests, `rtk ast-grep scan`; teardown pending group 5 |
| Overlay stack | Playlists sidebar | legacy | `App` playlist state and handlers | Variable-row geometry is duplicated in mouse path |
| Playlists | Save-playlist dialog | legacy | `App.save_playlist_dialog` and handlers | Child of Playlists workflow |
| Overlay stack | Help sidebar | component (2026-08-23) | `HelpComponent` (`src/app/components/help.rs`); shell Model mounts/umounts, intercepts F1, renders via `application.view()` | Local scroll; destination-derived content; 27 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Context menu | component (2026-08-23) | `ContextMenuComponent` (`src/app/components/context_menu.rs`); shell Model mounts/umounts on `App::context_menu` transitions, forwards keys to existing `handle_key_context_menu`, `App::render_context_menu` does placement (writes `layout.context_menu_rect`), component paints via `application.view()` | Exclusive overlay with anchor geometry; 4 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Selection modal | component (2026-08-24) | `SelectionModalComponent` (`src/app/components/selection_modal.rs`); shell mirror and typed source-specific requests in `shell_overlays.rs`; legacy App state/actions retained pending group 5 | App-free render seam returns the painted modal, selector targets, and row targets; component TestBackend/local interaction tests, existing `selection_modal` characterization tests; `rtk ast-grep scan` still reports the repository's existing screen-boundary diagnostics, with no new Selection modal component diagnostic; teardown pending group 5 |
| Overlay stack | Confirm modal | component (2026-08-23) | `ConfirmComponent` (`src/app/components/confirm.rs`); shell Model mounts/umounts on `App::confirm_modal` transitions, forwards keys to existing `handle_key_confirm_modal`, renders via `application.view()` | Shared yes/no workflow; component forwards key, shell owns action dispatch; 7 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Daemon-lost modal | component (2026-08-23) | `DaemonLostComponent` (`src/app/components/daemon_lost.rs`); shell Model mounts/umounts on `App::daemon_lost_modal` transitions, forwards keys to existing `handle_key_daemon_lost_modal`, renders via `application.view()` | Process-lifecycle effects remain shell-owned; 6 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Remote-reanchor popup | component (2026-08-23) | `RemoteReanchorComponent` (`src/app/components/remote_reanchor.rs`); shell Model mounts/umounts on `App::remote_reanchor_popup` transitions, forwards keys to existing `handle_key_remote_reanchor`, renders via `application.view()` | Remote reconciliation stays shell-owned; 5 component+render tests + ast-grep clean; teardown pending group 5 |
| Root | Playback prompts (skip-intro/next-up) | component (2026-08-24) | `PlaybackPromptComponent` (`src/app/components/playback_prompt.rs`); shell mirrors prompt text/visibility and routes keys to the existing App handlers | Status-bar placement and desktop-notification gating preserved; `rtk cargo nextest run -p mbv playback_prompt` (3 passed), component TestBackend and shell-sync tests; `rtk ast-grep scan` reports only the pre-existing screen diagnostics; teardown pending group 5 |
| Library | Inline album-track interaction | legacy | `LibraryTab.album_track_focus`, resolver and album handlers | Child state machine, not global Search |

## Verification Record

Each migrated row must link its OpenSpec change, narrow verification command, direct
component tests, shell-routing test, and final static check in Notes. Existing
render characterization remains required but does not by itself prove migration.
