# Neutrality audit — `unify-surface-colour-neutral`

Recorded at HEAD `5022e9a3`, branch `refactor/unify-surface-colour-neutral`,
2026-09-11, against base ref `origin/main`. Every claim below is
reproducible from the tree: by running the named script or test, or by the
cited file and line.

## Gate battery (task 5.1)

| Gate | Command | Result |
| --- | --- | --- |
| Test suite | `cargo nextest run -p mbv` | 1573 run, 1573 passed, 0 failed |
| Check | `cargo check -p mbv` | PASS, no warnings |
| Lint | `cargo clippy --workspace --all-targets` | PASS, 0 warnings |
| Format | `cargo fmt --all -- --check` | PASS |
| Guardrail scan | `ast-grep scan` | 0 hits (exit 0) |
| Guardrail rules | `ast-grep test` | 15 passed, 0 failed |

The known load-sensitive SIGABRT flake did not fire in these runs (see
residuals).

## D5 proof 2 — test-diff emptiness

`git diff origin/main --` over every pre-existing test file is empty: 166 files
at `origin/main` whose paths contain `test`, all byte-identical in this branch.
Mechanically checked by proof 2 of
`scripts/check-surface-colour-neutrality.sh`, which passes:

```text
surface-colour-neutrality: ok: 166 pre-existing test files unchanged
```

Main's buffer expectations therefore pass byte-identical, pinning today's
values and today's focus reactivity. The only test additions are new modules
and files (the table pins, `tests_surface_conformance.rs`, and the new
fixtures), never edits to an existing test.

## D5 proof 1 — Rgb multiset identity

The multiset of raw `Color::Rgb(...)` literals across `src/` matches
`origin/main` exactly, except for the five declared split literals of task 4.2,
each appearing exactly one more time than the base and only where the base
already carries the same bytes under another name (two names, identical bytes).
Mechanically checked by proof 1 of
`scripts/check-surface-colour-neutrality.sh`, which passes
(`ok: Rgb literal multiset matches origin/main (plus declared splits)`).

The five splits, each verified as base 1 / worktree 2 / delta +1
(`theme/primitives.rs`):

- `Rgb(72, 88, 78)` — new `SOFT_CONTENT_BODY_BG` (:24) beside the scrollbar
  track's `BG_GREEN_SOFT` (:31).
- `Rgb(60, 72, 65)` — new `SURFACE_FOCUSED_BG` (:43) beside the text-role
  green `BG_GREEN` (:47).
- `Rgb(63, 63, 63)` — new `ARTWORK_LOADING_PLACEHOLDER` (:14) beside the
  unfocused-border `OVERLAY` (:15).
- `Rgb(53, 167, 124)` — new `PLAYBACK_THROBBER_FG` (:33) beside the emby
  green `AQUA` (:35).
- `Rgb(58, 148, 197)` — new `PILL_SELECTOR_SELECTED_BG` (:38) beside the
  project blue `FOAM` (:37).

Each split is purpose-named so a future edit to one primitive can never move
another role's appearance while the two share a value today. No literal moved,
was deleted, or was added outside this list — any such move fails the script.

## D5 proof 3 — conformance coverage

`src/app/render/tests_surface_conformance.rs` renders representative frames
through the real shell paint path and asserts, per region, that the painted
background equals `palette::surface_colors(surface, site_bit).fill` for the
surface the table assigns that region — 13 test functions, all passing
(`cargo nextest run -p mbv surface_conformance`: 13 run, 13 passed). The
coverage table in that file's header (`tests_surface_conformance.rs:23-65`)
names every one of the table's 35 identities: **32 are pinned** to the
production resolver at buffer level — by this test where it says "yes", by the
named pre-existing test file otherwise. The table itself (`theme/
surface_table.rs`, 35 `Row` arms) is additionally pinned by its own unit test
(`surface_table.rs:454 resting_values_are_default_or_declared`) and the
ast-grep guardrails (0 scan hits).

### Unpinned surfaces — 3 residuals, with reasons

Three identities have no buffer-observable rect to pin and are recorded rather
than hidden (see `tests_surface_conformance.rs:67-84`):

1. `QueueCardVisualizer` (`surface_table.rs:224`) — resolves the content-body
   pair with the queue column's bit, byte-identical to the containing queue
   column's fill in both bool states, so no painted cell can be attributed to
   this identity. Guarded by the guardrails and the table's own unit tests.
