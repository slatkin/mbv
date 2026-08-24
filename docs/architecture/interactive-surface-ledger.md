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
| Shell | Root UI and overlay routing | component (2026-08-24) | `UiRootComponent` (`src/app/components/root.rs`) and `shell_root.rs`; TuiRealm owns active focus and native LIFO restoration while the component owns overlay z-order | `App` state and legacy handlers remain pending group 5; root UI and overlay stack use `active`/`umount`; `rtk cargo nextest run -p mbv root_ui`; `rtk ast-grep scan`; `rtk make check-code-file-lines` |
| Root | Playback chrome and global controls | component (2026-08-24) | `PlaybackComponent` (`src/app/components/playback.rs`); shell mirrors reduced playback status and retains Player/transport authority | Component emits typed `Msg::Playback` intents and owns rendered transport geometry; per-key `playback` precedence remains on legacy `resolve_key` dispatch because it is irreducible; no PlayerProxy or transport state crosses the boundary; `rtk cargo nextest run -p mbv playback_chrome` (3 passed); teardown pending group 5 |
| Root | Queue | component (2026-08-24) | `QueueComponent` (`src/app/components/queue.rs`); shell mirrors `PlayerTab` queue snapshots and keeps legacy queue handlers/Player effects | Component owns cursor, scroll, scope, queue rendering, and slot-id hit geometry; canonical queue remains Player-owned; `rtk cargo nextest run -p mbv queue`; component and shell-routing tests; teardown pending group 5 |
| Root | Library parent | component (2026-08-24) | `LibraryComponent` (`src/app/components/library.rs`); shell mirrors effective destination/panel focus/mode and routes the active child to the mounted Home, Feeds, Emby, or Audiobookshelf component | Child components remain unchanged; legacy `App` state and handlers remain pending group 5; `rtk cargo nextest run -p mbv library_parent` (3 passed), component and shell-routing tests; `rtk ast-grep scan` reports the repository's 69 pre-existing screen-boundary diagnostics; `rtk make check-code-file-lines` |
| Library | Home | component (2026-08-24) | `HomeComponent` (`src/app/components/home.rs`); shell mirrors Home content and preserves legacy input forwarding while cursor/section/scroll remain component-local | Cross-Service display; App teardown pending group 5; `rtk cargo nextest run -p mbv home`; App-free render and shell-routing tests; `rtk ast-grep scan` |
| Library | Emby generic/Movies/home-video browser | component (2026-08-24) | `BrowserComponent` (`src/app/components/browser.rs`); shell mirrors the typed list context and preserves legacy input/action forwarding | Generic/Movies/home-video rows paint through `render_generic_movies_home_video_rows_with_ctx`; music, TV/series, and album-track branches remain legacy for 4.2/4.3/4.4; App teardown pending group 5; `rtk cargo nextest run -p mbv emby_browser`; component TestBackend and shell-routing tests; `rtk ast-grep scan` |
| Emby browser | Inline library Search | component (2026-08-24) | `InlineSearchComponent` (`src/app/components/inline_search.rs`); shell mirrors `LibSearch` and validated plain/recursive result pools while App retains search workers, album indexes, activation, and legacy input | Distinct from global Search sidebar; App teardown pending group 5; `rtk cargo nextest run -p mbv inline_library_search`; component and shell-routing tests; `rtk ast-grep scan` |
| Library | TV workspace | component (2026-08-24) | `TvWorkspaceComponent` (`src/app/components/tv_workspace.rs`); shell mirrors `TvWideRenderCtx` and preserves legacy effects/input forwarding | Two focusable panes with season/episode child targets; App teardown pending group 5; `rtk cargo nextest run -p mbv tv_workspace`; component TestBackend and shell sync; `rtk ast-grep scan` |
| Library | Grouped Music workspace | component (2026-08-24) | `MusicWorkspaceComponent` (`src/app/components/music_workspace.rs`); shell mirrors the grouped Music render context and cached tracks while legacy input/effects remain | Wide album/track workspace; App teardown pending group 5; `rtk cargo nextest run -p mbv music_workspace`; component TestBackend and shell-sync tests; `rtk ast-grep scan` |
| Library | Audiobookshelf podcast browser | component (2026-08-24) | `AudiobookshelfPodcastComponent` (`src/app/components/audiobookshelf_podcast.rs`); shell mirrors validated ABS content and preserves legacy effects/input forwarding | Show/episode workspace; component-owned selector/show/episode geometry; App teardown pending group 5; `rtk cargo nextest run -p mbv abs_podcast`; component, App-free render, shell-routing, and existing render tests; `rtk ast-grep scan` |
| Library | Audiobookshelf book browser | component (2026-08-24) | `AudiobookshelfBookComponent` (`src/app/components/audiobookshelf_book.rs`); shell mirrors validated ABS book content and preserves legacy effects/input forwarding | Browser/chapter workspace; component-owned replacement/chapter geometry; App teardown pending group 5; `rtk cargo nextest run -p mbv abs_book`; component TestBackend and shell-routing tests; `rtk ast-grep scan` |
| Library | Feeds | migrated (2026-08-24) | `FeedsComponent` (`src/app/components/feeds.rs`) with shell-owned refresh/result validation | Grouping, selector, list, inline hero, filtered playback/enqueue selection, unchanged-snapshot cursor preservation, and group-count coverage verified by `rtk cargo nextest run -p mbv feeds`; exhaustive Feeds mouse routing; `rtk ast-grep scan` retains only the repository's existing screen-boundary diagnostics |
| Root | Overlay stack | component (2026-08-24) | `UiRootComponent` (`src/app/components/root.rs`) owns overlay z-order; shell mirrors mounted presence while TuiRealm owns focus stack | Open uses `active`, dismiss uses `umount` with native focus restoration; legacy `App` options/flags and handlers remain pending group 5; `rtk cargo nextest run -p mbv root_ui`; `rtk ast-grep scan`; `rtk make check-code-file-lines` |
| Overlay stack | Global Search sidebar | component (2026-08-23) | `SearchSidebarComponent` (`src/app/components/search_sidebar.rs`); shell Model mounts/umounts on `App::search_sidebar_open` transitions, component owns sidebar state (query/cursor/scroll/type_filter/loading/results) + 300 ms debounce driven by `UserEvent::Clock`, renders via `application.view()`, emits `Msg::Shell(DismissSearch/SearchActivate)` and `Msg::Service(SearchQuery)` for cross-boundary work | Component-owned debounce; 25 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Settings sidebar and setup forms | component (2026-08-24) | `SettingsComponent` (`src/app/components/settings.rs`); shell mounts `OverlayId::Settings`, mirrors App display/service state, and routes setup effects as typed `Msg::Service` | Local settings cursor, service cursor, and setup drafts; App-free render seam; first-frame setup remains before Remote Service startup; teardown pending group 5 |
| Settings | Multiselect popup | component (2026-08-24) | `MultiselectComponent` (`src/app/components/multiselect.rs`); shell mounts `PopupId::Multiselect`, mirrors `App.multiselect_popup`, and preserves the legacy close/persistence handler | Local cursor/choices; App-free render seam; component and shell-routing tests; teardown pending group 5 |
| Settings | Library-routes popup | component (2026-08-24) | `LibraryRoutesComponent` (`src/app/components/library_routes.rs`); shell mounts `PopupId::LibraryRoutes`, mirrors `App.library_routes_popup`, and preserves legacy route actions | Two-stage local cursor; App-free render seam; component and shell-routing tests; teardown pending group 5 |
| Settings | Feed-management popup | component (2026-08-24) | `FeedsManageComponent` (`src/app/components/feeds_manage.rs`); shell mounts `PopupId::FeedManage`, mirrors `App.feeds_manage_popup`, and forwards typed keys to legacy handlers | Local list/form draft; App-free render seam; component and shell-routing tests; `App.feed_tab` reset remains in `feeds_manage_actions.rs`; teardown pending group 5 |
| Overlay stack | Sessions sidebar | component | `SessionsComponent` with shell-owned session/cast snapshots | OpenSpec `migrate-tui-to-tuirealm`, `rtk cargo nextest run -p mbv sessions`, component and render characterization tests, `rtk ast-grep scan`; teardown pending group 5 |
| Overlay stack | Playlists sidebar | component (2026-08-24) | `PlaylistsComponent` (`src/app/components/playlists.rs`); shell mirrors playlist snapshots and preserves legacy effects/input forwarding | Variable-row geometry is component-owned for painting/hit testing; duplicated `input_mouse_panels.rs` geometry remains until group 5; App teardown pending; `rtk cargo nextest run -p mbv playlists`; component geometry and existing render tests; `rtk ast-grep scan` |
| Playlists | Save-playlist dialog | component (2026-08-24) | `SavePlaylistComponent` (`src/app/components/save_playlist.rs`); shell mirrors `App.save_playlist_dialog` and preserves legacy input forwarding | Child of Playlists workflow; converted by task 4.8; App teardown pending group 5 |
| Overlay stack | Help sidebar | component (2026-08-23) | `HelpComponent` (`src/app/components/help.rs`); shell Model mounts/umounts, intercepts F1, renders via `application.view()` | Local scroll; destination-derived content; 27 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Context menu | component (2026-08-23) | `ContextMenuComponent` (`src/app/components/context_menu.rs`); shell Model mounts/umounts on `App::context_menu` transitions, forwards keys to existing `handle_key_context_menu`, `App::render_context_menu` does placement (writes `layout.context_menu_rect`), component paints via `application.view()` | Exclusive overlay with anchor geometry; 4 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Selection modal | component (2026-08-24) | `SelectionModalComponent` (`src/app/components/selection_modal.rs`); shell mirror and typed source-specific requests in `shell_overlays.rs`; legacy App state/actions retained pending group 5 | App-free render seam returns the painted modal, selector targets, and row targets; component TestBackend/local interaction tests, existing `selection_modal` characterization tests; `rtk ast-grep scan` still reports the repository's existing screen-boundary diagnostics, with no new Selection modal component diagnostic; teardown pending group 5 |
| Overlay stack | Confirm modal | component (2026-08-23) | `ConfirmComponent` (`src/app/components/confirm.rs`); shell Model mounts/umounts on `App::confirm_modal` transitions, forwards keys to existing `handle_key_confirm_modal`, renders via `application.view()` | Shared yes/no workflow; component forwards key, shell owns action dispatch; 7 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Daemon-lost modal | component (2026-08-23) | `DaemonLostComponent` (`src/app/components/daemon_lost.rs`); shell Model mounts/umounts on `App::daemon_lost_modal` transitions, forwards keys to existing `handle_key_daemon_lost_modal`, renders via `application.view()` | Process-lifecycle effects remain shell-owned; 6 component+render tests + ast-grep clean; teardown pending group 5 |
| Overlay stack | Remote-reanchor popup | component (2026-08-23) | `RemoteReanchorComponent` (`src/app/components/remote_reanchor.rs`); shell Model mounts/umounts on `App::remote_reanchor_popup` transitions, forwards keys to existing `handle_key_remote_reanchor`, renders via `application.view()` | Remote reconciliation stays shell-owned; 5 component+render tests + ast-grep clean; teardown pending group 5 |
| Root | Playback prompts (skip-intro/next-up) | component (2026-08-24) | `PlaybackPromptComponent` (`src/app/components/playback_prompt.rs`); shell mirrors prompt text/visibility and routes keys to the existing App handlers | Status-bar placement and desktop-notification gating preserved; `rtk cargo nextest run -p mbv playback_prompt` (3 passed), component TestBackend and shell-sync tests; `rtk ast-grep scan` reports only the pre-existing screen diagnostics; teardown pending group 5 |
| Library | Inline album-track interaction | component (2026-08-24) | `MusicWorkspaceComponent` (`src/app/components/music_workspace.rs`); shell mirrors `album_track_focus` and cached tracks while legacy input/effects remain | Child track cursor/scroll and album-vs-track focus coupling; App teardown pending group 5; `rtk cargo nextest run -p mbv album_track`; component local-state test and Music shell sync; `rtk ast-grep scan` |

## Verification Record

Each migrated row must link its OpenSpec change, narrow verification command, direct
component tests, shell-routing test, and final static check in Notes. Existing
render characterization remains required but does not by itself prove migration.
