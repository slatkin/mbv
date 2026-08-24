# Task 5.3b — Feeds teardown: make `FeedsComponent` authoritative

Repo: `/home/slatkin/Dev/mbv/.worktrees/migrate-tui-to-tuirealm`. Work only
there; never `cd` to the main checkout.

Read `openspec/changes/migrate-tui-to-tuirealm/design.md` decision **D14**
before editing. Implement this yourself. Do not spawn subagents or delegate any
part of it.

## Goal

Move Feeds interaction state off `App` and onto `FeedsComponent`, which already
owns a duplicate of it. Five `FeedTabState` fields are being deleted:

    cursor, scroll, selected_group, watched_filter, filtered_entries

Everything else in `FeedTabState` is shell authority and **stays**:
`subscriptions`, `entries`, `all_entries`, `loading`, `pending_results`,
`refresh_rx`, `refresh_tx`. The refresh `mpsc`, its result validation, and
Home's Feeds-pill projection are not part of this change.

## Two defects this closes

Both are live today. They are symptoms of the split ownership, not separate
bugs — do not patch them independently. Confirm at the end that they are gone.

**(a) Keyboard cursor movement is discarded every tick.**
`FeedsComponent::handle_key` (`src/app/components/feeds.rs:186-262`) moves
`self.cursor` and returns `Msg::Legacy(LegacyTerminalEvent::NoOp)`, so
`App::handle_key` never runs for Feeds movement and `App.feed_tab.cursor` never
changes. `Model::sync_feeds` (`src/app/shell_feeds.rs:12-30`) then pushes
`state.cursor` / `state.scroll` / `state.selected_group` /
`state.watched_filter` into `set_content` unconditionally, every tick
(`src/app/shell.rs:674`), overwriting what the key just did.

**(b) Enter/`e` can act on the wrong entry.**
The component cycles `watched_filter` and `selected_group` locally; App's copies
never change. `ShellRequest::FeedsPlay(cursor)` / `FeedsEnqueue(cursor)`
(`src/app/shell.rs:554-571`) write that index into `App.feed_tab.cursor`, and
`feed_tab_play_selected` (`src/app/feed_tab_actions.rs:256`) indexes it into
**App's** `visible_entries()`. A different filter or group is a different index
space.

## Facts you must not re-derive wrong

Two things about the current code are easy to get backwards. Both are verified.

**1. `App::render_feeds` is live, not test-only.**
`src/app/render/components/feeds.rs:433` is reached in production:
`shell.rs:725 self.app.render(f)` → `render/screens/root.rs:521 render_library`
→ `render/components/widgets.rs:522 TabSelection::Feeds => self.render_feeds(..)`.
The shell then paints `render_feeds_component` over the top.

Critically, that legacy call is the **only** writer of
`app.layout.main.feeds_area` (set inside `render_feeds_content`, at
`render/components/feeds.rs:53`). `Model::render_feeds_component`
(`shell_feeds.rs:36`) reads `feeds_area` and early-returns when it is zero.
`FeedsComponent::view` writes into a *local* `LayoutMain`, not `app.layout.main`.

**So: deleting `App::render_feeds` without replacing that one assignment makes
the Feeds tab render nothing at all.** In the `widgets.rs` `TabSelection::Feeds`
arm, replace the `self.render_feeds(...)` call with `layout.feeds_area = area;`
and delete `App::render_feeds`. Keep `render_feeds_content` and
`FeedsRenderModel` exactly as they are — that seam is shared with the component.

**2. The mouse path is compile-forced into this change.**
`src/app/input_mouse.rs:423-495` writes `self.feed_tab.cursor` directly and
reads `self.feed_tab.scroll` / `visible_entries()`. Those fields are being
deleted, so this code cannot be left as-is, and it cannot be repaired in place:
it depends on `app.layout.main.left_item_rows` / `left_row_map` /
`selector_tabs` / `left_area`, which after change (1) above are no longer
populated for Feeds. The geometry now lives in `FeedsComponent::layout`, and
`impl App` cannot reach the component.

Moving Feeds hit-testing into the component is therefore in scope, not
deferrable to 5.3c.

## Commits

Two commits. Each must compile and pass tests standalone.

### Commit 1 — `ShellRequest` carries a guid, not an index

Behaviour-preserving for the unfiltered "All" group; closes defect (b) by
construction. No field is deleted yet.

