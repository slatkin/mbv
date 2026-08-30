## 1. Delete the fields

- [ ] 1.1 Confirm the precondition: every browse surface has an owning
      component and `library_list_render_ctx` takes cursor/scroll as
      parameters (`migrate-narrow-browse-to-components` task 2). Verify: no
      non-test reader of `BrowseLevel::cursor`/`scroll` remains outside
      `types_browse.rs` itself. Stop and report if any does — the inventory in
      `split-browse-state-interaction-fields/design.md` §1.1/1.1b is the
      checklist. (1.1a–1.1c re-point the three site-groups the field-split
      campaign left live; confirm they are landed before 1.2.)
- [x] 1.1a Re-point `actions_navigation.rs:92` (`select_item`, `lvl.cursor = pos`
      after resolving a playable item). Per design §1.1 **outcome 1**: drop the
      write — the mounted component owns the live cursor and the caller already
      holds the resolved item/index. Verify: `rtk cargo check -p mbv`;
      `select_item` restore/characterization tests pass.
- [x] 1.1b Re-point `context_menu_actions.rs:206` and `:345` (post-removal
      `lvl.cursor = …min(len-1)` re-clamp). Per design §1.1: drop both writes —
      the component re-clamps its own cursor against the projected content
      (projection reset). Verify: `rtk cargo check -p mbv`; context-menu
      item-removal tests pass.
- [ ] 1.1c Re-point the Music wide render path off `BrowseLevel` per design
      §1.1b **R16/R17/R18 (outcome 3)**: `wide_music_render_ctx`
      (`render/components/music_wide.rs` ~`:153/:157/:164`) and
      `selected_album_item` (`render/components/widgets.rs:612`) source the live
      album cursor/scroll and selected item from `MusicWorkspaceComponent`
      (`selected_item()` at `music_workspace.rs:142`, plus `album_cursor()` /
      `album_scroll()`), threading the value as a shell-side parameter where the
      ctx builder can't reach the component (Group A — wide surface, component
      always mounted, per D6/D7). Verify: `rtk cargo check -p mbv`,
      `rtk cargo nextest run -p mbv` (music characterization tests).
- [ ] 1.2 Delete `BrowseLevel::cursor` and `BrowseLevel::scroll`. Verify:
      `rtk cargo check -p mbv` is clean with no transitional accessor left
      behind (D5 — a field is removed in the same task that re-points its last
      reader; no accessor returning the old value survives). Re-back
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
