## 1. Halfblocks while dimmed

Do this first and independently of the search modal. It changes six existing modals, so it should be verifiable on its own before the new modal exists to confuse the picture.

- [ ] 1.1 Hold a second halfblock `Picker` alongside `image_picker` (`src/app/app_struct.rs:176`), built the same way as the fallback in `build_image_picker` (`src/app/construct.rs:496-512`).
- [ ] 1.2 Add a flag recording whether a dimmed backdrop is currently showing, set by `render_modal_frame` (`src/app/render/overlays/modal_frame.rs:14`) so every dimming overlay gets the behaviour without opting in.
- [ ] 1.3 Make the image protocol part of the `card_image_states` cache key, so sixel and halfblock variants coexist rather than evicting each other. Route fetch, resize-registration, and resize-response handling through the same keying — see the #164 comment at `app_struct.rs:164-175` about responses carrying no key of their own.
- [ ] 1.4 Select the picker and key suffix from the dim flag at render time, in the one place that builds a `ThreadProtocol` (`src/app/images.rs:300-307`).
- [ ] 1.5 Give `image_cache_size`/`image_lru` headroom for both variants of the visible set, so toggling a modal does not evict the variant it is about to need again.
- [ ] 1.6 Confirm a protocol miss refills from `read_image_disk_cache` (`images.rs:344`) and never re-hits the network.
- [ ] 1.7 Confirm no change when the configured protocol is already halfblocks, and no images in either state when image rendering is disabled.
- [ ] 1.8 Manually verify under sixel, kitty, and halfblocks: open the confirm modal over a poster-dense library view and check the artwork dims with everything else and does not bleed through the modal body. Repeat the open/close cycle and confirm images are not refetched.
- [ ] 1.9 Confirm the other five dimming modals (daemon-lost, remote-reanchor, multiselect, save-playlist, library-routes) look correct with dimmed artwork.

## 2. Unified search state

- [ ] 2.1 Add a `SearchMode` enum (`Fuzzy`, `Global`) and a `SearchModal` struct owning `mode`, `query`, `last_query`, `results: Vec<MediaItem>`, `corpus: Vec<MediaItem>`, `cursor`, `scroll`, `loading`, and `type_filter`, in its own module.
- [ ] 2.2 Port `type_sort_key` and `available_types` from `HomeSearch` unchanged, and the filtered-results accessor.
- [ ] 2.3 Port the background dispatch and channel drain from `SearchSubsystem` (`src/app/search.rs:136-166`), retargeting them at the new state. Keep the reset of cursor, scroll, and type filter on each new result set.
- [ ] 2.4 Add fuzzy scoring: match `query` against `corpus` item display names with `SkimMatcherV2`, sort by descending score, and store the matched items into `results`. Reuse the scoring logic from `src/app/library_load_actions.rs:331-385`.
- [ ] 2.5 Add a navigable-type predicate reading the same type-to-collection-type map used by `spawn_navigate_to_item` (`src/app/library_browse_actions.rs:384-390`), and filter global results through it before they reach `results`. One map, two readers — do not duplicate the type list.
- [ ] 2.6 Unit-test: fuzzy ordering by score; global drain replacing results and resetting cursor/scroll/filter; error drain clearing loading without populating results; unnavigable types filtered out; all-unnavigable yielding an empty result set.

## 3. Corpus wiring

- [ ] 3.1 On opening fuzzy search, populate `corpus` from the library root level's `all_items` (`BrowseLevel.all_items`, prefetched by `spawn_all_items_prefetch`), regardless of the current navigation depth.
- [ ] 3.2 On music libraries, populate `corpus` from the recursive album index instead, reusing the eligibility check and index lookup in `src/app/library_search_actions.rs:73-101`.
- [ ] 3.3 Set `loading` when the corpus is not yet available, and fill it when `LibEvent::AllItemsPrefetched` or `LibEvent::AlbumIndexBuilt` arrives while the modal is open.
- [ ] 3.4 Unit-test: opening at a nested level yields the root corpus, not the level's items; an active letter filter does not truncate the corpus; an unloaded corpus reports loading rather than no-matches.

## 4. Modal frame background

- [ ] 4.1 Add a background-colour parameter to `render_modal_frame` (`src/app/render/overlays/modal_frame.rs:14`).
- [ ] 4.2 Update the six existing callers (`confirm_modal.rs:18`, `daemon_lost_modal.rs:21`, `remote_reanchor.rs:18`, `multiselect.rs:171`, `playlists.rs:270`, `library_routes.rs:398`) to pass the colour they use today, so their appearance is unchanged.
- [ ] 4.3 Confirm each of those modals still renders identically.

## 5. Modal renderer