2. `StatusBarPill` (`surface_table.rs:280`) — its pill spans sit on the status
   band and carry the band's own `SURFACE_CHROME` value in both states,
   indistinguishable from the `StatusBar` band's fill.
3. `PopupDimBackdrop` (`surface_table.rs:362`) — paints no fill of its own; it
   blends every existing cell halfway toward black, and its row value is the
   named `Color::Black` blend base, which no cell's background ever equals.

## Residuals

- **The three unpinned surfaces above** — no rendered-fill assertion is
  possible for any of them; each reason is recorded in the conformance test's
  coverage header, and each painter is guarded by the ast-grep rules
  (`ast-grep scan`: 0 hits) and the table's own unit test.
- **The known SIGABRT flake** — pre-existing and load-sensitive; fires only
  under parallel full-suite runs and passes in isolation. It is recorded in
  this change's `design.md` (Risks, "The known load-sensitive SIGABRT flake")
  as inherited from the archived `unify-surface-colour` change's residuals. It
  did not fire in the gate battery recorded here.
- **The bool seam's provenance** — the resolver's bool is each call site's own
  focus input (`theme/surface_resolve.rs`, module doc): a mounted component's
  focus, a pane's `held` bit, a panel focus bool, or a cursor predicate. The
  table governs colour *identity*, not the bit's provenance, and the guardrails
  police names, not bits (design D1). The cursor-driven sites keep their
  predicates verbatim, as migrated — `tv_wide.rs:254`
  (`ctx.focused && ctx.episode_cursor.is_some()`); every other site hands the
  bool main already computed, e.g. the queue card's own panel focus
  (`card.rs:251`, matching main's `resolve_surface_focus(
  matches!(effective_panel_focus(), Queue))`), so no site's reactivity
  changed.
- **Test-fed re-exports** — task 4.2 retired the value-aliased role names and
  the two-arm resolver, but a frozen pre-existing test file pins each of the
  following names and the neutrality rule forbids editing those files
  (`palette.rs`, the `#[cfg(test)]` re-export block). Each survives only as a
  named re-export for exactly the tests that name it:
  - `resolve_surface_focus` — the retired two-arm resolver; still called by
    name by `tests_home_characterization.rs`, `tests_feeds.rs`,
    `tests_audiobookshelf_podcasts.rs` and
    `shell_tv_workspace_selection_tests.rs`.
  - `SURFACE_PLAYBACK` — `tests.rs` asserts the playback panel's focused
    surface by this name.
  - `SURFACE_ARTWORK_PLACEHOLDER` —
    `components/artwork_placeholder_tests.rs` asserts the no-artwork fill by
    this name.
  - `SURFACE_ACCENT_SOFT` — `tests_queue.rs`, `components/tv_wide_tests.rs`
    and `components/music_workspace_cursor_tests.rs` assert the accent-soft
    surface by this name.
  - `SURFACE_BACKDROP` — `tests_home_characterization.rs`,
    `tests_library_characterization.rs`, `tests_feeds.rs` and
    `tests_scroll_pills.rs` assert the backdrop fill by this name.
  - `SURFACE_CHROME` — `tests.rs`, `tests_queue.rs`,
    `queue_title_characterization_tests.rs` and
    `tests_surface_conformance.rs` assert the status/tab band value by this
    name.
  - `SURFACE_FOCUSED` — `tests.rs`, `tests_queue_regression.rs`,
    `tests_wide_hero_pane_characterization.rs` and
    `components/music_workspace_cursor_tests.rs` assert the focused level by
    this name.
  - `SURFACE_RESTING` — `tests_feeds.rs`, `tests_music_wide.rs`,
    `tests_wide_hero_pane_characterization.rs` and
    `components/tv_wide_tests.rs` assert the resting level by this name.
  - `PILL_BG` — `queue_title_characterization_tests.rs` asserts the queue pill
    background by this name.
  - `PILL_ROW_BG` — `tests_scroll_pills.rs` and `test_helpers.rs` assert the
    pill row background by this name.
  - `PILL_SELECTED_BG` — `tests_home_characterization.rs` and
    `test_helpers.rs` assert the selected pill background by this name.
