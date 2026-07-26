
## 368 — Split render/list.rs, album.rs, home.rs, detail.rs — 2026-07-25

Resolved by user ruling (2026-07-25):
- [x] Lane A3: **decompose**, cutting along list-kind seams rather than line counts. Verified against the code: `render_power_list` dispatches on `show_grouped` / `use_letter_groups`, a 3-way mutually exclusive set (grouped albums / letter-grouped / plain). Kind 1 is already a delegate; kinds 2 and 3 match its contract (`pub(super) fn` on `App`, sibling file, returns `usize`). Carrier narrowed from 11 fields to 8 after measuring per-branch usage. Own revertible commit after A1/A2.
- [x] `action.rs` / `input_resolver.rs`: **deferred**, dropped from #368. Survey preserved in the plan's §11 for a follow-up issue — not filed by the planner.

- [x] Executor context budget: mechanical span-cut protocol replaces "paste verbatim" (plan §1a); executors read a symbol overview, not the file (§1b); Lane A split into two agent runs, A-mech and A3 (§2); five standalone ~2k briefs replace handing over the whole plan (§12). Re-derived budget in §7a: every run now ~32–44k typical vs. 100–160k for lanes A/B before.

- [x] Lane D naming: **decided** — `detail_series.rs` (measure) + `detail_series_view.rs` (render).
- [x] Lane A new-function naming: **decided** — `render_power_letter_grouped_rows` / `render_power_plain_rows`.
- [x] `album.rs` residual at ~520 with `render_power_grouped_album_rows` (~508) intact: **accepted**.

Critic pass returned REVISE; design verified sound, four accuracy fixes applied (2026-07-25):
- [x] Visibility bumps were undercounted two → **~20**, across all four lanes. Per-lane inventory added; §5's review gate rewritten (as written it would have rejected a *correct* Lane B diff). No bump crosses a lane boundary, so parallelism is unaffected.
- [x] A3's byte-identity check was unsatisfiable — `final_offset` is assigned mid-body (L651/L923), not returned at the end. Two in-body edits now permitted and excluded from the diff.
- [x] Carrier was missing `ungrouped_total` (kind-2-only, L569); executor must now re-derive the free-variable set rather than trust the table.
- [x] Span hints orphaned preceding doc comments/attributes in 8 places, two of which break the build. Mandatory "extend start upward" step added to §1a.
- [x] §7a recalibrated: ~38–58k typical (was ~32–44k); "3× reduction" → ~2×. B and A3 heaviest at ~58k, still well under the 125k threshold.

**Nothing blocking — all five runs dispatchable.**

Still open (non-blocking):
- [ ] Promote the §1a/§1b extraction protocol into `docs/agents/` after this issue validates it? Every future "split a large file" issue wants it.
- [ ] Contingency only: if B or A3 approach budget (~80k pessimistic), split by output file — B into (`album_plan` + `album_cursor`) then (`album_art` + `album_detail`); A3 into one run per list kind.
- [ ] Repo-wide: flat siblings vs directory modules. This issue adds ~15 files to `src/app/render/` (~28 total there); the flat convention is straining but should be decided repo-wide, not per-issue.
- [ ] File the deferred `action.rs` / `input_resolver.rs` follow-up issue (both are pure test-extraction: production code ends at 536 / 237 lines).
