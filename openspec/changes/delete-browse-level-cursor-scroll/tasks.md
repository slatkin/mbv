## 1. Delete the fields

- [x] 1.1 Confirm the precondition: every browse surface has an owning
      component and `library_list_render_ctx` takes cursor/scroll as
      parameters (`migrate-narrow-browse-to-components` task 2). Verify: no
      non-test reader of `BrowseLevel::cursor`/`scroll` remains outside
      `types_browse.rs` itself. Stop and report if any does — the inventory in
      `split-browse-state-interaction-fields/design.md` §1.1/1.1b is the
      checklist. (1.1a through 1.1g re-point or retire the nine live non-test
      site-groups the field-split *and* `migrate-narrow-browse-to-components`
      campaigns left reading the raw fields; confirm all are landed before 1.2.
      Authoritative re-grep at HEAD `e05ba291` for raw `BrowseLevel` field access
      — excluding `.resting().cursor()`/`.scroll()`, `set_resting_*`,
      `from_position_level`/`to_position_level`, and `#[cfg(test)]` — finds reads
      at `context_menu_actions.rs:43,81,455`, `shell_browser.rs:293,297,380`,
      `shell_tv_workspace.rs:193,197`,
      `render/components/detail.rs:111,112,148,149,374,375`,
      `render/components/tv_wide.rs:105,109`, `shuffle_folder_actions.rs:24`; a
      write at `shell_tv_workspace.rs:149`; and the section-2 mover/mouse writes
      (`lib_cursor_actions.rs:218,233,263,298,311`,
      `mouse_gestures.rs:122,229,241`). Confirmed clean: `actions.rs`
      `current_lib_item` (R1) and `library_search_actions.rs`
      `maybe_fetch_next_page` (R7) already take `cursor` as a parameter;
      `bootstrap.rs:67` `state.cursor` is a queue-restore field, not
      `BrowseLevel`; `feed_actions.rs` R3 now logs
      `feed_home_video.video_cursor`. §1.1b's per-frame `level.scroll`
      write-back in the `render_list` painters is gone — the per-kind bodies
      (`list_plain.rs`, `list_letter_groups.rs`, `list_narrow.rs`) now *return*
      `final_offset` up the stack instead of storing it on `BrowseLevel`
      (`migrate-narrow-browse-to-components`) — so no task covers it.)
- [x] 1.1a Re-point `actions_navigation.rs:92` (`select_item`, `lvl.cursor = pos`
      after resolving a playable item). **Correction (design §1.1's "outcome 1 —
      drop" classification is wrong here):** `shell_browser` test
      `shell_emby_browser_effects_honor_component_target` (task 5.3d) asserts the
      activation effect lands the App cursor on the supplied item
      (`nav_stack[0].cursor == 1`, not the parked 0). This write is the
      **outcome-2 resting-position update at the activation event**, not a mirror.
      Keep it; spell it `lvl.set_resting_cursor(pos)` so it survives 1.2 as a
      `BrowseResting` write. Verify: `rtk cargo check -p mbv`;
      `rtk cargo nextest run -p mbv --no-fail-fast` — no failures beyond the two
      known-pre-existing (`browser_local_navigation_mirrors_legacy_flat_movement`,
      and — once this is fixed — nothing else).
- [x] 1.1b Re-point `context_menu_actions.rs:206` and `:345` (post-removal
      `lvl.cursor = …min(len-1)` re-clamp). Per design §1.1: drop both writes —
      the component re-clamps its own cursor against the projected content
      (projection reset). Verify: `rtk cargo check -p mbv`; context-menu
      item-removal tests pass. (Landed in `4b26a7ec`; full `--no-fail-fast`
      suite shows no regression attributable to this drop.)
- [x] 1.1c Re-point the Music wide render path off `BrowseLevel` per design
      §1.1b **R16/R17/R18 (outcome 3)**: `wide_music_render_ctx`
      (`render/components/music_wide.rs` ~`:153/:157/:164`) and
      `selected_album_item` (`render/components/widgets.rs:612`) source the live
      album cursor/scroll and selected item from `MusicWorkspaceComponent`
      (`selected_item()` at `music_workspace.rs:142`, plus `album_cursor()` /
      `album_scroll()`), threading the value as a shell-side parameter where the
      ctx builder can't reach the component (Group A — wide surface, component
      always mounted, per D6/D7). Verify: `rtk cargo check -p mbv`,
      `rtk cargo nextest run -p mbv` (music characterization tests).
