Task: 5.3b (first pass) — make `FeedsComponent` authoritative for Feeds
interaction state.

Repo: `/home/slatkin/Dev/mbv/.worktrees/migrate-tui-to-tuirealm`. Work only
there; do not `cd` to the main checkout.

Read `openspec/changes/migrate-tui-to-tuirealm/design.md` decision **D14**
(mirror-first conversion) before editing.

## Two live defects this removes structurally

Do not patch these separately. They disappear when the work below is done
correctly; confirm at the end that they do.

**(a) Keyboard cursor movement is discarded every tick.**
`FeedsComponent::handle_key` (`src/app/components/feeds.rs:202-246`) moves
`self.cursor` and returns `Msg::Legacy(LegacyTerminalEvent::NoOp)`, so
`App::handle_key` never runs for Feeds movement and `App.feed_tab.cursor`
never changes. `Model::sync_feeds` (`src/app/shell_feeds.rs:13-31`) then passes
`state.cursor` / `state.scroll` into `set_content` unconditionally, with no
mirror guard, every tick (`src/app/shell.rs:674`).

**(b) Enter can play the wrong entry.**
The component cycles `watched_filter` and `selected_group` locally; App's
copies never change. `ShellRequest::FeedsPlay(cursor)` /
`FeedsEnqueue(cursor)` (`src/app/shell.rs:554-566`) write that cursor into
`App.feed_tab.cursor`, and `feed_tab_play_selected`
(`src/app/feed_tab_actions.rs:257-261`) indexes it into **App's**
`visible_entries()`. A different filter or group is a different index space.

## Delete

From `FeedTabState` (`src/app/types_feed_tab.rs`), these five fields and the
methods that exist only to serve them:

    cursor, scroll, selected_group, watched_filter, filtered_entries

    visible_entries(), rebuild_filtered_entries(), cycle_watched_filter(),
    clamp_state(), group_count()

Keep `rebuild_all_entries()`, minus its `rebuild_filtered_entries()` call. The
`WatchedFilter` enum **stays** — `FeedsComponent` already imports and owns it.

From `src/app/feed_tab_actions.rs`, the cursor/group movement functions, which
`FeedsComponent` already reimplements:

    feed_tab_move_cursor, feed_tab_move_cursor_rows, feed_tab_jump_cursor,
    feed_tab_page_cursor, feed_tab_cycle_group, feed_tab_select_group

From `src/app/input_feed_tab_keys.rs`, the arms in `handle_feed_tab_key` for
movement, group cycling, watched-filter, Enter and `e`. **Preserve the
catch-all consumption**: the function's contract is that it consumes every
other key so Emby-only actions and queue-item handling stay unreachable while
Feeds is focused. Do not weaken that.

`App::render_feeds` (`src/app/render/components/feeds.rs:433`) — it is called
only from `src/app/render/tests_feeds.rs`, never in production.

## Change

`ShellRequest::FeedsPlay` and `FeedsEnqueue` currently carry a cursor index.
Make them carry the entry's `guid: String` instead. `FeedEntry.guid` already
exists and is unique per entry. Rewrite `feed_tab_play_selected` and
`feed_tab_enqueue_selected` to take that guid and resolve the entry by
searching `all_entries`, rather than indexing a filtered view. This is what
makes defect (b) unrepresentable: there is no second index space left to drift.

`Model::sync_feeds` must stop passing `watched_filter`, `selected_group`,
`cursor` and `scroll` into `set_content`. It pushes data and shell status only:
`subscriptions`, `entries`, `all_entries`, `loading`, `focused`.

Move Feeds mouse hit-testing out of `src/app/input_mouse.rs:430-500` and into
`FeedsComponent::handle_mouse`, which currently just forwards the raw event.
The component owns `self.layout`, so it already has the geometry; the group
selector, entry rows, and scroll handling all move with it. Keyboard and mouse
must land on the same cursor.

## Keep, untouched

`FeedTabState.subscriptions`, `entries`, `all_entries`, `loading`,
`pending_results`, `refresh_rx`, `refresh_tx`, and `rebuild_all_entries`.
`App::drain_feed_tab_results`, `refresh_feeds`, and `start_feed_fetch`. The
refresh `mpsc` channel and its result validation are shell authority and are
not part of this change.

Also leave alone: `src/app/library_load_actions.rs` (Home's Feeds section),
`src/app/feeds_manage_actions.rs`'s post-subscription reset, and
`src/app/render/components/feeds.rs`'s `render_feeds_content` /
`FeedsRenderModel` seam. If deleting the five fields forces a mechanical edit
in one of those files, make the minimal edit that compiles — but do not
restructure them.

## Tests

Retarget, do not delete. `src/app/render/tests_feeds.rs` currently drives
`App::render_feeds`; point those cases at `FeedsComponent` instead, preserving
what each one asserts. The same applies to the affected cases in
`src/app/tests_feed_tab_guard.rs`, `src/app/tests_home_latest.rs`,
`src/app/tests_feeds_manage.rs`, and
`src/app/render/tests_conformance_matrix.rs`.

Add one component test that would fail under defect (a): drive a `Down` key
through `FeedsComponent`, call `set_content` with an unchanged shell snapshot,
and assert the cursor is still where the key put it.

Add one that would fail under defect (b): set a watched filter that hides the
first entry, move the cursor, take the emitted `FeedsPlay` guid, and assert it
names the entry the component is actually pointing at.

**Report every test you remove and why it is obsolete rather than
retargetable.** A test asserting behavior you deleted is a signal to check
whether the behavior should have been deleted — not a cleanup target.

## Verify

    rtk cargo nextest run -p mbv
    rtk cargo clippy --workspace --all-targets
    rtk ast-grep scan            # baseline is 69 diagnostics; no new file flagged
    rtk make check-code-file-lines

Report the test count before and after, and the exact list of items reported
never-used.

## Rules

One commit. Never push. Scope every `git add` to explicit paths, never `-A`.
`CLAUDE.md` and `CLAUDE.md.bak` are unrelated user edits — leave them modified
and uncommitted. Do not edit `tasks.md`.

If a deletion forces you to touch a file this brief does not name, stop and
report it rather than widening scope or weakening a visibility marker.

Implement this yourself. Do not spawn subagents or delegate any part of it.
