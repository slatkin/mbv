# Scoping: tasks 3.3 (Inline Search) + 3.5 (Emby browser) for parallel agents

**Purpose:** break 3.3 and 3.5 into agent-sized briefs. Written after reading
the actual code (not the task-list description alone) because the two tasks
are more entangled with each other, and with future tasks 4.2/4.3/4.4, than
`tasks.md` suggests.

## Why this can't just be split by task number

- `BrowserKey` is currently a unit struct (`component_id.rs:28`) with a TODO
  saying it gets fleshed out "at task 3.5" — but 3.3's own `ComponentId`
  (`InlineSearch(BrowserKey)`) needs the same real key. Whoever defines it
  blocks the other task until it lands.
- No `BrowserKind` (or equivalent) enum exists anywhere in the codebase yet
  (confirmed by grep) — it has to be invented, and its variants (generic /
  movies / home-video / TV / music at minimum) double as the seam that keeps
  3.5 from silently absorbing 4.2/4.3's territory.
- `libs: Vec<LibraryTab>` (`app_struct.rs:106`) is a flat, index-addressed
  `Vec` today, not keyed by anything like `BrowserKey`. Deciding how tab
  index maps to a stable key, and when components mount/unmount as tabs are
  added or removed (`rebuild_library_tabs_from_views`,
  `library_load_actions.rs:248`), is a design decision — not a mechanical
  extraction.
- `LibraryTab` (`types_library_tab.rs`) bundles fields owned by **four**
  different tasks in one struct: `search` (3.3), `nav_stack` / `library` /
  `library_total` (3.5), `series_selection` / `series_season_cursor` (4.2,
  TV workspace), `album_track_focus` (4.4, inline album-track). None of these
  can move onto a component until the BrowserKey/instancing model exists, and
  none should move except by the task that owns it.
- The single input entry point for every Emby library tab,
  `handle_key_emby_library` (`input_browse_dispatch.rs:78`), interleaves all
  four of the above concerns as **runtime-gated branches inside one
  function** — music-group switching, home-video-feed-group switching,
  letter-pill cycling, album-track-focus mode, series-selection mode, and
  only at the bottom a fallthrough to `handle_lib_key` (the actual generic
  list movement 3.5 owns). A literal move of this function would drag
  Music/TV/Album input logic into 3.5 by accident.
- Render has the same shape: `list.rs` (620+ lines), `widgets.rs`,
  `home_feed.rs`, `screens/album_cursor.rs`, `screens/pills.rs` all branch on
  `nav_stack` / `is_music_group_view` / album / series state inline, shared
  across 3.5 and the not-yet-started 4.2/4.3/4.4.

## Recommended shape: one prerequisite, then parallel lanes

### 3.5a — `BrowserKey` + instancing model — DONE (this session, 2026-08-24)

Landed directly rather than handed to an agent, per user choice. Built:

- `BrowserKind` (`component_id.rs`): `Generic`/`Movies`/`TvShows`/`Music`/
  `HomeVideos`, with `from_collection_type(&str)` mirroring the
  `collection_type` string comparisons already scattered across
  `library_browse_actions.rs`/`music_actions.rs`/`feed_actions.rs`/
  `lib_cursor_actions.rs` (not a new taxonomy — named the existing one).
  Unrecognized/empty strings fall back to `Generic`, matching those call
  sites. `collection_type` itself is `EmbyItem`'s field, defined in
  `mbv-core/src/api_types.rs`.
- `BrowserKey { service: mbv_core::config::ServiceKind, library_id: String,
  kind: BrowserKind }` — reused the existing `ServiceKind` (`Emby`/
  `Audiobookshelf`) rather than inventing a new type; `library_id` is a
  plain `String` matching this codebase's existing identifier convention
  (`BrowseLevel::parent_id`, `focused_item_id`, etc. are all plain `String`
  already, not newtypes).
