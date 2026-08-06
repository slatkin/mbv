## Context

Two search implementations exist, sharing nothing.

**Fuzzy, in-library.** `LibSearch` (`src/app/types_browse.rs:3-10`) lives at `LibraryTab.search` and holds `results: Vec<usize>` — indices into its own `items` snapshot, sorted by `SkimMatcherV2` score (`src/app/library_load_actions.rs:331-385`). It is rendered by substitution: `render_power_list` swaps the search's items, cursor, and scroll in for the real nav-level values (`src/app/render/list.rs:100-131`) and draws a 3-row input box above the list (`list.rs:217-262`). The index-based result model exists solely to feed that substitution.

The corpus is already whole-library. `BrowseLevel.all_items` is prefetched by `spawn_all_items_prefetch` (`src/app/library_browse_actions.rs:538`), and `full_library_fetch_limit` (`library_browse_actions.rs:41`) deliberately uses `lib.library_total` rather than `lvl.total_count` so an active letter pill cannot truncate it. Music libraries instead use an async recursive album index (`open_recursive_album_search` → `sync_recursive_album_search`, `src/app/library_search_actions.rs:73-101`), fed by `LibEvent::AlbumIndexBuilt`.

**Global, server-wide.** `EmbyClient::search_items` (`crates/mbv-core/src/api_client_library.rs:156-164`) hits `/Users/{id}/Items?SearchTerm=…&Recursive=true`. `SearchSubsystem` dispatches it on a thread and drains via channel (`src/app/search.rs:136-166`). `HomeSearch` (`search.rs:5-14`) owns `results: Vec<MediaItem>` directly and already carries `type_filter`, `available_types()`, and a `type_sort_key()` ordering Movie → Series → Episode → Audio → MusicAlbum → MusicArtist. All of it works. None of it renders: `render/home.rs` was deleted with Standard view (`860e672`, #361) and nothing has read `self.search.state()` since.

The two states differ structurally in exactly one way — `Vec<usize>` versus `Vec<MediaItem>` — and that difference is an artifact of the rendering path being deleted.

## Goals / Non-Goals

**Goals:**

- One search surface, one state type, one renderer, for both modes.
- Make fuzzy search correct on music libraries by removing the filtered-list substitution rather than patching around it.
- Give global search a renderer again without reimplementing its state machine or API call.
- Keep the modal free of images, and free of any grouping.
- Make a mixed-type global result list readable at a glance.
- Make dimming actually dim, artwork included.

**Non-Goals:**

- Changing the dimming arithmetic in `dim_backdrop`, or using halfblocks at any time other than while a dimmed backdrop is showing.
- Grouping, letter pills, or multi-column layout inside the modal.
- Matching on anything but item name.
- Extending `spawn_navigate_to_item`'s type coverage.
- Search history or cross-session persistence.

## Decisions

### 1. One `SearchModal` state; delete both existing structs

```rust
pub(crate) enum SearchMode { Fuzzy, Global }

pub(crate) struct SearchModal {
    mode: SearchMode,
    query: String,
    last_query: String,
    results: Vec<MediaItem>,
    corpus: Vec<MediaItem>,   // Fuzzy only; empty in Global
    cursor: usize,
    scroll: usize,
    loading: bool,
    type_filter: usize,       // Global only
}
```

Results are owned `MediaItem`s in both modes. Fuzzy fills `results` by scoring `corpus` with `SkimMatcherV2` and cloning the winners; Global fills it from the channel drain. One renderer then handles both, because after the query resolves the two modes are indistinguishable to the view.

`LibSearch`'s `Vec<usize>` bought a cheap swap into the library list renderer. That renderer is no longer a consumer, so the indirection is pure cost — it forces the modal to carry a parallel `items` vector and index through it on every draw. Cloning matched items is bounded by what fits in a result list and happens once per query change, not per frame.

Promotion from Fuzzy to Global is a mode flip on a live struct: `query` and `last_query` survive, `corpus` is dropped, `results` is cleared, `loading` is set, and the Emby request goes out. No data migrates between types because there is only one type.

### 2. Fuzzy corpus is the library root's `all_items`, regardless of current depth

`/` inside `Shows > Breaking Bad > Season 2` searches the whole TV library, not that season. The corpus is `nav_stack[0]`'s `all_items` — already prefetched, already whole-library by construction — or the recursive album index on music libraries.

The consequence worth stating plainly: a library root holds top-level entities, so fuzzy matches series, movies, and albums, but not episodes or tracks. Reaching a leaf by name is what `//` is for. This is the existing behaviour, not a regression; making fuzzy reach leaves would require a recursive fetch per library, which is what the Emby search endpoint already does better.

When the corpus is not yet loaded (music index still building, or prefetch in flight), the modal shows its loading state rather than an empty result list, so "no matches" never lies.

### 3. Text-only hero — a sibling renderer, not a parameterized `render_power_compact_detail`

`render_power_compact_detail` (`src/app/render/detail.rs:358`) is image-shaped throughout. `compact_banner_layout_with_overview` (`detail.rs:189-230`) fetches the Primary image, consults the 150ms nav-idle debounce `power_right_panel_image_renders_allowed()`, queries `image_picker` for font metrics to fit the poster's aspect ratio, and reserves `img_actual_w` columns that the text then flows around. With no image, every one of those concerns is dead weight, and the text wants full panel width rather than a poster-shaped gap.

So the modal gets its own hero: title row, type-dispatched meta line, overview, wrapped to full width. The surrounding chrome is lifted from `list.rs:378-420` — `MEDIA_SELECTED_BG` fill with `▁`/`▔` borders in `SEEK_TRACK` — which is already item-agnostic.

This is why the modal does not reuse `render_power_list` at all. That function takes no items parameter and reads `self.library_tab`, `self.libs[i].nav_stack`, and `self.libs[i].search` directly (`list.rs:52-131`); its grouped-album branch reaches further into the music grouping catalog. With flat rows and a text hero, the modal's renderer is self-contained and `list.rs` needs no new parameters — only deletions.

### 4. `//` mirrors double-click, and does not defer

`input_mouse_dispatch.rs:153-157` computes `is_double` from a stored `last_click_time` and 400ms window, and critically does **not** delay the first click: the single action fires immediately, and the second click adds the double action. The same applies here. The first `/` opens the modal in Fuzzy mode with no latency; a second `/` within the window promotes it.

This works only because the first action is compatible with the second — the modal is already open and correct, so promotion is a mode swap rather than an undo. One `last_slash_at: Instant` on `App`; no position component, unlike the mouse case.

Promotion requires `query.is_empty()`. A `/` typed after any other character is a literal search character, not a mode request.

On the Home tab there is no current library, so `/` opens Global directly and the promotion path is unreachable there.

### 5. Type badge and type-dispatched meta line

`MediaItem.item_type` (`crates/mbv-core/src/api_types.rs:83`) carries Emby's `Type` verbatim.

| `item_type` | Badge | Meta line |
|---|---|---|
| `Movie` | MOVIE | year · runtime · rating · ★score |
| `Series` | SERIES | year · seasons · rating |
| `Episode` | EPISODE | series name · S1E3 · runtime |
| `MusicAlbum` | ALBUM | album artist · year · tracks |
| `Audio` | TRACK | artist · album · duration |
| `MusicArtist` | ARTIST | albums |
| `BoxSet` | COLLECTION | items |

`Episode` and `Audio` need their parent in the **row**, not only the hero — `S1E3` alone is meaningless in a flat mixed list. `api_types.rs:205` already prefixes episodes with `series_name` and `:161` handles the audio-plus-artist case.

The badge column is present in both modes even though it is near-uniform in Fuzzy. A mode-dependent column would shift the layout at the exact moment `//` promotes, which is the worst time to move things.

The badge is presentational only. The matcher scores `display_name()`, so typing `movie` does not match every film.

### 6. Type filter in Global only

`available_types()` and `type_sort_key()` carry over unchanged. In Fuzzy the results are near-homogeneous and the filter row is wasted chrome, so it is hidden. `type_sort_key` still orders results in both modes.

### 7. Enter navigates; unnavigable types never appear

`spawn_navigate_to_item` (`src/app/library_browse_actions.rs:375`) already resolves an item to a library tab and nav path, and is live behind the context menu (`context_menu_actions.rs:80`) and queue keys (`input_queue_keys.rs:394`).

Its type map (`library_browse_actions.rs:384-390`) covers `Series`, `Episode`, `Season`, `Movie`, `Audio`, `MusicAlbum`, `MusicArtist`. Anything else falls through to `"No matching library for this item type"`. Rather than surfacing that error, the Emby response is filtered to navigable types before display, so every visible result has a working Enter. The filter reads from the same map, so extending the map later widens results automatically without a second list to keep in sync.

Enter closes the modal only when it actually navigates. With no result selected — empty results, or a query that has not resolved yet — Enter is inert and the modal stays open. Closing on a keypress that accomplished nothing would read as an accidental dismissal, and it would punish the exact moment the user is most likely to be typing ahead of the results.

`Esc` always closes outright. It never demotes global back to fuzzy: an escape key that sometimes closes and sometimes changes mode has to be pressed twice to reliably leave, which is worse than `//` lacking a symmetric undo. Reopening with `/` costs one keystroke.

### 8. Modal geometry, and the `render_modal_frame` colour inversion

60% width × 80% height, centered, floored at a minimum below which the modal would be unusable. `render_modal_frame` (`src/app/render/overlays/modal_frame.rs:14`) already dims, centers, clamps to terminal bounds, and returns an inner rect.

It does not give the right colours. It paints `LIBRARY_SIDE_BG` (#2d353b) as the *frame border* and `BG_GREEN` (#3c4841) as the inner fill — the inverse of what this modal needs, which is #2d353b as the body. It takes a background parameter so the search modal can supply its own fill; existing callers pass what they use today.

The rest of the palette already exists: `PLAYBACK_PANEL_BG` (#333c43) for the search input row, `SEEK_TRACK` (#46545f) for its border and the hero rules, `SOFT_WHITE` for text.

```
┌────────────────── Search ──────────────────┐
│ ┌────────────────────────────────────────┐ │  SEEK_TRACK
│ │ blade runner▏                          │ │  PLAYBACK_PANEL_BG
│ └────────────────────────────────────────┘ │
│                                            │
│   MOVIE   Blade Runner                     │
│  ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁  │
│   Blade Runner                             │  hero: MEDIA_SELECTED_BG
│   1982 · 117m · R · ★8.1                   │  text only, full width
│   A blade runner must pursue and termin…   │
│  ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  │
│   MOVIE   Blade Runner 2049                │
│   ALBUM   Blade Runner · Vangelis          │
│   EPISODE Blade Runner: Black Lotus · S1E3 │
│                                            │
└──── LIBRARY_SIDE_BG ───────────────────────┘
```

### 9. Deleting the inline path fixes the music bug at its root

`show_grouped` (`list.rs:170`) lacks the `&& search.is_none()` guard that `use_letter_groups` has (`list.rs:200-206`), so with a search active the grouped-album renderer receives the filtered, reordered vector while `GroupedAlbumCatalog.entries[i].album_index` (`src/app/music_grouping.rs:29-31`) indexes the original unfiltered items. In-range indices point at the wrong album; the rest are dropped by a bounds filter.

Adding the guard would fix it. Removing the substitution removes the possibility: with no filtered vector reaching `list.rs`, the catalog's indices cannot desync from what they index. The `search.is_none()` term on `use_letter_groups` becomes dead and goes too.

### 10. Backdrop images render in halfblocks whenever the backdrop is dimmed

`dim_backdrop` (`src/app/render/overlays/backdrop.rs:28`) walks the frame's cells and halves each RGB channel of `fg` and `bg`. Sixel and kitty images are not cells — they are escape sequences written past the cell grid — so they are untouched by it. Today a dimmed backdrop leaves every poster at full brightness while everything around it darkens, and a modal large enough to overlap artwork risks that artwork bleeding through its body. Halfblocks are ordinary cells, so they dim correctly and are erased correctly by anything painted over them.

The rule is tied to dimming, not to this modal: **while a dimmed backdrop is showing, images render in halfblocks.** Every caller of `render_modal_frame` gets it, because a per-modal opt-in would leave the same defect in the five other dimming modals and invite the question again at each new one.

Implementation turns on the cache, not the picker. `card_image_states` (`src/app/app_struct.rs:154`) is `HashMap<String, Option<ThreadProtocol>>`, and the `DynamicImage` is consumed into `picker.new_resize_protocol(img)` (`images.rs:307`) with no copy retained — so entries are protocol-bound and cannot be re-encoded in place. Invalidating and refilling on every open and close would be wasteful and visibly flickery.

Instead the protocol becomes part of the cache key, and a second halfblock `Picker` is held alongside the configured one:

```
in-memory   card_image_states["abc123@sixel"]     → ThreadProtocol (sixel)
            card_image_states["abc123@halfblock"] → ThreadProtocol (halfblock)

on-disk     read/write_image_disk_cache("abc123")  ← NO protocol suffix
```

**The suffix must apply to the in-memory key only.** The disk cache stores the downloaded source bytes (`write_image_disk_cache(&cache_key, b)`, `images.rs:405`), which are protocol-independent — the same `Vec<u8>` decodes to the same `DynamicImage` whichever picker later encodes it. Suffixing the disk key too would make the first dim miss on disk as well and re-download every visible image from the server, turning a local re-encode into a network round trip. Since `fetch_card_image` currently passes one `cache_key` to both layers, the two keys have to be derived separately at that seam.

Opening a dimming modal then misses only the in-memory cache and refills from disk — a decode plus resize-encode, already off the render thread via the existing resize worker, no network. Closing hits the still-warm sixel entries instantly; reopening hits the now-warm halfblock entries instantly. The LRU (`image_lru`, `image_cache_size`) needs headroom for both variants of the visible set, or the two evict each other and every toggle pays the decode again.

When the configured protocol is already halfblocks, the key is the same in both states and nothing changes.

## Risks / Trade-offs

- **The image pipeline is concurrent and terminal-dependent.** The swap touches `card_image_tx`, the per-key `resize_register_tx` channels, and `resize_response_rx` — machinery that already carries a scar from #164 about routing responses to the right cache key. Doubling the key space is the smallest change that does not disturb that routing, but it must be verified under sixel, kitty, and halfblocks in a real terminal; render tests cannot see any of this.
- **Six existing modals change appearance.** Tying the rule to dimming rather than to this modal is the right call, but it means confirm, daemon-lost, remote-reanchor, multiselect, save-playlist, and library-routes all start dimming their backdrop artwork. That is the intended fix, and it is also a wider blast radius than the feature itself.
- **First-open latency.** The first dim after startup pays a decode plus encode for each visible image. Off-thread and disk-backed, so it should read as images settling in rather than a stall — but on a poster-dense view it is the most likely place for visible lag.
- **`list.rs` is fragile.** It carries the scroll clamp duplicated across two renderers and was reworked recently by #448. The change here is deletion only, which is the safest kind, but the deletions are interleaved with live layout code.
- **Cloning matched items.** Fuzzy now clones winners instead of storing indices. Bounded by result-list size and recomputed only on query change.
- **Fuzzy cannot reach leaves.** Searching a TV library by episode name finds nothing until the user types `//`. Discoverable only if the modal says so; a hint in the empty state is cheap insurance.
- **Two orphaned-code paths must actually die.** `HomeSearch`, `LibSearch`, `input_home_search_keys.rs`, and the `list.rs` search box all have to be removed rather than left alongside the new state, or the next reader inherits three search implementations instead of two.

## Migration Plan

Not applicable. No persisted state, no protocol surface, no config keys. `search_items` is unchanged on the wire.

## Open Questions

None outstanding.
