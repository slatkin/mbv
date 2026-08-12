## Context

See `proposal.md` — Why. This document resolves the choices the proposal left open
and records two findings from reading the pre-modal code that change what "restore"
has to mean.

Relevant current state:

- `src/app/app_struct.rs:198-204` — `search_modal: Option<SearchModal>`,
  `search_modal_prior_focus: Option<PanelFocus>`, `last_slash_at: Option<Instant>`,
  and the already-existing `search_debounce_deadline` / `search_debounce_pending` pair.
- `src/app/input_resolver.rs` — `CONTEXT_STACK`, an ordered array of
  `ContextEntry { name, handler }`. Each handler returns `Some(quit)` to consume a
  key or `None` to fall through. `search_modal` sits between `queue_column_width`
  and `panel_mode_cycle_x`; `view_dispatch` is last.
- `src/app/render/mod.rs:198` —
  `power_panel_area = (layout.main.panel_area.width > 0).then_some(layout.main.panel_area)`,
  then one `if self.show_* { self.render_*_panel(f, power_panel_area) }` per panel.
- `src/app/render/chrome.rs:105` / `:148` — `render_panel_shell` (fixed width against
  `f.area()`, used when the slot is collapsed) and `render_panel_shell_at` (into a
  given `Rect`, used when the slot exists). Both return the content `Rect`.
- `src/app/render/list.rs:328-364` — `show_grouped` and `use_letter_groups`, the two
  gates that pick between the grouped-album renderer, the letter-grouped renderer,
  and the plain column-aware renderer.
- `PanelFocus` (`src/app/types_settings.rs:2`) has exactly two variants, `Queue` and
  `Library`, and is matched on in many places.

Two findings from the git history that the proposal does not mention:

1. **The last inline-search code was already broken for navigation.** At `1c608c7`,
   `handle_key_power_lib_search` returned `Some(false)` for *every* key once search
   was open, and `handle_lib_search_key` matched only `Esc`, `Backspace`, and
   `Char(c)` — arrows, page keys, Home/End and Enter fell into `_ => {}` and were
   swallowed with no effect. The original implementation at `ce1daa5` did handle all
   of them; the `c95b0f1` input refactor dropped them. A literal revert of `03a725b`
   would restore a search box you cannot navigate or activate from, contradicting the
   proposal. See Decision 5.
2. **One requirement in the search-modal spec is not about search.** See Decision 7.

## Goals / Non-Goals

**Goals:**

- Land both features without a third search state object surviving anywhere.
- Keep the sidebar's input precedence consistent with the F1–F4 panels rather than
  inventing a new focus mechanism.
- Reuse the debounce, response-channel, and navigation plumbing the modal already
  built, so the diff is a re-surfacing rather than a rewrite.

**Non-Goals:**

- Mouse support for either surface. Neither the old inline search nor the modal had
  click targets for results; adding them is out of scope.
- Any change to `spawn_navigate_to_item`, `spawn_search_items_load`, the album-index
  build, or the Emby search call.
- Persisting sidebar state across sessions.

## Decisions

### 1. `LibSearch` goes back into `src/app/types_browse.rs`

The proposal offers `types_browse.rs` or `types_library_tab.rs`. Choose
`types_browse.rs`, its original home.

`LibSearch.results` is a `Vec<usize>` of indices, and when recursive album search is
active those indices point into `AlbumIndexState::Ready(entries)` — a
`Vec<AlbumSearchEntry>`. `AlbumSearchEntry` is declared in `types_browse.rs`
immediately below where `LibSearch` used to sit, and `update_lib_search` scores
against its `search_text` field. Two types sharing an index space belong in one file;
splitting them across files makes the coupling invisible at both sites.

`types_library_tab.rs` (77 lines) holds only `LibraryTab` itself, which merely *owns*
an `Option<LibSearch>` field. Ownership is a weaker tie than a shared index space, and
putting it there would mean `types_library_tab.rs` importing `AlbumSearchEntry` from
`types_browse.rs` purely to document the relationship in a comment.

Restoring to the original file also makes the revert mechanical:
`git show 1c608c7:src/app/types_browse.rs` can be diffed directly against the current
file. Both files stay far under the 800-line cap either way (163 and 77 lines today).

### 2. Sidebar state: `search_sidebar: Option<SearchSidebar>` in `src/app/search_sidebar.rs`