- `ShellRequest::FeedsPlay(usize)` → `FeedsPlay(String)`; same for
  `FeedsEnqueue`. (`src/app/components/msg.rs:195,198`.) Use the guid, not the
  `FeedEntry` — `Msg` derives `PartialEq` and `FeedEntry`
  (`crates/mbv-core/src/playback_queue_items.rs:137`) does not. Do not add a
  derive to `mbv-core` for this.
- `FeedsComponent` emits `self.visible_entries[self.cursor].guid.clone()`
  (`components/feeds.rs:256-257`), or no `Msg` when the list is empty.
- `feed_tab_play_selected` / `feed_tab_enqueue_selected`
  (`feed_tab_actions.rs:256,280`) take `&str` and resolve via
  `self.feed_tab.all_entries.iter().find(|e| e.guid == guid)`. Every visible
  list is a subset of `all_entries`, so this always resolves. The existing
  `primary_source()` guard, `hydrate_feed_entry_state`, and `submit_queue_item`
  calls are unchanged.
- `shell.rs:554-571` stops writing `App.feed_tab.cursor` and calls the two
  functions directly instead of synthesising a crossterm `Enter`/`e` and
  re-entering `handle_feed_tab_key`.

  `# ponytail:` if a feed ever ships duplicate guids across subscriptions,
  first-match wins. Add feed-scoped identity only if that is observed.

### Commit 2 — delete the five fields

**`src/app/types_feed_tab.rs`** — delete the five fields and the methods that
exist only to serve them: `visible_entries()`, `rebuild_filtered_entries()`,
`cycle_watched_filter()`, `clamp_state()`, `group_count()`. Keep
`rebuild_all_entries()` minus its trailing `rebuild_filtered_entries()` call;
keep `sort_entries_newest_first`. The `WatchedFilter` enum **stays** —
`FeedsComponent` imports and owns it.

**`src/app/components/feeds.rs`** — `set_content` drops the four interaction
parameters. New signature:

    set_content(&mut self, subscriptions, entries, all_entries, loading, focused)

Then the reset-trigger work, which is the part most likely to be got wrong:

> Once the shell stops feeding a mirror-guarded field, the guard silently
> becomes unconditional preservation. Every reset the shell used to perform
> must be re-established component-side, keyed off the event that triggered it.
> This is the exact defect `153c9b97` shipped and `758d0a84` fixed for TV.

`App` currently performs two Feeds resets the component must take over:

- `feeds_manage_actions.rs:328-342` re-clamps `selected_group` and `cursor`
  after any subscription mutation. Replace with the `last_series_id` pattern
  from `components/tv_workspace.rs`: hold the previous subscription URL list on
  the component, and in `set_content`, **before** anything else, compare it to
  the incoming one; on any change reset `selected_group = 0`, `cursor = 0`,
  `scroll = 0`. Note that today's `set_content` clamps the *incoming*
  `selected_group`; after this change it must clamp `self.selected_group`.
- `feed_tab_actions.rs:86` calls `clamp_state()` after a drain. The existing
  `rebuild_visible_entries()` + `clamp_cursor()` at the end of `set_content`
  already covers it — verify, don't add a second clamp.

Add `pub(in crate::app) fn layout(&self) -> &LayoutMain` so tests can assert
geometry.

**`src/app/shell_feeds.rs`** — `sync_feeds` passes data and shell status only.

**`src/app/feed_tab_actions.rs`** — delete `feed_tab_move_cursor`,
`feed_tab_move_cursor_rows`, `feed_tab_row_delta`, `feed_tab_jump_cursor`,
`feed_tab_page_cursor`, `feed_tab_cycle_group`, `feed_tab_select_group`. The
component already reimplements every one.

**`src/app/input_feed_tab_keys.rs`** — delete the movement, group-cycling,
watched-filter, Enter and `e` arms. **Keep `handle_key_feeds`'s catch-all
consumption**: its contract is that it consumes every other key so Emby-only
actions and queue-item handling stay unreachable while Feeds is focused.
`tests_feed_tab_guard.rs` asserts exactly this. Do not weaken it. `F5` /
refresh must keep working.

**Mouse — three sites, all Feeds arms of `match self.tab`:**

- `input_mouse.rs:423-495` — click on selector pill and on entry row/cell.
  Move into `FeedsComponent::handle_mouse`, which today just forwards the raw
  event. It owns `self.layout`, so the geometry is already there. Preserve the
  three-tier resolution the current code implements: `selector_tabs` hit first,
  then two-column `left_item_rows` cell resolution, then `left_row_map`, then
  the flat offset fallback. Return `NoOp` for a consumed click and forward as
  `Msg::Legacy(LegacyTerminalEvent::Mouse(..))` for anything outside its area.