- [x] 1.1d Re-point the narrow inline-hero item resolvers `selected_movie_item`
      and `selected_series_item` (`render/components/detail.rs:107` / `:144`),
      which build a `LibraryListRenderCtx` from
      `nav_stack.last().map_or(0, |l| l.cursor / .scroll)` (`:111,112` and
      `:148,149`). **Live — design §1.1b R19/R20, Group B**: both are reached
      every frame through `narrow_browse_extras`
      (`render/components/list_narrow.rs:585` / `:595`) from
      `render_emby_browser_component` (`shell_browser.rs:359`), which composes the
      narrow generic / Movies / home-video / TV browse inline hero while the
      mounted `BrowserComponent` owns the live cursor — a resting read here lags
      the painted selection mid-scroll. **D2 outcome 3**: give both resolvers a
      `cursor: usize` parameter (scroll is irrelevant to item resolution — pass
      `0`) and thread `BrowserComponent::cursor()` from the `shell_browser.rs:359`
      seam through `narrow_browse_extras(lib_idx, cursor)`. Pitfall: read
      `browser.cursor()` before the `get_component_mut(id)` mutable borrow *and*
      before `narrow_extras` is built (currently at `shell_browser.rs:353`, ahead
      of the component borrow) — reorder so the cursor is in hand first. The
      mouse-only caller `activate_selected_series` (`input_browse_dispatch.rs:12`,
      D16 accepted-broken) passes
      `self.libs[lib_idx].nav_stack.last().map_or(0, |l| l.resting().cursor())`.
      `selected_*_item_with_ctx`, `render_compact_detail_with_ctx` and the
      transitive `detail.rs` consumers already take the ctx — no change.
      Verify: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv
      --no-fail-fast` — baseline is exactly ONE known pre-existing failure
      `browser_local_navigation_mirrors_legacy_flat_movement`.
- [x] 1.1e Drop the dead `BrowseLevel` fallback in `wide_tv_render_ctx`
      (`render/components/tv_wide.rs:104-110`): the `cursor_scroll: Option<(usize,
      usize)>` parameter's `None` arm reads
      `nav_stack.last().map_or(0, |l| l.cursor / .scroll)` (`:105` / `:109`).
      **Dead branch** — the sole live caller `render_library`
      (`render/components/widgets.rs:589`) is fed `cursor_scroll` by
      `render_main` / `compose_base_frame` (`render/screens/root.rs:210,323`),
      which `Model::draw_frame` (`shell_run.rs:63-79`) always resolves to
      `Some((component.cursor(), component.scroll()))` for an active Emby library
      now that `migrate-narrow-browse-to-components` mounts a component at every
      Emby breakpoint (narrow TV and podcast included). The only `None` callers
      are tests and the `compose_base_frame(f, None)` helpers. **D2 outcome 3**:
      replace both `map_or_else` fallbacks with `0` — matching
      `wide_music_render_ctx`, whose `None` arm already resolves to `0` rather
      than a field read — leaving the `Option` signature untouched (zero
      test-call churn). Verify: `rtk cargo check -p mbv`; `rtk cargo nextest run
      -p mbv --no-fail-fast` (baseline as 1.1d).
- [x] 1.1f Two single-line re-points the deletion forces:
      - **C** `shell_browser.rs:380` (`render_emby_browser_component` poster
        prefetch): the ctx is built with `browser.cursor()` for cursor but
        `nav_stack.last().map_or(0, |level| level.scroll)` for scroll. Replace
        the scroll read with `browser.scroll()` (accessor exists,
        `components/browser.rs:123`). **D2 outcome 3.**
      - **D** `shell_tv_workspace.rs:149` (`hand_off_tv_breakpoint`, wide→narrow):
        `level.cursor = cursor` is a raw write beside
        `level.set_resting_scroll(scroll)`. Spell it
        `level.set_resting_cursor(cursor)`. **D2 outcome 2** — resting write at
        the breakpoint hand-off event, the same shape 1.1a landed for
        `select_item`.
      Verify: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv
      --no-fail-fast` (baseline as 1.1d).
