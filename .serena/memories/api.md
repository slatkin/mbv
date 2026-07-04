# api.rs

`EmbyClient` — all HTTP calls to Emby. `MediaItem` — universal item type.

## Section comments in file (use for navigation)

- `// ── HTTP infrastructure ──`
- `// ── Authentication ──`
- `// ── Browse / fetch ──`
- `// ── Library actions ──`
- `// ── Playback reporting ──`
- `// ── Playlists ──`
- `// ── Series / episodes / chapters ──`
- `// ── Remote session control ──`

## fetch_items

`fetch_items(path, &[("Key", "value"), ...])` — for `Vec<MediaItem>` from paginated `{"Items": [...]}`.
Do NOT use for: top-level JSON arrays (`get_latest`, `get_ancestors`) or when `total_count` is needed (`get_items_sorted`).

## MediaItem parsing gotchas

- `production_year`: `ProductionYear` then `Year` (audio items use `Year`)
- `is_folder`: forced `true` for `MusicAlbum`, `MusicArtist`, `Series`, etc., regardless of Emby's `IsFolder`
- `total_count`: `ChildCount` (non-Series) or `RecursiveItemCount` (Series)

## Lang table sync requirement

`parse_audio_info` lang table **must stay in sync** with `lang_code_to_name()` in `player.rs`. Same ISO 639-1/2 → English name mapping. Nothing enforces at compile time — if you touch one, update the other.