- **Breaking consequence, fixed:** a `String` field means `BrowserKey`, and
  therefore `ComponentId`, can no longer derive `Copy` (only `Clone`).
  Fixed the ~10 call sites this broke (`shell_overlays.rs`'s six
  mount-then-`active()`/`umount()` sites needed `id.clone()`;
  `key_policy.rs`'s `KeyPolicyEntry`/`Owner`/`Gate` derives dropped `Copy`
  too). Purely mechanical, `cargo check`-driven.
- **Real snag found (not swept under the rug):** `key_policy.rs`'s
  `lib_search` and `album_track_mode` entries previously "constructed"
  `ComponentId::InlineSearch(BrowserKey)`/`Browser(BrowserKey)` using the
  old unit-struct placeholder — invalid now that `BrowserKey` carries real
  per-instance data. There is no single static `ComponentId` value for "the"
  inline search or browser, since one exists per mounted tab. Changed both
  entries' `owner` to `KeyPolicyOwner::Active(None)` (the same "whichever is
  actually focused, not statically known" pattern the table already used for
  `view_dispatch`) and left their `gate` as a `Custom(&str)` describing the
  real runtime condition. **This is still open, not resolved:** whoever
  wires `lib_search`/`album_track_mode` live (3.3 / 4.4) has to decide how a
  per-instance `SubClause` guard gets built at mount time, since TuiRealm's
  `SubClause::IsMounted`/`HasAttrValue` take one concrete `ComponentId`, not
  "any instance of this variant."
- No behavior change to production code; full suite green (1152 tests, fmt
  clean, clippy clean — 3 pre-existing warnings only, ast-grep 71
  pre-existing errors unchanged, all governed files ≤ 800 lines).

### Lane A — 3.5 render (mirrors the 3.1 → 3.2 precedent already used for Search)

Seam-extraction scope: **broad** (per user choice) — split all four
concerns now (generic/movies/home-video, music-group, album-track-focus,
series-selection) into their own named functions, not just the one 3.5
itself needs, since 4.2/4.3/4.4 will need that same split later and
re-touching `input_browse_dispatch.rs`/`list.rs` per future task is wasteful.

- **3.5b, seam extraction (behavior-preserving):** split
  `handle_key_emby_library`'s runtime branches into four named functions —
  generic/movies/home-video fallthrough, music-group switching,
  album-track-focus mode, series-selection mode — and do the same for the
  equivalent inline branching in `list.rs` / `widgets.rs` / `home_feed.rs` /
  `screens/album_cursor.rs` / `screens/pills.rs`. Output must be provably
  unchanged (existing characterization tests pass unmodified). This is the
  biggest single diff in this whole scoping — budget accordingly, and
  consider whether it's still one task or wants its own further split by
  concern (e.g. input seam vs. render seam as two sequential steps for one
  agent, since they touch different files).
- **3.5c, conversion:** move only the generic/Movies/home-video-owned
  functions produced by 3.5b into the new `BrowserComponent`. The
  music/TV/album functions stay `impl App` for now, called from the
  component through the same kind of legacy-bridge pattern `LegacyInput` /
  `shell.rs` already establish for Home, until 4.2/4.3/4.4 claim them.

### Lane B — 3.5 input/action + shell wiring

Can run in parallel with Lane A once 3.5a lands. Touches mostly disjoint
files (`input_browse_dispatch.rs`'s dispatch shell, `library_load_actions.rs`,
`lib_event_actions.rs`), but needs its own audit pass before scoping further:
`library_load_actions.rs` interleaves playlist / home-fetch / ABS / feeds /
lib-search concerns with library-tab lifecycle (it is not purely a "browse"
file despite the name); `lib_event_actions.rs` (790 lines) has not been read
this session and should be mapped by whoever picks this up before editing.

### 3.3 — Inline Search

**Superseded by the 2026-08-24 session-3 correction below: 3.3 does NOT run
in parallel with Lane A/B on 3.5a alone.** It needs a real seam-extraction
prerequisite of its own, overlapping Lane A's deferred render work. Left
here for the original (too-optimistic) reasoning; see the correction section
for the actual dependency.

