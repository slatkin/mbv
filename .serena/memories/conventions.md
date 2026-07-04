# Conventions

## UI / Output

- **No emoji, ever.** No filled Unicode media symbols. Plain geometric/box-drawing or ASCII only.
- **No Co-Authored-By** trailers in commit messages.
- Commit only when user explicitly asks; never push unless user explicitly says to.

## Code style

- Fix all compile warnings — delete unused code, never `#[allow(unused)]`.
- Small, modular files logically separated by function. No monolithic files.
- `pub(super)` for methods called by sibling modules; `pub(crate)` only when needed outside `app/`.
- `impl App` is split across multiple files in `src/app/` — this is intentional Rust pattern.

## Colors

- All color constants in `src/app/palette.rs` (e.g. `FOAM` = Emby blue, `IRIS` = Emby green).

## MediaItem quirks and api.rs browse methods

See `mem:api` — MediaItem parsing gotchas (`production_year`, `is_folder`, `total_count`) and `fetch_items` usage.

## QueueSource

`queue_source: QueueSource` tracks how queue was loaded (`Playlist`, `Album`, `Series`, `Shuffle`, `Remote`, `Collection { collection_type }`, `Unknown`). Persisted in `queue_state.json`. Set after every queue-replacing op; cleared by `on_queue_replace_silent()`.

## card_image_states

`card_image_states: HashMap<String, Option<StatefulProtocol>>` — keyed by `"{item_id}:{slot}"` (e.g. `"abc123:lib"`, `"abc123:A"`).

## image_protocol_enabled

Cached from `client.config.image_protocol.is_some()`; updated in settings handler. Read via `images_enabled()`, not by checking config directly.

## music_levels

`music_levels: Vec<String>` from config (e.g. `["group", "album"]`), maps nav stack depth to folder semantics. Drives `is_album_level()` / `is_viewing_album_folders()`.

## Divider status indicators

Right-aligned bracketed indicators in `render/mod.rs`. See `docs/divider-indicators.md` for the recipe when adding a new one.