- [x] 1.1g Retire three dead `BrowseLevel::cursor` readers that
      `migrate-narrow-browse-to-components` orphaned but did not delete; the
      field deletion forces the choice and re-pointing dead code is pointless
      (D5), so they land here rather than in section 2:
      - `App::shuffle_play` (`shuffle_folder_actions.rs:8`,
        `.and_then(|lvl| lvl.items.get(lvl.cursor))` at `:24` — design §1.1 R13).
        No live caller: the live shuffle path is `shuffle_play_selected` →
        `shuffle_play_target` (`shell_browser.rs:57`). Only refs are tests
        (`tests_library_position_refresh.rs:125`, `tests_feed_tab_guard.rs:76`).
        Delete `shuffle_play`; retarget those two tests to `shuffle_play_target`
        (its own bounds-miss no-op guard, `shuffle_folder_actions.rs:55`,
        preserves what they assert).
      - `App::render_compact_detail` (`render/components/detail.rs:358`,
        raw ctx build at `:374,375`) and its only caller
        `App::render_selected_home_video_detail`
        (`render/components/home_video.rs:161`). Both have zero live callers
        (grep at HEAD: `render_selected_home_video_detail` is unreferenced; the
        narrow home-video hero is composed by `render_compact_detail_with_ctx`
        from `list_narrow.rs:226,456`). Delete both;
        `render_compact_detail_with_ctx` stays.
      Verify: `rtk cargo clippy --workspace --all-targets` reports no new dead
      code; `rtk cargo nextest run -p mbv --no-fail-fast` (baseline as 1.1d).