- Render entry: `render_search_box` (`hero.rs:657`) — distinct from the
  already-converted global search sidebar (`search_sidebar.rs`, task 3.2).
- Input entry: `handle_key_lib_search` / `handle_lib_search_key`
  (`input_lib_keys.rs:27,61`).
- State/actions: `LibSearch` (`types_browse.rs:3`), `update_lib_search`
  (`library_load_actions.rs:514`), `library_search_actions.rs` (321 lines,
  all `impl App`).
- **Collision risk:** shares `library_load_actions.rs` and
  `types_library_tab.rs` with Lane B. If run truly in parallel, coordinate
  edit order on those two files or expect a merge.

## Correction (2026-08-24, session 3): 3.3's real entanglement, found by tracing the code

An `/opsx:apply` session scoped narrowly to 3.3 traced every read/write of
`LibraryTab.search` and every caller of `render_search_box` before writing
any code, and found the entanglement is bigger than this doc's "only needs
3.5a" claim above — on both the input side (worse than expected) and the
render side (much worse, and not previously identified at all). No code was
written this session; `tasks.md`'s 3.3 checkbox is still unticked.

### Input/cursor/mouse: 4 more files than scoped

`LibraryTab.search` (the field, not just the query-editing keys) is read or
written directly inside shared, not-yet-converted browse-cursor code that
belongs to task 3.5, not 3.3:

- `lib_cursor_actions.rs` — `move_lib_cursor`/`jump_lib_cursor`/
  `current_library_columns`/`is_viewing_season_grid` all branch on
  `lib.search.is_some()` to redirect cursor movement between search results
  and the nav-stack list (8 branches total).
- `actions_navigation.rs` — `select` (search-result activation reuses the
  same folder-descend/play logic as a normal list select, resolved through
  `current_lib_item`'s search branch) and `go_back` (Esc/Backspace pops
  search before nav_stack).
- `input_mouse.rs` — at least one click-to-select site sets `search.cursor`
  as a fallback alongside `nav_stack` cursor (e.g. the wide-TV right-rail
  click handler, ~line 318; there may be others not yet enumerated).
- `lib_event_actions.rs` — three `LibEvent` handlers touch `lib.search`
  directly: `SearchItemsLoaded` (async full-library fetch completion),
  `RecursiveAlbumActivated` (clears search after a recursive-album
  activation), and one more session-2 didn't isolate.

