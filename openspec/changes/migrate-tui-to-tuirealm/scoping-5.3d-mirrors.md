# Scoping — 5.3d *Mirrors and framework* (nested row)

Working notes for the final framework teardown. Deletion order is normative:
**surface mirrors → CONTEXT_STACK → LegacyInput**, each wave committed green.

## Recount (2026-08-25)

Production sync/framework definitions as of `7de7d409` — 19 `Model` methods plus
`UiRootComponent::sync_overlay_order` = 20 candidates. Classified by ownership:

**Keep (domain sync, not surface mirrors):**
- `sync_volume_from_player`, `sync_visualizer` (player state)
- `sync_subtitle_prefs_to_player` / `sync_subtitle_prefs_from_emby` (subtitle prefs)
- `sync_feed_subscriptions` (feed fetch/refresh)
- `sync_playback_queue_after_append` / `sync_playback_queue_items_after_append` (queue append)
- `PlayerTab::sync_active_slot` (player-tab slot projection)
- test-local helpers (`sync_layout_to_app` x2, `sync_series_refresh`, …)
- `sync_playback` (player/transport projection into `PlaybackComponent`)

**Delete (surface mirrors / temporary adapters):** all others, in waves:

- Wave 1 (framework scaffolding): `App::blocking_overlay_active` field,
  `sync_precedence_gates`, `sync_multiselect`, `sync_library_routes`.
- Wave 2 (overlay z-order): `sync_overlay_stack` + `UiRootComponent::sync_overlay_order`
  (canonical `OVERLAY_IDS` order replaces the retained mount order).
- Wave 3+: per-surface mirrors `sync_home`, `sync_feeds`, `sync_queue`,
  `sync_emby_browser`, `sync_tv_workspace`, `sync_music_workspace`,
  `sync_inline_search`, `sync_library_parent`, `sync_audiobookshelf_podcast`,
  `sync_audiobookshelf_book`, `sync_playback_prompt`, `sync_modal_requests`,
  `sync_feeds_manage`, plus content bridges `update_settings_content`,
  `update_playlists_content`.

Each mirror deletion is a move, not a copy: the App field it mirrored is deleted
(or re-homed to shell/component ownership) with its `impl App` interaction
handlers, legacy branches and render reads, and the component fallbacks that
still emit `Msg::Legacy(...)` / `ShellRequest::*Key` / `NoOp`.

## Absence proofs required at the end

- `LegacyInput`, `LegacyTerminalEvent`, `Msg::Legacy`
- TuiRealm→crossterm reconstruction helpers (`to_crossterm_*`)
- `CONTEXT_STACK`, `ContextEntry`, `App::handle_key`, stack-owned
  `handle_key_*` endpoints
- no surface mirror / temporary adapter (incl. `ShellRequest::*Key` fallbacks)
- no remaining `impl App` interaction ownership

## Landed so far (2026-08-25, commits from clean 7de7d409)

- **Wave 1** `4ce46d0a` — delete `App::blocking_overlay_active` (temporary
  precedence adapter) + no-op mirrors `sync_multiselect`/`sync_library_routes`
  + `sync_precedence_gates` (attr writes; `ATTR_*` constants + KEY_POLICY stay
  for the 5.4 static proofs). Readers re-homed: dim/stay-alive flag computed
  by the shell in the draw closure; `any_other_modal_open` removed (a mounted
  blocking modal is always the active component and swallows input);
  playlists dismiss gates use `pending_overlay.is_none()`.
- **Wave 2** `4152a9e5` — overlay z-order mirror: `sync_overlay_stack` +
  `UiRootComponent::sync_overlay_order`/`overlay_order()` deleted;
  `render_overlay_stack` iterates canonical `OVERLAY_IDS` × mount state.
- **Wave 3** `b9d1abef` (+fmt `893ba2bf`) — feeds-management popup two-way
  mirror (`sync_feeds_manage`/`sync_feeds_manage_to_app`) deleted;
  `FeedsManageComponent` owns the draft and gets targeted `set_stage`/
  `set_feeds`/`set_pending_add` pushes; `Model::feeds_manage` shrinks to the
  add-channel + pending marker. Tests rewritten at the component/shell boundary.
- **Wave 4** `a46d635b` — library-parent routing mirror: `sync_library_parent`
  replaced by direct `sync_active_destination` (idempotent `active()`);
  `LibraryComponent` (inert after the mirror) deleted with its mount and the
  mirror-pinning tests; new Model-boundary routing tests.
- **Wave 5** `f35ed7f6` — first surface-mirror deletion: `sync_inline_search`
  replaced by `push_inline_search_content()` at the search's actual input
  change points (open, flat-result apply, album-index `Ready` completion and
  async nav completions via the `lib_rx` drain tail, activation, resize), with
  stale-mount release when an async completion moves the tab; placement flows
  through the render seam (component paints the `view`-time rect, `self.area`
  removed). Ledger row moved to `migrated`.
