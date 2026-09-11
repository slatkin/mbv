## 1. The table, locked to main's behaviour

- [ ] 1.1 Port the three theme modules from the archived branch's 8dd8fa97
  (`surface.rs` / `surface_table.rs` / `surface_resolve.rs`), reshaping the
  resolver to `surface_colors(surface, focused: bool)` (D1). Re-walk the
  archived 2.1 inventory against main and correct the identity set before
  porting names. Verify: `cargo check -p mbv` with the modules
  `cfg_attr(not(test), allow(dead_code))` as the archived row 4.1 did.
- [ ] 1.2 Derive every row per D2: focused fill = what main's site paints with
  its bit true; resting = with false; fixed sites pin focused == resting.
  Enumerate the special sites (D3) with file:line evidence in the row docs.
  Verify: the pinned-value test plants a change in one row and fails.
- [ ] 1.3 Record the neutrality proofs as runnable checks from the start
  (D5): the Rgb multiset comparison scripted, and `git diff origin/main --`
  over existing test files asserted empty by the verify step of every
  migration task.

## 2. Guardrails

- [ ] 2.1 Port both rules with D4's adjusted text (role names and any
  non-`surface_colors` resolver banned in painters; role backgrounds banned).
  Verify: both probes fire with ids and locations, then revert; `ast-grep
  test` passes.

## 3. Migration units (each site keeps its own bool)

- [ ] 3.1 Unit A — shared painters: `widgets`, `list_rows`,
  `media_list/wide` + `wide_row`, `card`, `artwork_placeholder`,
  `album_art`, `wide_hero`, `wide_hero_boundary`. Verify: buffer
  expectations byte-identical; no test-file edits.
- [ ] 3.2 Unit B — shell chrome: `chrome.rs` backdrops (`queue_focused` stays
  the input), `queue_boundary`, `chrome_status`, `chrome_player`,
  `shell_playback`, `shell_draw`'s strip, `chrome_tabs`. Verify as 3.1.
- [ ] 3.3 Unit C — screens: `home`, `music_wide`, `home_hero_emby`,
  `detail_series_view`, `search_sidebar`, `hero`, `playback`. The hero-pane
  match collapses to the one bool per D3(d). Verify as 3.1.
- [ ] 3.4 Unit C2 — `queue.rs` bands and scope pills; the selected scope pill
  keeps the aqua through its declared row. Verify as 3.1.
- [ ] 3.5 Unit D — `modal_frame` and its nine callers, `backdrop` dim,
  `context_menu`, the sidebar bodies/bands in `chrome.rs`. Verify as 3.1.

## 4. Conformance and retirement

- [ ] 4.1 Port the conformance test to the bool resolver (D5 proof 3);
  coverage table for all 34 identities; the surfaces pinned nowhere at buffer
  level recorded, not hidden. Verify: planting another level's fill in a
  painter fails the test by name.
- [ ] 4.2 Retire value-aliased role names and split shared primitives (the
  archived rows 4.3/5.1 substance, re-based onto main). Verify: Rgb multiset
  matches main with only split-count deltas; grep proves no production role
  name remains outside `theme/`.

## 5. Verify

- [ ] 5.1 Gates: `cargo nextest run -p mbv`, `cargo check -p mbv`,
  `cargo clippy --workspace --all-targets`, `cargo fmt --all -- --check`,
  `ast-grep scan`, `ast-grep test`. No new warnings.
- [ ] 5.2 Neutrality audit (D5): the three proofs run and recorded in the
  report — test-diff emptiness, multiset identity, conformance coverage — plus
  the residuals list (unpinned surfaces, the known SIGABRT flake, the bool
  seam's provenance note).
- [ ] 5.3 Sync the delta requirement into `openspec/specs/ui-design-language/
  spec.md` and archive the change.