- [ ] 5.1 Add the search modal renderer under `src/app/render/overlays/`, sized at 60%×80% with a minimum-size floor, via `render_modal_frame`.
- [ ] 5.2 Render the search input row using the playback-panel background with a seek-track border, and the modal body using the library-side background.
- [ ] 5.3 Render results as a single-column flat list: type badge, then title, then type-appropriate row meta. Include the parent series for episodes and the artist for tracks.
- [ ] 5.4 Render the type-dispatched hero inline below the selected row: title, meta line, overview at full content width, with the `MEDIA_SELECTED_BG` fill and `▁`/`▔` `SEEK_TRACK` rules lifted from `src/app/render/list.rs:378-420`. No image, no reserved image width, no image fetch.
- [ ] 5.5 Implement scroll so the selected row and its hero are always fully visible, scrolling by the minimum amount needed.
- [ ] 5.6 Render the type filter control in global mode only, populated from `available_types()`.
- [ ] 5.7 Render loading, empty, and error states. In fuzzy mode, have the empty state hint that a second search key press searches the whole server.
- [ ] 5.8 Confirm no code path in the modal reaches `render_power_list`, `render_power_compact_detail`, or `compact_banner_layout_with_overview`.

## 6. Input

- [ ] 6.1 Add `last_slash_at: Instant` to `App` and open the modal in fuzzy mode on `/` from a library tab, acting immediately with no deferral.
- [ ] 6.2 Promote to global on a second `/` within the double-click window, only while the query is empty; preserve the query and dispatch. Reuse the same interval constant as `src/app/input_mouse_dispatch.rs:153-157`.
- [ ] 6.3 Insert `/` into the query as a literal character when the query is non-empty.
- [ ] 6.4 Open the modal in global mode directly on `/` from the home tab.
- [ ] 6.5 Handle text entry, selection movement, type-filter cycling, activation, and dismissal within the modal, and ensure the modal takes input precedence over the view beneath.
- [ ] 6.6 On activation with a selected result, call `spawn_navigate_to_item` and close the modal. With no selected result, do nothing and leave the modal open with its query intact.
- [ ] 6.7 On dismissal, close outright from either mode without demoting global to fuzzy, restore prior focus, and leave the underlying navigation position untouched.
- [ ] 6.8 Unit-test: double press promotes; single press does not; `/` after a character is literal; promotion preserves the query; home-tab `/` opens global; activation with no selection is inert and non-closing; dismissal from global closes in one press.

## 7. Delete the old paths

- [ ] 7.1 Remove the inline search input box from `src/app/render/list.rs:217-262`.
- [ ] 7.2 Remove the filtered-item/cursor/scroll substitution at `src/app/render/list.rs:100-131`.
- [ ] 7.3 Remove the now-dead `&& search.is_none()` term from the `use_letter_groups` condition (`list.rs:200-206`), and confirm `show_grouped` (`list.rs:170`) can no longer receive a filtered vector.
- [ ] 7.4 Delete `LibSearch` (`src/app/types_browse.rs:3-10`), the `LibraryTab.search` field, and `update_lib_search` (`src/app/library_load_actions.rs:331-385`).
- [ ] 7.5 Delete `HomeSearch` and the old `SearchSubsystem` (`src/app/search.rs`), and `src/app/input_home_search_keys.rs`.
- [ ] 7.6 Remove tests that covered the deleted inline-search rendering path. Per repo policy, do not troubleshoot them — delete and replace with tests against the new behaviour.
- [ ] 7.7 Grep for remaining references to `LibSearch`, `HomeSearch`, `home_search`, and `lib.search` and confirm none remain.

## 8. Verification

- [ ] 8.1 `cargo clippy --workspace --all-targets` clean.
- [ ] 8.2 `make check-code-file-lines` passes; split any file pushed over the 800-line cap in this change.
- [ ] 8.3 Manually verify in a real terminal: fuzzy search on a music library returns correct albums in score order with no artist headers and no mismatched rows.
- [ ] 8.4 Manually verify fuzzy search from a nested level searches the whole library.
- [ ] 8.5 Manually verify `//` reaches items outside the current library, that badges and meta lines are correct across mixed types, and that activating a result lands on it in the right library tab.
- [ ] 8.6 Manually verify the modal at a very small terminal size and immediately after a resize.
- [ ] 8.7 Manually verify the search modal specifically over a poster-dense view under sixel and kitty: backdrop artwork dims and nothing bleeds through the modal body.

## 9. Close out

- [ ] 9.1 Update `AGENTS.md` if any path referenced there has moved.
- [ ] 9.2 Commit the proposal, design, spec, and tasks with the code in the same PR.
- [ ] 9.3 Close issue #446 with a note on what shipped and what was deliberately left out.