- **Prep (sync_home mirror deletion)** `7c1168a2` — deletes the per-frame
  `sync_home` mirror (phase 1 of the `sync_home` move; the App-field re-home
  to shell/component ownership is the follow-up unit, scheduled separately —
  no `App.home.*` deletion here). `HomeComponent` is now driven by
  `push_home_content()` at the writers of its projected inputs: the shell's
  startup `fetch_home` (with `home_loading` around it), the Emby
  startup/setup drains (`apply_emby_bootstrap`), the player/lib/feed/ws
  drains (`refresh_after_stop`, `AudiobookshelfShelfFetched` →
  `rebuild_audiobookshelf_latest`, `rebuild_feeds_latest`,
  `UserDataChanged`), the legacy key seam (panel-focus + key-driven
  `fetch_home`), the context-menu/confirm/multiselect seams, the
  `HomeDelete`/`HomeToggleWatched` refetch arms and `Row`/`ContextMenu` Home
  clicks (panel focus), and the queue-click seam. `set_content` clamps the
  section synchronously so the persisted-pill restore and the semantic-pref
  reconcile preserve their exact ordering (pending restored → cleared →
  reconcile records the restored source; kept pending while absent so an
  unrelated `save_prefs` never overwrites it); `[`/`]`/pill selection already
  reaches the shell via the typed `HomeSectionSelected`/`HomeClick::Pill`
  notifications, so no new section-changed notification was needed. The CW
  wheel quirk (`cw_move_cursor`) is untouched. Tests adapt `sync_home()`
  calls to drive `push_home_content()`. Ledger Home row stays `component`
  (no ledger edit).
- **Prep (sync_home typed-effects)** `d2b24d0c` — bounded routing seam ahead
  of the `sync_home` deletion (not the ownership move itself):
  `HomeComponent` already emits `HomePlay`/`HomeEnqueue`/`HomeDelete`/
  `HomeToggleWatched`/`HomeSectionSelected`, but the shell had no consumers
  (they fell into the catch-all `_` arm). Shell now routes them via
  `Model::handle_home_request` to the legacy effects, and the effects
  (`home_play`/`home_enqueue`/`home_current_item` + new `home_delete`) take
  the component-provided flat cursor instead of reading
  `App::home.home_cursor` — the requested target is honored even when the
  App cursor differs. Legacy `handle_cw_key`/double-click pass their resolved
  cursor explicitly; CW resume, enqueue, delete, watched-toggle, and
  section-preference behavior unchanged. `sync_home`, the Home
  cursor/section/scroll fields, the legacy Home renderer, `handle_key_home`,
  `CONTEXT_STACK`, and `LegacyInput` all remain. One focused Model-boundary
  test drives each typed effect.
- **5.3d re-home** `0d93e43f` — full `App.home`/`App.home_loading` deletion;
  Home content (continue-items, per-pill latest, CW column cursor) + the
  loading flag move to new Model-owned `HomeContent`
  (`Model.home_content`, `types_playback.rs`). Delivery mechanism: the fetch
  is never deferred — `fetch_home` computes and returns `HomeContent`
  synchronously (its App-side side effects are order-sensitive), shell-side
  callers (startup `fetch_home_at_startup`, multiselect commit) assign the
  return directly; App-internal writers deliver through lib_tx/lib_rx via
  `LibEvent::HomeContentRefreshed(Box<HomeContent>)` (context-menu
  refresh/remove/toggle, `refresh_after_stop`, `UserDataChanged`),
  `HomeContentCleared` (`clear_emby_memory`), and the section-delta variants
  `AudiobookshelfLatestRebuilt`/`FeedsLatestRebuilt` (shelf-fetch / feed
  drain; the ABS delta is drained in the same lib_rx pass, the Feeds delta
  one loop pass later — a bounded one-iteration latency on the Feeds pill,
  the known residual). `apply_emby_completion`/`apply_emby_setup_completion`
  now return `Option<HomeContent>` (bootstrap merges Emby pills into the
  shell-supplied current `latest`; `None` on stale/error/replace-decline).
  `push_home_content` + all 11 phase-1 seams read `home_content`; the
  persisted-pill restore and semantic-pref reconcile are untouched
  (`home_section_*` stay in App; `save_prefs` untouched). Effects take
  resolved items at the boundary: `home_play_target`/`home_enqueue_target`
  (shell resolves the flat cursor via `Model::home_flat_target`),
  `cw_play`/`cw_enqueue`/`cw_toggle_watched`/`remove_from_continue_watching`
  take the CW item (`Model::home_cw_item`), and the CW item is threaded
  through the keyboard `CONTEXT_STACK` (`handle_key_with_home_context` +
  every handler) and `build_context_menu_for`/`open_context_menu*` for the
  `.`-on-Home and queue-coupling menus; `cw_move_cursor` is now a Model
  method over `home_content` with the identical clamp. Render legacy branch
  deleted (`render_list` is only reached under EmbyLibrary).
  `Model`-side Home content helpers live in new `shell_home_content.rs`
  (assign/merge/clear/resolve/drain methods keep `shell.rs`/`shell_home.rs`
  net-delta ≤ 0). Tests adapt to the returned-content/Model-hosted APIs.
  Ledger Home row stays `component` (2a owns no ledger edit).
- **Wave 6** `4f98154` — Home ownership completion: moved the semantic section
  preference and one-time restore marker from `App` to `Model`, made
  `save_prefs` preserve the shell-owned `home_section` key, and persisted only
  on actual shell-side preference changes. Home is now fully migrated; the
  ledger row records the focused Home/Model boundary and full `mbv` nextest
  verification.

**Not yet landed** (remain, in required order): the per-surface interaction
mirrors whose `App` state still holds the cursor (`sync_emby_browser`,
`sync_tv_workspace`, `sync_music_workspace`,
`sync_audiobookshelf_podcast`, `sync_audiobookshelf_book`)
— each needs the full 5.3a-style ownership move (component keys + typed Msg +
App-field deletion + legacy render branch); then `CONTEXT_STACK`/`handle_key_*`
endpoints; then `LegacyInput`/`Msg::Legacy`/`LegacyTerminalEvent` + every
component fallback. Classified **keep** (domain/player projections, per the
instruction's own keep-list): `sync_playback`, `sync_playback_prompt`,
`sync_queue`, `sync_feeds`, `sync_modal_requests`, and the settings/playlists
content bridges.

The nested *Mirrors and framework* row stays **unchecked** (unit incomplete).