Rename `src/app/search_modal.rs` to `src/app/search_sidebar.rs` and cut it down
rather than writing a new file. `SearchModal` becomes `SearchSidebar` with:

```
query: String
results: Vec<MediaItem>
cursor: usize
scroll: usize
loading: bool
type_filter: usize          // 0 = All; 1..=n indexes available_types()
last_drain_error: Option<String>
```

Dropped from the modal struct: `mode: SearchMode` (the enum goes entirely) and
`corpus: Vec<MediaItem>` (the sidebar never scores locally — that is now
`LibSearch.items`). `score_corpus_against` and its `fuzzy_matcher` imports go with the
corpus; the sidebar's only matcher is the server's.

Kept verbatim, because they are correct and already tested: `is_navigable_type`,
`available_types`, `filtered_results`, `filtered_count`, `type_sort_key`, and
`apply_drain`'s stale-response guard (`if query != self.query { return }`).
`apply_drain` loses its `if !matches!(self.mode, Global) { return }` line.
`on_query_changed` loses its whole fuzzy branch and becomes a plain reset.

`SearchModalDrainOutcome` is renamed `SearchDrainOutcome`; the channel it drains
(`search_tx`/`search_rx`, typed `(String, Result<Vec<MediaItem>, String>)`) is unchanged.

`app_struct.rs` loses `search_modal`, `search_modal_prior_focus`, and `last_slash_at`,
and gains one field: `search_sidebar: Option<SearchSidebar>`. `last_slash_at` has no
other consumer once the promotion gesture is gone — check with `rtk grep` before
deleting.

**Alternative rejected:** a `show_search_sidebar: bool` plus loose fields on `App`,
matching `show_help` / `show_playlists`. Those panels have one or two pieces of state;
this one has seven, and `Option<SearchSidebar>` makes "closed" and "has a query"
unrepresentable together.

### 3. Debounce: reuse the existing `Instant` deadline on the run loop, no timer

The plumbing already exists and is generic — nothing about it is modal-specific:

- `App::search_debounce_deadline: Option<Instant>` and
  `search_debounce_pending: Option<String>` (`app_struct.rs:202-203`)
- `App::maybe_flush_search_debounce()` (`run_loop_drains.rs:84`), which compares
  `Instant::now()` against the deadline, takes the pending query, and calls
  `spawn_search_modal_query`
- called every frame from `src/app/mod.rs:345` and folded into `had_events`

Keep all of it. Move `const SEARCH_DEBOUNCE_MS: u64 = 300` from
`input_search_modal_keys.rs` into the new `input_search_sidebar_keys.rs`, rename
`dispatch_search_modal_query_if_global` to `dispatch_search_sidebar_query` and drop
its mode check (keeping the `query.len() < 2` gate), and rename
`spawn_search_modal_query` to `spawn_search_sidebar_query`.

No `tokio::time::interval`, no spawned timer thread, no new channel. The TUI already
has a frame loop with an event-driven wake; adding a second timing source for a 300 ms
debounce would be strictly more machinery for the same behavior. `Instant`-vs-now
comparison on a loop that already runs is also trivially testable without a clock.

**Consequence to accept:** the flush fires on the next loop iteration after the
deadline, not exactly at it. With a 300 ms window this is invisible, and it is already
the shipped behavior.

### 4. Focus locking: `CONTEXT_STACK` precedence, not a new `PanelFocus` variant

`PanelFocus` stays a two-variant enum. Do not add a `SearchSidebar` variant and do not
reintroduce `search_modal_prior_focus`.

The F1–F4 panels already implement exactly the lock the proposal asks for, without
touching `panel_focus` at all: their handler sits earlier in `CONTEXT_STACK` than
`view_dispatch`, returns `None` when the panel is closed, and returns `Some(false)`
for every key when it is open (`handle_key_help`, `input_settings_keys.rs:113`, is the
clearest example — it consumes unmatched keys explicitly). The view beneath keeps
rendering and keeps its state; it simply never sees a key.

`handle_key_search_sidebar` follows that shape. Because it never mutates
`panel_focus`, "restore the prior focus on dismiss" is satisfied by construction and
`search_modal_prior_focus` deletes cleanly.

Adding a third `PanelFocus` variant would force every `matches!(self.panel_focus, ...)`
site in the codebase to grow an arm for a state that is not a panel, to express
something the context stack already expresses.

