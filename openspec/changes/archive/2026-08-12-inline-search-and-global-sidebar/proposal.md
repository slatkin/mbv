## Why

The unified search modal is a worse experience than what it replaced. It blocks
the view, forces a promotion gesture (`//`) to switch modes, and wraps results
in heavy hero-block markup. The two search operations it merges — fuzzy filtering
the current library and querying the server — have different scopes, different
data sources, and different usage patterns. Reunifying them into a single overlay
was a net loss.

This change restores the original inline fuzzy search in the library list and
moves global (server-side) search to a sidebar panel, giving each mode a UI
surface that matches how it's actually used.

## What Changes

### Inline fuzzy search (restore)

- `/` in a library tab opens a 3-row bordered input box at the top of the
  library list area, identical to the old `LibSearch` behavior.
- The loaded library items are scored with `SkimMatcherV2` against the query.
  Results replace the library list in-place below the input box.
- Up/Down navigates results; Enter activates the selected item; Esc or
  empty-backspace dismisses the search and restores the normal list.
- A `LibSearch` struct returns to `LibraryTab`, holding query, corpus, scored
  result indices, cursor, scroll, and loading state.
- `/` on the home tab does nothing (home has no library list to filter).

### Global search sidebar (new)

- `Ctrl+/` from any tab opens a global search sidebar panel.
- The sidebar takes over the queue column (same slot as F1–F4 panels) using
  `render_panel_shell` / `render_panel_shell_at`.
- Layout: text input at top, type-filter chips below (All / Movie / Series /
  etc.), then a plain library-style result list — single-row items, no hero
  blocks, no two-row metadata.
- Server queries are debounced, dispatched once the query reaches 2 characters.
- Up/Down navigates results; Tab/Shift-Tab cycles the type filter; Enter
  activates the selected result and dismisses the sidebar; Esc dismisses
  without navigating.
- Focus locks to the sidebar while it's open. The library list is visible but
  not interactive.

### Deleted

- `SearchModal` struct and `SearchMode` enum.
- `src/app/render/overlays/search_modal.rs` (modal renderer).
- `src/app/input_search_modal_keys.rs` (modal key handler).
- The `//` promotion gesture and `last_slash_at` arming state.
- `search_modal_prior_focus` on `App`.
- The `dim_backdrop_active` interaction with the search modal (other modals
  still use it).

## Non-goals

- Changing how the album-index fuzzy corpus is built or loaded.
- Changing the Emby search API call or its response parsing.
- Adding new search features (history, recent items, etc.).

## Capabilities

### New Capabilities

- `global-search-sidebar`: Ctrl+/ opens a sidebar panel for server-side search
  with type filtering and plain result rows.

### Restored Capabilities

- `inline-library-search`: / opens an inline fuzzy filter in the library list,
  restoring pre-modal behavior.

### Removed Capabilities

- `search-modal`: the unified search modal and its fuzzy/global mode switching.

## Impact

- `src/app/types_browse.rs` or `src/app/types_library_tab.rs`: re-add
  `LibSearch` struct.
- `src/app/input_lib_power_keys.rs`: restore `/` key handler for inline search.
- `src/app/render/list.rs`: restore search input box and filtered result
  rendering at the top of the library list.
- `src/app/render/overlays/search_modal.rs`: delete.
- `src/app/input_search_modal_keys.rs`: delete.
- `src/app/search_modal.rs`: delete.
- New file for the global search sidebar renderer (e.g.
  `src/app/render/search_sidebar.rs`).
- New file for sidebar key handling (e.g. `src/app/input_search_sidebar_keys.rs`).
- `src/app/app_struct.rs`: replace `search_modal` field with global search
  sidebar state; add `LibSearch` back to `LibraryTab`.
- `src/app/input_resolver.rs`: route `Ctrl+/` to sidebar open, `/` to inline
  search.
- `src/app/render/mod.rs`: remove search modal overlay call, add sidebar
  rendering in the panel slot.
