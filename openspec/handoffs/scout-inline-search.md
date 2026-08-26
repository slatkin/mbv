# Scout: 5.3d.20 — Inline library Search mirror + raw endpoint (context)

Read-only. Surface already `migrated` (ledger line 65) and per-frame
`sync_inline_search` deleted in commit `f35ed7f6`. Remaining "mirror" =
event-scoped projection; remaining "raw endpoint" = component-owned input.

## Mirror surface (shell → component)
- Deleted `Model::sync_inline_search` (commit `f35ed7f6`). No live refs.
- Replacement: `Model::push_inline_search_content()` — `src/app/shell_inline_search.rs:46-94`.
  Syncs `pool` (SearchPool: recursive `Albums` from `app.album_indexes`, plain
  `Items` from `app.libs[index].nav_stack`), `loading` (recursive
  `AlbumIndexState::Loading`), `focused` (`effective_panel_focus()==Library`).
- Call sites (event-scoped): `open_inline_search` (96-143),
  `activate_inline_search_item` (152-185), `handle_inline_search_lib_event`
  (208-219, on Refreshed|AllItemsPrefetched|AlbumIndexBuilt|NavigateTo),
  `apply_inline_search_items` (222-258, w/ parent_id guard), Resize
  (`shell.rs:542-545`), RestoreLibraryPosition (`shell.rs:316-318`).
- Stale-mount release: dismiss when tab != EmbyLibrary or component id mismatches.

## Raw endpoint (component input)
- `InlineSearchComponent::on()` `src/app/components/inline_search.rs:208-218`
  → `handle_key` (116-152): ALT|CTRL swallow→NoOp; Up/Down/Home/End move
  cursor; Enter→`InlineSearchActivate{id,item_type}`; Esc→`InlineSearchDismiss`;
  Char/Backspace mutate `query`, reset cursor/scroll; mouse Left-Down→cursor row.
- Open trigger: `BrowserComponent::handle_crossterm_key` `/` →
  `ShellRequest::OpenInlineSearch` (`browser.rs:90-92`).
- No `input_inline_search_keys.rs` / `handle_key_inline_search` — removed.

## Component state (owned)
`inline_search.rs:58-66`: query (owned), pool (synced), loading (synced),
cursor (owned), scroll (owned; recomputed in view()), focused (synced),
layout (Default per view). `filtered_items` SkimMatcherV2 (chars<2→empty);
`selected_item` = filtered.get(cursor).

## Shell adapter
`shell_inline_search.rs`: `inline_search_component_id` (8-17),
`inline_search_area` (19-31, render-seam rect not stored),
`push_inline_search_content` (46-94), `open` (96-143), `dismiss` (146-150),
`activate_inline_search_item` (152-185), `set_inline_search_loading` (187-205),
`handle_inline_search_lib_event` (208-219), `apply_inline_search_items`
(222-258), `render_inline_search_component` (256-267 → application.view).
Model field `inline_search_id: Option<ComponentId>` (`shell.rs:60/104`) — only
App-level inline-search state; consumed at `shell_library.rs:41` for mount-id
precedence over `Browser`.

## Smallest units (teardown)
U1 drop `inline_search_id` field + `inline_search_component_id` +
shell_library.rs:41 branch. U2 merge redundant re-pushes (184/218). U3 drop
apply_inline_search_items (group 5). U4 drop recursive pool branch (group 5).
U5 re-home `/` trigger. U6 fix mouse `left_area` Default-layout quirk.

## Risks
- `scroll` written inside view() (render side-effect) — preserve on teardown.
- Render seam: list.rs:17 + list_rows.rs:290 `with_search` (search header/loading).
- Mouse `left_area` vs real area mismatch (U6).
- Already migrated; this targets residual shell scaffolding + group-5 App teardown.