- `input_mouse_dispatch.rs:256` — `TabSelection::Feeds => feed_tab_select_group`
  in the shared selector-tab loop.
- `input_mouse_dispatch.rs:609` — `TabSelection::Feeds => feed_tab_move_cursor`
  (scroll wheel).

Both `match self.tab` arms are exhaustive over `TabSelection`; leave a no-op arm
rather than restructuring the match. Wheel scrolling must still move the Feeds
cursor — handle it in the component. Keyboard and mouse must land on the same
cursor.

## Keep, untouched

`App::drain_feed_tab_results`, `refresh_feeds`, `start_feed_fetch`,
`sync_feed_subscriptions`, `has_feeds_subscriptions`, `feeds_tab_pos`.
`library_load_actions.rs`'s `feeds_latest_section` (Home's Feeds pill reads
`all_entries`, which stays on `App`). `render_feeds_content` /
`FeedsRenderModel`. `feeds_manage_actions.rs` keeps clearing `entries` and
`all_entries` — only its `selected_group` / `cursor` / `clamp_state` /
`rebuild_filtered_entries` lines go.

## Tests

Retarget; do not delete. Report **every** test you remove and why it is obsolete
rather than retargetable. A test asserting behaviour you deleted is a signal to
check whether that behaviour should have been deleted.

- `src/app/render/tests_feeds.rs` (5 tests) drives `App::render_feeds` through
  `test_helpers::render_view`. Point each at `FeedsComponent` — `set_content`,
  then `Component::view` into a `TestBackend`, asserting via the new `layout()`
  accessor. `src/app/components/feeds_component_tests.rs` is the reference for
  the shape. Preserve what each case asserts (wide left-detail split, narrow
  inline detail, short-viewport suppression, buffer characterization, pill-row
  targets).
- `src/app/tests_feed_tab_guard.rs` — the key-consumption guards
  (`feeds_tab_keys_cannot_enter_emby_action_paths`,
  `feeds_tab_does_not_route_into_library_behavior`, the two F5 tests) must keep
  passing against the surviving catch-all. The two
  `feed_tab_play_selected_*` cases retarget to the guid-resolving signature.
- `src/app/tests_home_latest.rs` —
  `feeds_pill_reflects_all_entries_newest_first_independent_of_tab_filter` and
  `home_play_and_enqueue_leave_feeds_tab_state_untouched` assert that Home's
  build does not disturb Feeds selection. That property still matters; assert it
  on the component instead of on `App.feed_tab`.
- `src/app/tests_feeds_manage.rs::post_mutation_clears_entries_and_clamps_group_and_cursor`
  is the reset-trigger test. It must survive as a **component** test proving
  the subscription-change reset fires.
- `src/app/render/tests_conformance_matrix.rs` uses `rebuild_all_entries` only —
  expect a mechanical fix at most.
- The `types_feed_tab.rs` `mod tests` cases covering deleted methods
  (`visible_entries_*`, `clamp_state_works`, `group_count_includes_all`, the
  `watched_filter_*` / `filter_*` / `group_change_*` set) test behaviour that
  now lives in `FeedsComponent`. Move them there; do not drop the coverage. The
  sort tests (`all_group_sorted_*`, `subscription_groups_are_sorted_*`) and
  `hydration_merges_by_guid_and_ignores_unknown` stay put.

Add two new component tests:

- Fails under defect (a): send `Down`, then call `set_content` with an unchanged
  shell snapshot, and assert the cursor is still where the key put it.
- Fails under defect (b): set a watched filter that hides the first entry, move
  the cursor, and assert the emitted `FeedsPlay` guid names the entry the
  component is pointing at.

## Verify

    rtk cargo nextest run -p mbv
    rtk cargo clippy --workspace --all-targets
    rtk ast-grep scan
    rtk make check-code-file-lines

Take your own `nextest` and `ast-grep` baselines on a clean tree **before**
editing, and report before/after for both. Report the exact list of items
reported never-used, not a count. Manually confirm defects (a) and (b) are gone.

## Rules

Two commits, as scoped above. Never push. Scope every `git add` to explicit
paths, never `-A` — another agent may be working concurrently. `CLAUDE.md` and
`CLAUDE.md.bak` are unrelated user edits: leave them modified and uncommitted.
Do not edit `tasks.md`.

Keep unrelated `App` state and unrelated `handle_key_*` handlers. Delete only
render methods and handler branches that *this* change makes uncallable.

If a deletion forces you to touch a file this brief does not name, stop and
report it rather than widening scope or weakening a visibility marker.