- [x] 1.2a Delete `BrowseLevel::cursor` and `BrowseLevel::scroll` from the type
      and land every **production** consequence in one commit, so the crate
      compiles (`rtk cargo check -p mbv`) with no transitional accessor left
      behind. Tests do not gate this row — `BrowseLevel`'s fields are
      `pub(super)`, so the test corpus is a sibling module and
      `cargo check --tests` stays red until 1.2b; that red is expected and is
      1.2b's scope, not a reason to defer the type change.
      - **Re-back the type** (`types_browse.rs:39-57` / `:59-76`): drop the two
        raw fields, add `resting: BrowseResting` as a real field
        (`#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]` already covers
        it), and update `resting()` (`:121`) to return the stored value, the two
        `set_resting_*` setters (`:128`, `:132`) to write it, and add
        `BrowseResting::new(cursor: usize, scroll: usize) -> Self` — its two
        fields are private to `types_browse`, so without a constructor every
        construction site outside that module (all 15 production literals and
        every test literal) fails to compile. `pub(super)` is sufficient: the
        module sits under `crate::app`, so the constructor is visible
        throughout `src/app/**` (matching today's `pub(super) resting()`); do
        NOT widen anything to `pub(crate)`. All construction sites then spell
        `resting: BrowseResting::new(C, S)`.
        `from_position_level` (`:80-112`) to build `resting: BrowseResting {
        cursor, scroll }` from its local `cursor` +
        `scroll_for_cursor(cursor, visible_rows)`, and
        `to_position_level` (`:135`) unchanged — it already goes through
        `resting()`. `BrowseResting`'s own doc comment says that re-back is
        pending, so update the wording rather than delete the type.
      - **16 production construction literals** — replace
        `cursor: C, scroll: S` with `resting: BrowseResting { cursor: C, scroll:
        S }` (13 are `0`/`0`, so the shape never varies):
        `actions_navigation.rs:54`, `browse_level_actions.rs:90`,
        `feed_actions.rs:345`, `feed_actions.rs:430`,
        `library_browse_actions.rs:136`, `:171`, `:362`, `:475` (non-zero
        `cursor`, position of the target id, `unwrap_or(0)`),
        `library_load_actions.rs:293` (a field-by-field copy that already reads
        `lvl.resting().cursor() / .scroll()`; it can collapse to
        `resting: lvl.resting()`), `library_position_state.rs:165`,
        `library_search_actions.rs:137` (non-zero `cursor`, position `unwrap()`),
        `music_actions.rs:71`, `:120`, `:263`, `:333`,
        `shell_library.rs:238` (an inline `#[cfg(test)]` site, already
        `resting()`-sourced), plus `types_browse.rs:99` itself.
        `crates/` builds no `BrowseLevel`.
      - **7 outcome-2 resting reads**, mechanical `.cursor` ->
        `.resting().cursor()`, `.scroll` -> `.resting().scroll()`:
        `context_menu_actions.rs:43` / `:81` / `:455` — `.map(|l| l.cursor)`
        feeding `current_lib_item(lib_idx, cursor)` on the legacy context-menu
        action path (design §1.1 R1: "the App nav-level cursor on the legacy
        context-menu/mouse paths"; the menu blocks browse movement while open, so
        resting == live and no characterization test asserts otherwise);
        `shell_browser.rs:293` / `:297` — `push_emby_browser_content` seeding the
        mounted `BrowserComponent` via `set_content` (content projection at a
        nav/tab event; `BrowserComponent::set_content` re-adopts this value as
        its own cursor, `components/browser.rs:97-108`);
        `shell_tv_workspace.rs:193` / `:197` — `push_tv_workspace_content` seeding
        `TvWorkspaceComponent` (same shape; the one-shot `reanchor` already
        overrides on breakpoint hand-off).
      - **Transitional re-spellings of the writes section 2 deletes** — the same
        spelling 1.1a and 1.1f used, and nothing more: do NOT delete or
        restructure any of these functions here, rows 2.1-2.3 delete them (with
        the reads at `lib_cursor_actions.rs:137`, `:206`, `:232` that feed them).
        Five writes in `lib_cursor_actions.rs` (`:218`, `:233`, `:263`, `:298`,
        `:311`) and three in `mouse_gestures.rs` (`:122`, `:229`, `:241`)
        — `level.cursor = v` -> `level.set_resting_cursor(v)`; plus
        `lib_cursor_actions.rs:137` (`last()?.cursor` ->
        `.and_then(|l| l.resting())` then `.cursor()`, the variant 1.1a/1.1f did
        not cover).
      The 11 writes above are re-spelled, not repaired: after this row the delta
      movers still write resting state they no longer own, which is exactly the
      unreachable code section 2 removes.
      Verify: `rtk cargo check -p mbv` clean (D5 — the field is removed in the
      same task that re-points its last production reader; no accessor returning
      the old value survives, and the resting accessor stays because it is the
      sanctioned resting-position path, not a transitional live accessor).
- [x] 1.2b Mechanical test-side migration only — no production change, no
      design question. `BrowseLevel` is still constructed in 91 test literals
      across 30 files, and ~120 raw `.cursor` / `.scroll` touches sit outside
      those literals (`shell_tv_workspace_tests.rs:535`,
      `shell_browser_tests.rs:240`, `render/components/list_tests.rs:94`,
      `render/tests_library_characterization.rs:64`,
      `render/components/list_late_tests.rs:24`,
      `render/tests_music_groups.rs:25`,
      `render/tests_music_characterization.rs:25`,
      `render/tests_conformance_matrix.rs:89` construct levels some other way).
      The 1.2a commit may land with `cargo check --tests` red; **this row closes
      it and nothing may land between the two.**
      - **Pattern, spelled once**: inside a literal,
        `cursor: C, scroll: S` -> `resting: BrowseResting::new(C, S)` (the
        1.2a constructor; `BrowseResting`'s fields stay private — a read
        becomes `.resting().cursor()`, a write `set_resting_*`). Do **not**
        widen
        `BrowseResting`'s fields to `pub(super)` to keep the literal shape short —
        that re-exposes exactly the pair this change deletes.
      - **Add one fixture ctor, in the module that already owns the test
        `BrowseLevel` literals**: `render/test_helpers_fixtures.rs` (5 literals),
        `input_music_track_test_support.rs` (4),
        `split_browse_state_browse_level_tests.rs`'s `movie_level` (3) already
        build full literals; add a `#[cfg(test)]`
        `browse_level(parent_id, title, items, cursor, scroll) -> BrowseLevel`
        beside them and route every literal that differs only in those five
        values through it, rather than hand-editing 91 sites to the new shape.
        Where a literal sets `letter_filter`, `all_items`, `item_types`, or
        `music_grouping`, use the ctor plus field assignment (there is no
        `..Default::default()` for `BrowseLevel`, and adding a
        `Default`/builder for the tests' convenience would be a production
        abstraction the ladder does not authorise).
      - **The three `split_browse_state_browse_level_tests.rs` assertions** keep
        their meaning verbatim — the file is the behaviour gate for this whole
        change, so map, do not weaken:
        `:60` `assert_eq!(level.cursor, 2)` -> `level.resting().cursor()`
        (from_position_level resolves `focused_item_id` to the resting cursor,
        and `:62`'s `to_position_level` round-trip is unchanged);
        `:83` `nav_stack[0].cursor == 1` -> `nav_stack[0].resting().cursor()`
        (`go_back`'s parent re-anchor is already
        `actions_navigation.rs:239` / `:273` `set_resting_cursor(idx)`);
        `:96` / `:104` read and `:103` writes raw `.cursor` to drive the
        `maybe_fetch_next_page(lib_idx, cursor)` threshold ->
        `.resting().cursor()` and `set_resting_cursor(5)`; the parameter is
        already explicit (§1.1 R7), so the *semantics under test here are the
        arithmetic*, not where the cursor lives. `:75`'s `..child` functional
        update is the only one in the tree and stays valid in either shape.
      - **No assertion may be deleted, relaxed, or `let _ =`-discarded to reach
        green.** If a test fails once it compiles, that is a real regression from
        1.2a — stop and fix 1.2a, not the test.
      Verify: `rtk cargo check -p mbv --all-targets` clean, `rtk cargo nextest
      run -p mbv --no-fail-fast` at the pre-existing baseline of exactly
      `browser_local_navigation_mirrors_legacy_flat_movement`
      (`components/browser_component_tests.rs`) — the one failure the accepted
      1.1a/1.1d/1.1e/1.1f rows name, and the only exception to "no failures"
      here; `rtk cargo fmt`, `rtk cargo clippy --workspace --all-targets`, and
      `rtk make check-code-file-lines` (this row touches ~30 files, several of
      them already near the 800-line ceiling).
- [x] 1.3 Verify: `rtk cargo nextest run -p mbv`. The restore characterization
      tests from `split-browse-state-interaction-fields` tasks 2.1/3.1/4.1 and
      `migrate-narrow-browse-to-components` 2.1 are the behavioural gate.

## 2. Retire what the deletion makes unreachable

The 11 mover/mouse writes named below were re-spelled to `set_resting_*` by 1.2a
so production would compile; rows 2.1-2.3 delete the functions containing them.
Their line references predate 1.2a and that re-spelling shifts none of them.

- [ ] 2.1 Delete `App::apply_lib_cursor_index` (`lib_cursor_actions.rs:241`)
      and route `ShellRequest::BrowserCursorIndex` to the resting-position
      writer and effect tail directly. Verify: `rtk cargo check -p mbv`.
- [ ] 2.2 Delete `App::move_lib_cursor_rows` and `App::jump_lib_cursor` — both
      already have no live caller — and `App::move_lib_cursor`, whose only
      non-test caller is `mouse_gestures.rs:83`. Delete the mouse call sites
      with them; do not repair mouse behaviour (D16). Verify:
      `rtk cargo clippy --workspace --all-targets` reports no dead code.
- [ ] 2.3 Re-check `mouse_gestures.rs` for remaining writes to the deleted
      fields (`:122`, `:219`, `:231`) and delete those paths. Verify:
      `rtk cargo check -p mbv`.

## 3. Retire the conventions the types now enforce

- [ ] 3.1 Review `rules/interactive-component-boundary/` and remove only the
      clauses the types now make unrepresentable; keep every clause still
      guarding a real boundary. Verify: `rtk ast-grep test` fixtures pass and
      `rtk ast-grep scan` is clean.
- [ ] 3.2 Delete the warning comments that documented the old rule (for example
      `input_browse_dispatch.rs:22`, `context_menu_actions.rs:305`), since the
      thing they warned about no longer compiles. Verify: `rtk grep -n
      "mirror" src/app/` returns only historical references in archived docs.

## 4. Close out

- [ ] 4.1 Split any file pushed over 800 lines. Verify:
      `rtk make check-code-file-lines`.
- [ ] 4.2 Full gate: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt`, `rtk make check-code-file-lines`.
- [ ] 4.3 Confirm #607's acceptance criterion "component-local interaction
      state has one owner" holds literally: no `App` field stores a live
      cursor, scroll, or selection for a mounted component. Verify: stated
      against `split-browse-state-interaction-fields/design.md` §1.1/1.1b/1.2,
      every row resolved. Recording this across the ledger, ADR 0022, and the
      spec is `sync-interactive-surface-docs` (#614).