**Placement in `CONTEXT_STACK`:** replace the existing `search_modal` entry in place
(between `queue_column_width` and `panel_mode_cycle_x`) with
`{ name: "search_sidebar", handler: App::handle_key_search_sidebar }`. That position
is already proven for a search surface that swallows everything, and it keeps the
`Ctrl+L`, `F5`, confirm-prompt and daemon-modal contexts above it, which is correct —
those must stay reachable.

**Opening the sidebar:** `Ctrl+/` is handled in
`App::handle_key_global_overlay_open` (`src/app/input.rs:88`), alongside F1–F4, since
that entry already owns "open a panel". Note it sits *above* `search_sidebar` in the
stack, so it sees the chord first even while the sidebar is open — guard the re-press
case there: `if self.search_sidebar.is_some() { return Some(false) }` before opening,
so a second press is a no-op rather than a state reset.

**`/` while the inline search box is open** keeps its old shape: a `lib_search`
context entry above `view_dispatch` (see Decision 5).

### 5. The inline handler must own its navigation keys

Restore `handle_lib_search_key` to the `ce1daa5` key set, not the `1c608c7` one:

| Key | Action |
| --- | --- |
| `Esc` | `self.libs[lib_idx].search = None` |
| `Backspace` | pop a char, or close when the query is empty |
| `Up` / `Down` | `self.move_lib_cursor(-1)` / `(1)` |
| `PageUp` / `PageDown` | `self.move_lib_cursor(∓ self.lib_page_size() as i64)` |
| `Home` / `End` | `self.jump_lib_cursor(false)` / `(true)` |
| `Enter` | `self.select()` |
| `Char(c)` | push to query, then `self.update_lib_search(lib_idx)` |
| anything else | swallowed |

Use `move_lib_cursor` (raw index step), not `move_lib_cursor_rows` — results always
render single-column-aware through the plain renderer, and `move_lib_cursor` already
routes to the search cursor when `libs[lib_idx].search.is_some()` (see
`lib_cursor_actions.rs`, which guards its grouped/feed/letter branches on
`search.is_none()`). Likewise `current_lib_item` already resolves through
`search.results[search.cursor]` first, so `select()` activates the right item with no
further change.

`Tab`/`BackTab` continue to fall through (`return None`) so tab cycling still works, as
at `1c608c7`. The `Enter`-with-series-selection fallthrough at `1c608c7` should also be
kept — it exists so series-detail Enter is not shadowed.

### 6. The `show_grouped` fix

`src/app/render/list.rs:328-332` today:

```rust
let show_grouped = if self.library_tab > 0 {
    self.is_viewing_album_folders(self.library_tab - 1)
} else {
    false
};
```

The bug this reintroduces: `render_power_grouped_album_rows` (`render/album.rs:26`)
reads its catalog from `nav_stack.last().music_grouping.settled`, a
`GroupedAlbumCatalog` whose `order` indexes the **unfiltered** nav-level item vector.
Under search, `items` is the **filtered** vector. The catalog then reorders and labels
rows by positions that no longer refer to the same albums.

The guard to add is `self.libs[lib_idx].search.is_none()`, the same condition
`use_letter_groups` carries:

```rust
let show_grouped = self.library_tab > 0 && {
    let lib_idx = self.library_tab - 1;
    self.is_viewing_album_folders(lib_idx) && self.libs[lib_idx].search.is_none()
};
```

And restore the matching clause on `use_letter_groups` (line 358-364), which at
`1c608c7` read `... && self.libs[lib_idx].search.is_none()` inside its final block.

With both false during search, results fall through to the plain column-aware
renderer, which is what `current_library_columns` already assumes — its first branch
returns the pane-derived column count when `search.is_some()`, ahead of the
album-folders and music-group checks. The three sites then agree.

**Regression bar:** the "Searching a grouped music library" scenario in
`specs/inline-library-search/spec.md` is the modal's scenario carried over unchanged.

### 7. The dimming requirement is relocated, not deleted — a scope deviation

`openspec/specs/search-modal/spec.md` holds "Dimmed backdrops render images in
halfblocks", which states in its own text that it "SHALL apply to every overlay that
dims its backdrop, not only to the search modal". Five other overlays rely on it and
none of them change here.

