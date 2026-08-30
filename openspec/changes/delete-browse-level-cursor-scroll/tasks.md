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
- [ ] 1.2 Delete `BrowseLevel::cursor` and `BrowseLevel::scroll`. In the same
      commit, re-spell the remaining outcome-2 resting reads that only stop
      compiling at deletion — mechanical `.cursor` -> `.resting().cursor()`,
      `.scroll` -> `.resting().scroll()`:
      - `context_menu_actions.rs:43` / `:81` / `:455` — `.map(|l| l.cursor)`
        feeding `current_lib_item(lib_idx, cursor)` on the legacy context-menu
        action path (design §1.1 R1: "the App nav-level cursor on the legacy
        context-menu/mouse paths"; the menu blocks browse movement while open, so
        resting == live and no characterization test asserts otherwise).
      - `shell_browser.rs:293` / `:297` — `push_emby_browser_content` seeding the
        mounted `BrowserComponent` via `set_content` (content projection at a
        nav/tab event; `BrowserComponent::set_content` re-adopts this value as
        its own cursor, `components/browser.rs:97-108`).
      - `shell_tv_workspace.rs:193` / `:197` — `push_tv_workspace_content` seeding
        `TvWorkspaceComponent` (same shape; the one-shot `reanchor` already
        overrides on breakpoint hand-off).
      Verify: `rtk cargo check -p mbv` is clean with no transitional accessor
      left behind (D5 — a field is removed in the same task that re-points its
      last reader; no accessor returning the old value survives). Re-back
      `BrowseResting` as a real `BrowseLevel` field (or equivalent) so
      `resting()` / `set_resting_*` / `from_position_level` keep compiling; the
      resting accessor stays — it is the sanctioned resting-position path, not a
      transitional live accessor (D5).
- [ ] 1.3 Verify: `rtk cargo nextest run -p mbv`. The restore characterization
      tests from `split-browse-state-interaction-fields` tasks 2.1/3.1/4.1 and
      `migrate-narrow-browse-to-components` 2.1 are the behavioural gate.

## 2. Retire what the deletion makes unreachable

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
