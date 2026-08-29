## 1. Delete the fields

- [ ] 1.1 Confirm the precondition: every browse surface has an owning
      component and `library_list_render_ctx` takes cursor/scroll as
      parameters (`migrate-narrow-browse-to-components` task 2). Verify: no
      non-test reader of `BrowseLevel::cursor`/`scroll` remains outside
      `types_browse.rs` itself. Stop and report if any does — the inventory in
      `split-browse-state-interaction-fields/design.md` §1.1/1.1b is the
      checklist.
- [ ] 1.2 Delete `BrowseLevel::cursor` and `BrowseLevel::scroll`. Verify:
      `rtk cargo check -p mbv` is clean with no transitional accessor left
      behind (D5 — a field is removed in the same task that re-points its last
      reader; no accessor returning the old value survives).
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