This part is still tractable: once `InlineSearch` owns its own results
cursor (Up/Down/PageUp/PageDown/Home/End over its own `results`/`cursor`/
`scroll`, mirroring `SearchSidebarComponent::move_cursor`) and Enter emits
one `Msg::Shell` carrying the resolved item (instead of round-tripping
through `App::select`/`current_lib_item`'s search branch), every
`search.is_some()` branch above collapses to its already-there `else` arm —
mechanical dead-code deletion once the field is gone, not a redesign. `select`
likely wants a small extraction (`select(lib_idx)` → resolve item →
`select_item(lib_idx, item)`) so both the plain-list Enter and the new
component's activation Msg share one body.

Also non-trivial and not previously flagged: `update_lib_search`
(`library_load_actions.rs:514`) and its neighbours in
`library_search_actions.rs` are not just "edit a query string" — they cover
two really different search modes:
- **Plain mode:** fuzzy-match (`fuzzy_matcher` crate, local computation, no
  network per keystroke — unlike the global Search sidebar's debounced
  server round-trip) against either already-loaded `nav_stack` items or a
  one-time full-library fetch (`spawn_search_items_load`,
  `library_browse_actions.rs:588`, delivered back via
  `LibEvent::SearchItemsLoaded`).
- **Recursive album mode** (music libraries, `recursive_album_search_enabled`):
  fuzzy-matches against `App.album_indexes` — a library-scoped, long-lived
  cache built by `spawn_album_index_build`/`AlbumIndexBuilt`, NOT owned by
  the search session — and `Enter` on a match runs `activate_recursive_album`
  (`library_search_actions.rs:148`), a separate async multi-level fetch that
  replaces the whole `nav_stack` and sets `album_track_focus`. This is
  clearly shell-owned cross-boundary work (Service access, nav_stack
  mutation), not something the component can do itself — but it means
  `InlineSearch` needs two candidate-pool shapes (plain items vs. recursive
  `AlbumSearchEntry`, which carries `ancestors`/`search_text` for the
  fuzzy-match key and the activation path), pushed in by the shell the same
  way `apply_drain` pushes results into `SearchSidebarComponent` today.

### Render: not scoped at all before this session, and much larger

`render_search_box` (`hero.rs:657`) only paints the single-row query input.
The actual results *list* renders through the same shared, unconverted list
painter used for the plain browse list — confirmed by grepping every caller
and every `.search` reference under `src/app/render/`:

- `render/components/list.rs` — `render_list` (536 lines) and
  `render_wide_library_rows` (69 lines) both branch on `lib.search`, and not
  just at the top: `search_active` feeds into hero-row sizing
  (`selected_movie_item`/`selected_series_item`, which have their own search
  branches in `detail.rs` — `selected_movie_item` ~104-126,
  `selected_series_item` ~128-147), letter-pill suppression, column
  computation, and the inline-hero-replacement geometry
  (`InlineReplacementPlan`). `render_list` dispatches *first* into three
  more full-page renderers for wide layouts, each with its own independent
  `lib.search` branch:
  - `render/components/tv_wide.rs::render_wide_tv` (~70 lines)
  - `render/components/movies_wide.rs::render_wide_movies` (~104 lines,
    plus `selected_wide_movie`'s own search branch)
  - `render/components/music_wide.rs::render_wide_music_group` (~193 lines)
    plus `render_wide_left_tracks` (~170 lines, the persistent wide-music
    track list, also search-branching)

Total: 5 files, roughly 1,500+ lines of dense, image/geometry-sensitive
code, none of it parameterized for component use today (`render_plain_rows`/
`render_letter_grouped_rows` in `list_letter_groups.rs` already take a typed
`ListRenderCtx` rather than `&self` — a real seam already exists one layer
down — but everything *above* that layer, the part that decides
`items`/`cursor`/`scroll`/`cols`/hero sizing, is still `impl App` reading
`lib.search`/`nav_stack` directly).

### What this means for the task order

3.3 as scoped ("child of one Emby browser," "distinct from global Search")
undersold how deep its render ownership goes: it doesn't paint its own
results, it swaps the item source feeding the SAME painter task 3.5 (and
4.2 for TV, 4.3 for Music) haven't converted yet. Converting 3.3 first,
correctly, means doing the render-seam-extraction half of 3.5b (parameterize
`render_list`/`render_wide_library_rows`'s item-source selection and the
three wide renderers to take component-supplied data) as 3.3's own
prerequisite — not "3.5a only" as this doc previously claimed.

Recommendation for whoever picks this up: either (a) sequence 3.3 genuinely
after 3.5b's render-seam extraction lands (the render_list/wide_* seam this
session found doubles as most of what 3.5b needs anyway, so this isn't
wasted if done as part of 3.5), or (b) explicitly scope a "3.3 render seam"
lane that does only the search-branch extraction in these 5 files (not the
rest of 3.5b's seam work) as 3.3's prerequisite, accepting some duplicate
effort against a later full 3.5b pass. Either way, budget for ~1,500 lines
of careful, behavior-preserving extraction across dense image/layout code
before any `InlineSearchComponent` can render — this is not a small task.

## Files read this session to produce this scoping (for the next agent's context)

`component_id.rs`, `app_struct.rs` (grep only), `types_browse.rs`,
`types_library_tab.rs`, `input_browse_dispatch.rs` (full), `input_lib_keys.rs`
(grep), `library_load_actions.rs` (grep), `library_search_actions.rs` (grep),
render-side file list under `src/app/render/**` (grep for `nav_stack`
occurrences only — none of these files' bodies were read).
