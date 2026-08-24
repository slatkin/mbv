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

Only needs 3.5a (the `BrowserKey` type), not 3.5b/c/d — can run in parallel
with both lanes above.

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

## Files read this session to produce this scoping (for the next agent's context)

`component_id.rs`, `app_struct.rs` (grep only), `types_browse.rs`,
`types_library_tab.rs`, `input_browse_dispatch.rs` (full), `input_lib_keys.rs`
(grep), `library_load_actions.rs` (grep), `library_search_actions.rs` (grep),
render-side file list under `src/app/render/**` (grep for `nav_stack`
occurrences only — none of these files' bodies were read).
