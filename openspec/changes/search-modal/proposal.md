## Why

Search is currently two half-working features that share nothing.

Fuzzy search over the current library filters the library list in place: `LibSearch` (`src/app/types_browse.rs:3-10`) holds indices into its own item vector, and `render_power_list` swaps those indices in for the real nav-level items (`src/app/render/list.rs:100-131`) behind a 3-row input box drawn above the list (`list.rs:217-262`). That in-place substitution is the source of two problems. The transition between "searching" and "browsing" is disorienting, because the same pane means two different things depending on invisible state. And on a music library it renders wrong results outright: `show_grouped` (`list.rs:170`) is derived only from `is_viewing_album_folders()` without the `&& search.is_none()` guard its sibling `use_letter_groups` check has (`list.rs:200-206`), so the grouped-album path receives the filtered, reordered vector while `GroupedAlbumCatalog.entries[i].album_index` (`src/app/music_grouping.rs:29-31`) still indexes the *original* unfiltered items. Indices that happen to fall in range point at the wrong album; the rest are dropped by a bounds filter.

Global search across the whole Emby server is fully implemented and completely unreachable. `EmbyClient::search_items` (`crates/mbv-core/src/api_client_library.rs:156-164`), the `HomeSearch` state machine with its type filter and type ordering (`src/app/search.rs:5-56`), the background dispatch and drain (`search.rs:136-166`), and the key handling (`src/app/input_home_search_keys.rs`) all still work and are still reachable via `/` on the Home tab. Its renderer died with `render/home.rs` when Standard view was deleted (`860e672`, #361). Results are fetched and thrown away.

Both problems dissolve into the same fix: move search out of the library list and into a modal that owns its own presentation. A modal cannot desync from a grouping catalog it does not use, and it gives global search the renderer it lost.

## What Changes

- Add a search modal at 60% of terminal width by 80% of height, over a dimmed backdrop, replacing the inline search box entirely.
- Unify `LibSearch` and `HomeSearch` into one `SearchModal` state owning `Vec<MediaItem>` results plus a `SearchMode` of `Fuzzy` or `Global`. Delete both existing structs.
- Open fuzzy search over the current library with `/`. A second `/` within the double-click window promotes the open modal to global search, preserving the query, mirroring the existing double-click detection at `src/app/input_mouse_dispatch.rs:153-157`.
- Render results as a flat single-column list with no groupings of any kind, with a text-only hero block inline below the selected row.
- Render no images in the modal. The hero shows title, a type-dispatched meta line, and overview at full panel width.
- Prefix each result row with a type badge, and dispatch the meta line on `MediaItem.item_type` so a mixed-type global result list is readable.
- Keep the type filter from `HomeSearch` in global mode only, reusing `available_types()` and `type_sort_key()`.
- On Enter, navigate to the selected item via the existing `spawn_navigate_to_item` (`src/app/library_browse_actions.rs:375`), which already resolves an item to its library tab and nav path and is already used by the context menu and queue keys.
- Delete the inline search rendering path from `list.rs`, which removes the music-grouping desync at its root rather than patching the missing guard.
- Render backdrop images in halfblocks whenever the backdrop is dimmed, so dimming applies to artwork as well as text. This applies to every modal that dims, not only the search modal, because `dim_backdrop` is a cell-level post-process and out-of-band graphics protocols are unaffected by it.

## Capabilities

### New Capabilities

- `search-modal`: a single modal surface for both current-library fuzzy search and server-wide Emby search, with flat results, a text-only inline hero, and type-aware presentation.

### Modified Capabilities

None. Library browsing, sort order, playback, and the queue are unchanged. The library list loses only its ability to render a filtered view of itself.

## Impact

- **Code**: New modal state module replacing `src/app/search.rs` and the `LibSearch` half of `src/app/types_browse.rs`. New modal renderer under `src/app/render/overlays/`. Deletions in `src/app/render/list.rs` (inline search box, filtered-item swap, and the `search.is_none()` guard that becomes dead). Key handling in `src/app/input_lib_power_keys.rs`, `src/app/input_queue_keys.rs`, and `src/app/input_home_search_keys.rs`. Background-fuzzy corpus wiring in `src/app/library_search_actions.rs`. A background-color parameter on `render_modal_frame` (`src/app/render/overlays/modal_frame.rs:14`). A second halfblock `Picker` plus protocol-aware image cache keys in `src/app/images.rs` and `src/app/app_struct.rs`.
- **Behavior**: `/` opens a modal rather than filtering the list in place. `//` reaches the whole server. Fuzzy search works correctly on music libraries. The Home tab's `/` stops being a dead end. Every dimming modal now dims artwork as well as text, which changes the appearance of the existing confirm, daemon-lost, remote-reanchor, multiselect, save-playlist, and library-routes modals.
- **Data/API**: None. `search_items` already exists and its response shape is unchanged.
- **Risk**: Medium-high. Deleting the inline search path touches `list.rs`, which is historically fragile and was reworked recently by #448. The protocol swap touches the image pipeline, which is concurrent (`card_image_tx`, `resize_register_tx`, `resize_response_rx`) and whose behaviour differs per terminal — it needs manual verification under sixel, kitty, and halfblocks, not tests alone. The swap also changes six existing modals, so a regression there is wider than this feature.

## Non-Goals

- Changing the dimming arithmetic itself in `dim_backdrop` (`src/app/render/overlays/backdrop.rs:28`). Only which images it can reach changes, not how much it darkens.
- Halfblock rendering at any time other than while a dimmed backdrop is showing. The user's configured protocol is unchanged everywhere else.
- Any grouping, letter pills, or multi-column layout inside the modal.
- Search history, saved searches, or query persistence across sessions.
- Searching by anything other than item name, including type names, genre, or cast.
- Extending `spawn_navigate_to_item`'s type coverage beyond the types it already handles.
