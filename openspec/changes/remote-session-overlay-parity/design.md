# Design

## Context

Local playback resolves the active queue slot to a full `EmbyItem`, so the title overlay gets a
two-part title (`PlaybackTitleParts::two`) and a logo owner (`overlay_logo_source` in
`src/app/state/projection/card.rs`: Movie → `{id}:Logo:{etag}`, Episode → `{series_id}:Logo`).
A watched remote Emby session's now-playing item is deliberately held without an item record
(`project_slotless_session`): the session parser (`crates/mbv-emby/src/client_sessions.rs`)
extracts only `Name`, `Id`, `Type`, and `SeriesId` from `NowPlayingItem`, the slotless title
parts are one-part (`slotless_playback_title_parts` in `src/app/shell/playback.rs`), and
`queue_title_site` resolves a logo only from a `QueueItem::Emby`, so the slotless path never
issues a logo fetch. The overlay composer and painter (`ensure_title_overlay_protocol`,
`mbv_images::title_overlay`) are already item-kind agnostic — they take parts plus an optional
logo cache key.

The remote payload facts the design needs were not probed on the live server (user opted for a
defensive design), so every new read degrades to today's behavior when absent.

## Goals / Non-Goals

**Goals:**
- Remote-session episodes draw the same overlay as local playback: show logo upper-left, show
  name as context, episode title in the bottom row.
- Remote-session movies draw their own logo alone when the session payload advertises one.
- The playback panels' title presentation gains the same parity for free (they consume the same
  parts).

**Non-Goals:**
- No item-record fetch for session now-playing items (keeps the "held without an item record"
  design and its single-round-trip cost).
- No Cast-side changes (a Cast now-playing name carries no series context).
- No changes to the overlay composer, painter, cache keys, or site-decision gates.

## Decisions

- **D1 — Carry the missing facts on `SessionInfo`.** Add `now_playing_series_name:
  Option<String>` and `now_playing_logo_etag: Option<String>` to `SessionInfo`, parsed in
  `client_sessions.rs` from `NowPlayingItem.SeriesName` and `NowPlayingItem.ImageTags.Logo`.
  Only one production constructor site exists; absent JSON keys parse to `None` and every
  consumer treats `None` as today's behavior. Alternative considered: fetch the full item by id
  — rejected (extra round trip per now-playing change, contradicts the held-without-record
  design).

- **D2 — Two-part slotless title from the session payload.** `slotless_playback_title_parts`
  builds `PlaybackTitleParts::two(episode_name, series_name)` when the watched Emby session's
  now-playing item type is `Episode` and the series name is present and non-empty; otherwise the
  one-part title from the session name, as today. The Cast branch stays one-part. This reuses
  the exact parts type and split rule the local path uses (`crates/mbv-queue/src/items.rs`).

- **D3 — Logo owner derived from session fields, shared helper.** Generalize
  `overlay_logo_source` into a field-based helper
  (`item_type, item_id, series_id, logo_etag` → `Option<OverlayLogoSource>`) that both the
  `EmbyItem` path and the slotless path call. The slotless path supplies
  `now_playing_item_type`, `now_playing_item_id`, `now_playing_series_id`, and
  `now_playing_logo_etag`; an Episode needs a non-empty series id, a Movie needs a non-empty
  logo etag (the `{id}:Logo:{etag}` cache key needs the etag to bust the cache). `queue_title_site`
  then issues the same `["Logo"]` fetch and passes the same `logo_cache_key` it does for local
  items. Alternative considered: a separate slotless resolver — rejected (duplicates the key
  rules and lets them drift).

- **D4 — No new gates.** The existing title-site gates and the `title_covers` glyph check apply
  unchanged; a series name is ordinary text. `item_kind` stays `"Remote"` for slotless decisions
  (log identity only).

## Risks / Trade-offs

- [An Emby server may omit `SeriesName` or `ImageTags` from session payloads] → Every new read
  is optional and degrades to today's one-part, logo-free overlay; the spec scenarios pin both
  branches.
- [A second Logo fetch for the slotless card (the card chain already requests `Logo` under the
  card key)] → This duplication already exists on the local path; parity is exact and the image
  cache deduplicates by key.
- [`SessionInfo` grows by two fields across a hot session-refresh path] → Two `Option<String>`
  moves per refresh; negligible, and the struct is already cloned per refresh.

## Migration Plan

Additive fields and additive projection branches; no data migration. Rollback is a revert.

## Open Questions

(none)
