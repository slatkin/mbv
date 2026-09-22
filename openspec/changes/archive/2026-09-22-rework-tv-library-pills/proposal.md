# Proposal

## Why

The TV library top level is still navigated with the movie library's nine
alphabet-range pills (`A-C` … `V-Z`, `#`) plus a size threshold that, above
`LIBRARY_PILL_THRESHOLD`, silently auto-scopes the first load to `A-C`. It is a
disambiguation tool: it helps a user find one letter range, and offers no other
way to look at the library. There is no "newest episodes" or "coming up" view
in the library at all — the user has to go to Home and find the library's
`Latest` pill, then leave Home to browse anything.

TV is the library whose content is naturally two different questions — *which
show* (a flat series list) and *what's new* (a flat episode list) — so the top
level should be a content-mode selector, not only an alphabet split. Home's
per-library `Latest` pill already answers the second question; giving the same
`Latest` mode to the library, together with Emby's `Upcoming` episode feed and
a smaller set of alphabet ranges, turns the pill row into that selector. The
new-content marker must follow the content: a library's `Latest` mode shows the
same dot Home shows for that library, and looking at it once acknowledges it
everywhere.

This is deliberately a presentation-neutral change. TV will later become a
tree (the shared list seam from `extract-shared-list-components` is the first
step of that), but `Latest` and `Upcoming` are flat episode lists and never
nest, and the series modes are the current flat series list. Nothing here
depends on the tree, and the tree change will adopt these pills unchanged.

## What Changes

- The TV top-level pill row becomes a content-mode selector: `Latest` and
  `Upcoming` are always present; alphabet ranges appear only above
  `LIBRARY_PILL_THRESHOLD`, and only when they are absent does an `All` mode
  appear.
  - Above threshold: `Latest | Upcoming | A-I | J-R | S-Z`, default `Latest`.
  - At or below threshold: `Latest | Upcoming | All`, default `All`.
- TV alphabet ranges drop from nine buckets to three: `A-I` (everything
  sorting before `J`, so digits and non-alphabetical names land here under
  their `#` in-list header), `J-R` (`J` through `R`), `S-Z` (`S` onward, with
  no upper bound). A TV-specific bucket set; **movie libraries keep the
  existing nine-pill set unchanged**.
- Add a `Latest` mode: the library's newest episodes, the same content and
  source Home's `Latest` pill uses for that library. It fetches independently
  of Home, so Home need not have loaded.
- Add an `Upcoming` mode backed by the Emby `GET /Shows/Upcoming` route scoped
  to the library.
- `Latest` and `Upcoming` are flat episode lists: Enter plays the episode, and
  they never open a series detail or workspace. A hero appears for the selected
  episode only in mini view.
- The content mode becomes part of the library's sticky navigation position:
  it is saved and restored on reopen, and the existing keyboard cycling
  (`[`/`]`) moves across every mode.
- The library's `Latest` mode carries the same new-content dot Home's `Latest`
  pill for that library carries, and acknowledging it on either surface clears
  it on both. This requires the acknowledgement (today owned inside the Home
  component) to become shell-owned state.

Non-goals: converting TV to a tree (the following change); changing movie,
music, feed, or Audiobookshelf pills; changing the `Latest`/`Upcoming` fetch
semantics beyond reusing Home's existing `Latest` source; adding a hero or
workspace for episodes in non-mini-view geometry.

## Capabilities

### New Capabilities

- `tv-library-content-modes`: The TV library top-level pill row as a
  content-mode selector — which modes exist, in what order, at which library
  size, which one opens by default, how each mode sources and presents its
  rows, how the mode persists and cycles, and how the `Latest` mode carries the
  new-content acknowledgement shared with Home.

### Modified Capabilities

- `tv-letter-filtering`: The alphabet-range pills drop from the movie library's
  nine three-letter buckets to three TV-specific buckets, `#` is folded into
  the first bucket, and eligibility/row composition is owned by
  `tv-library-content-modes`.
- `home-latest-sections`: A library's new-content marker is no longer only a
  Home pill marker — the library's own `Latest` mode reflects it, and the
  acknowledgement is shared between the two surfaces.

## Impact

Affected code:

- `src/app/render/screens/sort_filter.rs` — `LETTER_FILTER_BUCKETS`,
  `LetterFilter` (bucket set becomes kind-aware; TV-specific table).
- `src/app/components/tv_content/mod.rs` and `src/app/shell_tv_workspace.rs` —
  the TV selector row's pills/active/markers and the mode-driven content.
- `src/app/music_actions.rs` — `should_show_letter_pills`,
  `select_letter_pill`, `cycle_letter_pill` (mode-aware).
- `src/app/lib_event_actions.rs` — the TV default/auto-scope path
  (`maybe_capture_library_total_and_apply_default_pill`).
- `src/app/components/emby_library_content.rs` — movie side keeps nine pills.
- `src/app/types_browse.rs` / `src/app/library_load_actions.rs` — the persisted
  browse-level mode and its restore.
- `src/app/components/home_content/mod.rs` and shell-owned state — hoist the
  visited/acknowledged `Latest` sources out of the component.
- `crates/mbv-core/src/api_client_library.rs` — a `get_upcoming` fetch; the
  `Latest` fetch already exists.

Tests: `tv_content` tests, `actions_tests_letter.rs`, the TV tick-integration
suites, `render/tests_non_music.rs`, the Home marker/visitation tests
(`tests_tick_integration_home.rs`, `home_content/mod.rs` tests), and
`render/tests_scroll_pills.rs` for the marker row.
