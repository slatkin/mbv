# Scoping — 5.3d *Mirrors and framework* (nested row)

Working notes for the final framework teardown. Deletion order is normative:
**surface mirrors → CONTEXT_STACK → LegacyInput**, each wave committed green.

## Recount (2026-08-xx)

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