The proposal's Capabilities section lists only three capabilities and marks
`search-modal` as removed wholesale. Following that literally would delete a spec for
behavior that stays in the code, and leaving it behind would leave a capability folder
named `search-modal` describing image protocols.

This design therefore adds a fourth, unlisted spec file:
`specs/dimmed-backdrop-images/spec.md`, carrying the requirement verbatim, with the
`search-modal` delta removing it and pointing there. **No code changes** — this is a
spec-organisation move only. Flagged here because it exceeds the proposal's stated
capability list; if rejected, the alternative is to drop the requirement and accept
that the halfblock-on-dim behavior becomes unspecified.

### 8. `panel-mode` needs no delta

`openspec/specs/panel-mode/spec.md` specifies the `x` three-state layout cycle, that
focus follows the mode, and that resize keys deactivate outside `both`. It never
enumerates which panels may occupy the queue column, and it does not describe F1–F4
panel focus behavior — that lives in `CONTEXT_STACK` and is unspecified today.

The sidebar changes nothing panel-mode asserts: it does not touch `panel_focus`
(Decision 4), does not alter the `x` cycle, and handles the collapsed-slot case with
the same `power_panel_area` / fixed-width fallback the other panels use. No delta.

### 9. `Ctrl+/` chord detection

Terminals do not agree on `Ctrl+/`. Most send the ASCII unit-separator, which crossterm
surfaces as `KeyCode::Char('_')` (or `'/'`) with `KeyModifiers::CONTROL` depending on
the terminal and whether the kitty keyboard protocol is active. Match all of:

- `KeyCode::Char('/')` with `CONTROL`
- `KeyCode::Char('_')` with `CONTROL`

The spec requires this ("whether the terminal reports the chord as the search
character with the control modifier or as the control code that terminals substitute
for it"). Verify in the user's terminal during implementation; if neither arrives,
that is a finding to report, not a workaround to invent.

### 10. File sizes

Against the 800-line cap:

| File | Now | After | Note |
| --- | --- | --- | --- |
| `render/list.rs` | 501 | ~545 | input box (~40) + search empty-state (~10) |
| `input_lib_power_keys.rs` | 402 | ~465 | `/` handler + `handle_lib_search_key`, minus `handle_search_key` |
| `search_sidebar.rs` | 686 (as `search_modal.rs`) | ~450 | corpus, mode, fuzzy scoring and their tests removed |
| `render/search_sidebar.rs` | — (694 as the modal renderer) | ~300 target | no hero block, no dim, no image path, no two-row meta |
| `input_search_sidebar_keys.rs` | 220 (as modal keys) | ~150 | promotion gesture removed |
| `render/mod.rs` | 676 | ~676 | one call swapped for one call |

Only `render/search_sidebar.rs` carries real risk. If it lands over 800, split the
chip row and the result-row builder into `render/search_sidebar_rows.rs` — do not
compress to fit.

## Risks / Trade-offs

- **A literal `git revert` of `03a725b` reintroduces the dead-navigation bug and the
  `show_grouped` bug.** → Decisions 5 and 6 name both fixes explicitly, and the task
  list places them as their own steps rather than folding them into "restore the old
  file". Do not cherry-pick the revert.
- **`Ctrl+/` may not be deliverable in the user's terminal.** → Decision 9 matches both
  encodings. If neither arrives, report it before substituting a different chord —
  silently rebinding would make the spec wrong.
- **The sidebar swallows every key, including `q`.** → This matches the modal's current
  behavior and is what "focus locks to the sidebar" means, but it does mean quit is
  unreachable without dismissing first. The spec states it. `Ctrl+C` and the
  daemon-lost / confirm contexts sit above `search_sidebar` in `CONTEXT_STACK` and stay
  reachable.
- **Deleting `SearchModal` touches the tests that reference it.** `search_modal.rs`
  carries its own `#[cfg(test)]` block, and `input_resolver_handle_key_tests.rs`,
  `tests.rs` (the `App` literal at line ~248) and `construct.rs` all name the removed
  fields. → The task list makes the struct-literal fixups an explicit step; `cargo
  check` will enumerate them.
- **`update_lib_search` re-scores the entire corpus on every keystroke, synchronously.**
  This is the pre-existing behavior and is fine at library scale, but a very large
  album index will make typing feel heavy. → Not addressed here; the proposal's
  Non-goals exclude changing how the corpus is built. Note it if it shows up in use.
