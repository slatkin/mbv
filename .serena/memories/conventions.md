# Conventions

## Code style

- No emoji — plain Unicode box-drawing / ASCII only in UI strings.
- No `#[allow(...)]` suppressions — fix warnings by deleting dead code.
- `pub(super)` for methods called by sibling modules; `pub(crate)` only when needed outside `app/`.
- Files stay small and modular; no monolithic files.

## `App` struct

- All `impl App` blocks split across `src/app/` files.
- New fields: add to struct, set default in `build()`, add to `AppInit` only if constructors need different values.
- `image_protocol_enabled` is a cached bool — read via `images_enabled()`, not config directly.

## API / browse methods

- Browse methods returning `Vec<MediaItem>` from paginated `{"Items":[...]}` use `fetch_items()`.
- Methods returning a top-level JSON array or needing `total_count` do **not** use `fetch_items` — intentional.
- `MediaItem.is_folder` is forced `true` for `MusicAlbum`, `MusicArtist`, `Series`, etc. regardless of Emby's field.
- `production_year` parsed from `ProductionYear` then `Year` (Emby uses `Year` for audio items).
- `total_count` from `ChildCount` (non-Series) or `RecursiveItemCount` (Series).

## Sync invariant

`parse_audio_info` language table (`api.rs`) ↔ `lang_code_to_name()` (`player.rs`) map identical ISO 639-1/2 codes. Touch one → check the other.

## Background threading

Use `std::thread::spawn`; results sent over mpsc channels. No async except MPRIS tokio thread.

## Commits

Never add `Co-Authored-By` trailers. Always ask user before committing or pushing (CHECKIN.md).
After a release commit: `git tag vX.Y.Z && git push origin vX.Y.Z`.
