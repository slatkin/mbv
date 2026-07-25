# Plan — Issue #368: Split `render/list.rs`, `album.rs`, `home.rs`, `detail.rs`

Status: REVISED after user rulings (see §7) — ready for critic pass, then executor handoff
Base commit surveyed: `9a3e915` (HEAD, branch `main`, clean tree)
Scope: production code only, same discipline as #365 / #367 (move-only refactor, sibling files, minimal visibility bumps, independent review, no self-merge)

**User rulings applied:**
1. **Lane A3 is IN** — decompose `render_power_list`, cutting along **list-kind seams**, not line counts. Analysis in §0 Finding 5 and §3 Lane A.
2. **`action.rs` / `input_resolver.rs` are DEFERRED** — dropped from this plan; survey findings preserved in §11 for a follow-up issue.
3. **Executor context budget is now a first-class constraint** — mechanical extraction protocol (§1a), symbol-overview-not-full-read (§1b), Lane A split into two agent runs (§2), per-run standalone briefs (§12), derived budget table (§7a).

**Reading guide.** §12 is the operational output — five self-contained executor briefs. Everything before it is the reasoning behind them. **Do not hand this document to an executor; hand it one brief from §12.**

---

## 0. Survey results (confirm / refute the issue's hypotheses)

Verified line counts at `9a3e915`:

| File | Issue said | Actual now | Inline `#[cfg(test)]`? |
|---|---:|---:|---|
| `src/app/render/list.rs` | 1,813 | **1,811** | yes — L1152–1811 (**660 lines**) |
| `src/app/render/album.rs` | 1,740 | **1,740** | no (already clean) |
| `src/app/render/home.rs` | 1,423 | **1,423** | yes — L1093–1423 (**331 lines**) |
| `src/app/render/detail.rs` | 1,354 | **1,354** | yes — L1029–1354 (**326 lines**) |

### Finding 1 — three of the four render files never got #365 step 1 (test extraction)
#365 ran as two steps: **step 1** extracted `#[cfg(test)]` modules to sibling files, **step 2** split production code. `list.rs`, `home.rs`, `detail.rs` still carry inline test modules. Pulling those out alone removes 1,317 lines of the problem across the three files, at essentially zero risk. This should be a separate first commit per lane, exactly as #365 step 1 did (`20b3477`, `b31d55c`, `c9e918e`).

### Finding 2 — the issue's "`#[path]` siblings" phrasing is imprecise
The established convention (documented in `docs/plans/367-further-split-mod-actions-input.md` §0 and verified against `73a9096`, `c95b0f1`, `80fdc20`) is:
- **Production siblings**: plain `mod name;` declaration in the parent `mod.rs`. `#[path]` is *not* used.
- **`#[path]`**: used *only* for test files hung off a non-`mod.rs` parent, e.g. `src/app/actions.rs:625`, `src/app/input.rs:219`.

For this issue the parent is `src/app/render/mod.rs`, a real `mod.rs`, so all new production siblings get plain `mod x;` lines there. One consequence for parallelism — see §2.

### Finding 3 — cross-lane collisions are nonexistent, but visibility bumps are *not* (corrected)

Almost everything being moved is an `impl App` method. Rust resolves inherent methods on the type, not by module path, so moving `App::foo` from `render/album.rs` to `render/album_plan.rs` requires **zero call-site edits anywhere** — only that the method's visibility still reaches its callers. Since all new siblings are children of `render`, `pub(super)` keeps meaning "visible within `render`", identical to today.

**An earlier draft of this plan claimed "no visibility widening is required for any moved method" and "exactly two private→`pub(super)` bumps". Both were wrong.** The real count is **~20, spread across all four lanes.** The error came from checking only the symbols with *pre-existing* `pub(super)`/`pub(crate)` annotations — the ones with cross-file callers today — and missing that splitting a file converts many *intra*-file private calls into cross-module calls. Every private symbol that stays behind while its caller moves out (or vice versa) needs widening.

This does **not** affect the parallelism claim: every bump below is on a symbol within its own lane's file, resolved by its own executor. **No bump crosses a lane boundary.** Lanes remain fully independent.

**Per-lane bump inventory** (verified at `9a3e915`; all are private → `pub(super)` unless noted):

**Lane B — ~14 bumps.** `render_power_grouped_album_rows` stays in `album.rs` while much of what it calls moves out:

| Symbol | Line | Why |
|---|---:|---|
| `App::album_artist_label` | 136 | → `album_plan.rs`, called from residual `album.rs` |
| `App::build_grouped_album_display_plan` | 144 | → `album_plan.rs`, called from residual |
| `App::selected_power_music_artist_header` | 464 | → `album_cursor.rs`, called from residual |
| `App::render_inline_album_art` | 1244 | → `album_art.rs`, called from residual |
| `App::render_inline_artist_collage` | 1283 | → `album_art.rs`, called from residual |
| `enum GroupedAlbumDisplayRow` | 86 | → `album_plan.rs`; ~25 references from residual |
| `GroupedAlbumDisplayRow::row_target` | 124 | → `album_plan.rs` |
| `struct GroupedAlbumDisplayPlan` | 104 | → `album_plan.rs` |
| …**and all five of its private fields** | 105–113 | `order`, `rows`, `display_cursor`, `selected_artist_header_valid`, `selected_block_bounds` — every one is read from residual `album.rs` |
| `INLINE_ALBUM_ART_ROWS` | 17 | → `album_art.rs`, read from residual |
| `INLINE_ALBUM_ART_RESERVED` | 20 | → `album_art.rs`, read from residual |

Struct-field widening is the same "move + systematic field-visibility widening" pattern #367 L3 hit; it is expected and appears as visibility-only hunks, not a logic change.

**Also corrected:** of the eight symbols assigned to `album_cursor.rs`, only **six** carry `pub(in crate::app)`. `selected_power_music_artist_header` (464) and `set_artist_header_focus` (477) are **private** — the former needs a bump (above), the latter is called only from within `album_cursor.rs`'s own moved set and does not.

**Lane C — 6+ bumps.** The Keep Watching hero cluster moves to `home_hero.rs` while `render_power_home_list` stays:

| Symbol | Line |
|---|---:|
| `App::keep_watching_hero_image_types` | 519 |
| `App::keep_watching_hero_layout` | 530 |
| `App::render_keep_watching_hero_image` | 577 |
| `App::render_keep_watching_hero_meta` | 623 |
| `struct KeepWatchingHeroLayout` **and its four private fields** (`title_lines`, `show_name`, `overview_lines`, `height`) | 163–168 |
| `power_home_panel_scroll` | 17 |

The struct's fields are destructured by the hero tuple around L870 in residual `home.rs`, so field-level widening is required, not just the type.

**Lane D — 8 bumps**, not one. Beyond `wrap_overview_lines`, everything moving to `detail_series.rs` is read by `render_series_inline_detail` in `detail_series_view.rs`:
`series_meta_line` (72), `SERIES_DETAIL_DIVIDER_ROWS` (58), `SERIES_DETAIL_EPISODE_ROWS_ESTIMATE` (59), `SERIES_DETAIL_TRAILING_BLANK_ROWS` (64), `SERIES_IMAGE_COLS` (65), `SERIES_IMAGE_ROWS` (66), `SERIES_IMAGE_PLACEHOLDER_ROWS` (67).

**Lane A3 — 2 bumps.** `App::render_series_detail_if_visible` (216) and `App::render_series_detail_top_border` (257) are private and stay in `list.rs`, but are called from the moved bodies (the latter at 858 and 1129).

**Lane A-mech — the previously-noted set** (the free helpers moving to `list_rows.rs` all become `pub(super)`).

The only cross-file *path* dependency between the four target files is one free function:

```rust
src/app/render/list.rs:2:  use super::detail::compact_banner_image_cache_key;
```

Handled by a guardrail (§4): that function stays in `detail.rs`.

### Finding 4 — `list.rs` is a different shape of problem from the other three
`album.rs`, `home.rs`, `detail.rs` each decompose along clean pre-existing symbol boundaries. `list.rs` does not: after test extraction it is **1,151 lines of which 860 are a single function**, `render_power_list` (L290–1150). Removing the free helpers only gets it to ~985. Getting `list.rs` under the guideline requires decomposing that function's body — a genuine (if mechanical) code change, not a pure move. Precedent: #365 lane D2 (`d6418df`, "decompose `run()`'s internals into helper methods").

### Finding 5 — the seams in `render_power_list` are genuinely one-branch-per-list-kind (user's hypothesis: **confirmed**)

The user's framing — "there are different kinds of lists" — matches the code exactly. Verified at L427–465 and L538–1137.

**The discriminant** is a pair of booleans computed in the prelude, which by construction form a **3-way mutually exclusive** choice (`use_letter_groups` is defined as `!show_grouped && …`, so the two can never both be true):

```rust
// list.rs:428
let show_grouped = if self.library_tab > 0 {
    self.is_viewing_album_folders(self.library_tab - 1)
} else { false };

// list.rs:458
let use_letter_groups = !show_grouped
    && self.library_tab > 0
    && (ungrouped_total >= 50 || active_letter_filter.is_some())
    && { self.libs[lib_idx].library.collection_type != "music"
         && self.libs[lib_idx].search.is_none() };
```

**The exhaustive set of list kinds is exactly three** — no more, no fewer:

| # | Kind | Condition | Branch | Body size |
|---|---|---|---|---:|
| 1 | **Grouped albums** — albums under artist headers | `library_tab > 0 && is_viewing_album_folders(lib_idx)` | L538–549 | 12 (already a delegate) |
| 2 | **Letter-grouped** — A/B/C headers injected at bucket boundaries | `!grouped && library_tab > 0 && (total ≥ 50 or letter pill active) && collection_type != "music" && not searching` | L550–865 | **316** |
| 3 | **Plain** — flat list, no injected headers | everything else | L866–1137 | **271** |

Kind 3 is the genuine catch-all and absorbs several distinct *situations* that nonetheless render identically: the Home "Continue Watching" tab (`library_tab == 0`), any active search result set, small libraries (< 50 items with no letter pill), and non-album levels of music libraries. That is a real merge, not an oversight — state it in the executor handoff so nobody "helpfully" splits kind 3 further.

**One caveat on exhaustiveness:** there is a fourth path, but it is *not* a list kind — `n == 0` early-returns at L508–533 with a `render_power_placeholder` showing "Indexing music library…" / "Loading…" / "(empty)". It fires **before** the kind dispatch and returns, so it never reaches a branch. Leave it in the prelude; do not model it as a kind.

**Is the grouped branch already the right shape? The contract yes, the parameter list no.**

The kind-1 delegate is:

```rust
// list.rs:538
if show_grouped {
    let lib_idx = self.library_tab - 1;
    final_offset = self.render_power_grouped_album_rows(
        f, content_area, lib_idx, &items, cursor, stored_scroll, focused, layout,
    );
} else if use_letter_groups { /* 316 lines inline */ }
  else { /* 271 lines inline */ }
```

The **contract** is exactly what kinds 2 and 3 should be refactored into, and the executor should match it point for point:
- a `pub(super) fn` on `impl App`, defined in a sibling file under `render/`
- takes `&mut self, f: &mut Frame, …` first
- **returns `usize`** (the final scroll offset), assigned to `final_offset`
- called from a single arm of the existing `if / else if / else`, with the prelude and the offset-persisting tail (L1139–1148) left untouched in `list.rs`

The **parameter list** is *not* directly reusable: it is an 8-arg positional list, and kinds 2 and 3 need three more prelude values (`banner_rows`, `banner_content_rows`, `series_detail_rows`) that grouped-album lists never have — an album list shows no movie banner and no series inline detail. Growing that to an 11-arg positional list would be worse than what exists. See A3 for the resolution.

**Explicitly out of scope:** do **not** change `render_power_grouped_album_rows`'s signature to match the new two. It lives in `album.rs`, which is Lane B's file — that would be a cross-lane edit, and the symmetry gained is cosmetic. The three kinds come out symmetric at the level that matters (same contract, same call shape, one sibling file each); the argument lists differ because the kinds genuinely differ.

---

## 1. Guardrails

**Must have**
- Every resulting file lands **well under 800 lines**; target **200–500**.
- Behavior-preserving. **Moves are performed mechanically — see §1a. The model must never retype moved code.**
- One independently reviewable diff per lane; independent review (`code-reviewer`) before merge; **no self-merge**.
- Each executor works in an isolated git worktree (AGENTS.md, `worktrees` skill).
- Test-extraction commits are separate from production-split commits within a lane.
- Per lane: `cargo fmt --all -- --check`, `cargo build`, `cargo clippy --all-targets -- -D warnings`, and the lane's targeted tests — all green before review (§5).

**Must NOT**
- No logic changes, no signature changes, no drive-by cleanups (surgical rule).
- Never edit a call site to satisfy visibility — widen the moved item instead.
- No new dependencies.
- **Do not create `render/layout.rs`** — collides with `src/app/layout.rs` one level up (#365 flagged this).
- Do not move the entry-point methods that `power_widgets.rs` / `music.rs` dispatch into out of their origin file (see §4).
- Do not touch `render/tests.rs` (the 2,627-line #365 step-1 artifact) — out of scope for this issue.
- Do not modify `render_power_grouped_album_rows` (Lane B's file) from Lane A.
- **Do not read the target file in full.** See §1b.

---

## 1a. Mechanical extraction protocol (supersedes "paste verbatim")

Earlier drafts said "cut a symbol from its origin, paste verbatim". That instruction round-trips every moved line through the model — paid twice (once read, once written) and, worse, every pass is an opportunity to silently alter a line that only a careful reviewer would catch. **Replaced.** Moved code is relocated by shell span-cut. Verbatim-ness becomes a structural property of the operation rather than a discipline the reviewer has to police.

**Correctness guardrail is unchanged: locate by symbol name, never by the line numbers in this plan.** Every span in §3 is a locator *hint* recorded at `9a3e915` and will drift with any edit. The executor resolves real boundaries first (grep for the symbol, `mcp__serena__find_symbol`, or LSP), confirms the closing line, and only then cuts the resolved span. Symbol-name lookup is the correctness step; the span-cut is the transport.

**Procedure per moved cluster:**

1. **Resolve** each symbol's true start and end line in the current working tree. Record them.
1a. **Extend each start upward** through every contiguous preceding line that is a doc comment (`///`, `//!`) or an attribute (`#[…]`), stopping at the first blank line or unrelated code. **This step is mandatory and is the single most likely thing to be skipped.** Symbol-range tools (serena, LSP, `grep -n 'fn foo'`) report the *item* line, excluding its docs and attributes — so a naive cut silently strips them, orphaning the doc block in the origin and, when an attribute is involved, changing behavior. Real casualties in this plan's own hints:

   | Origin | Hint | True start | What a naive cut loses |
   |---|---:|---:|---|
   | `album.rs` `enum GroupedAlbumDisplayRow` | 86 | **85** | `#[derive(Clone)]` — the moved enum loses `Clone`; compile error |
   | `album.rs` `render_power_album_detail` | 1467 | **1457** | `#[allow(clippy::too_many_arguments)]` (L1466) — fails `-D warnings` — *and* a 9-line doc block above it starting L1457 |
   | `album.rs` `enum ArtAnchorX` | 47 | **46** | `#[derive(Clone, Copy)]` — `align_art` takes it by value, so losing `Copy` cascades |
   | `album.rs` `enum ArtAnchorY` | 54 | **53** | `#[derive(Clone, Copy)]` — same cascade |
   | `list.rs` `COMPACT_BANNER_RULE_ROWS` | 28 | **14** | a 14-line doc block |
   | `home.rs` `power_home_panel_scroll` | 17 | **13** | 4-line doc block |
   | `home.rs` `struct KeepWatchingHeroLayout` | 163 | **157** | 6-line doc block |
   | `detail.rs` `SERIES_DETAIL_DIVIDER_ROWS` | 58 | **51** | 7-line doc block |
   | `detail.rs` `SERIES_DETAIL_TRAILING_BLANK_ROWS` | 64 | **61** | 3-line doc block |
   | `detail.rs` `series_meta_line` | 72 | **68** | 4-line doc block |

   The two attribute cases are compile/lint failures and will be caught. The doc-block cases are silent — the code compiles, the documentation is simply lost from one file and stranded in the other. Only this step catches them.
2. **Extract** to the new file, appending in the order the plan lists (multiple disjoint spans concatenate):
   ```
   sed -n '<start>,<end>p' src/app/render/<origin>.rs >> src/app/render/<new>.rs
   ```
   Redirect to the file — **do not** let span content print to the terminal, or it lands in context anyway and the whole exercise is wasted. Append spans in **ascending source order** so the new file reads in the same order as the original. `sed -n 'p'` preserves each line's trailing newline, so consecutive spans concatenate cleanly; if you build the header with a heredoc, make sure it ends with a newline before the first append.
3. **Delete** the same spans from the origin, **highest line number first**, so earlier ranges keep their numbering:
   ```
   sed -i '<start>,<end>d' src/app/render/<origin>.rs
   ```
4. **Hand-write only the wrapper**: the new file's `use` header (~10–20 lines), the `mod <new>;` line in `render/mod.rs`, and any visibility edits (one-line `Edit` calls against a symbol's declaration, never a rewrite of its body).
5. **Verify byte-identity** before committing — see §5.

**Escape hatch — when the compiler demands a bump your brief didn't list.** This will happen in every lane; §0 Finding 3's inventory is a forecast, not a whitelist. When `cargo build` reports a privacy error on a symbol your brief did not mention:
- Widen that symbol's declaration to `pub(super)` — one line, no body change. This is authorized; you do not need to stop and ask.
- **Never** widen beyond `pub(super)`, and **never** edit the call site to route around the error.
- If the symbol is a struct or enum, its **fields/variants may need widening individually** — that is normal (Lane B and Lane C both require it) and is still a one-line-per-field change.
- If a fix seems to require anything else — a signature change, a body edit, moving a symbol not in your brief, or touching a file outside your lane — **stop and report** rather than improvising. That is the signal that the plan is wrong about something, which is worth more than a workaround.
- Record every widening you applied in your final report, including the ones your brief did predict.

**Test extraction is the purest case: a single trailing span-cut, zero model reproduction.** Verified at `9a3e915` — each of the three files has exactly one `#[cfg(test)]` occurrence, opening a `mod tests {` block that closes with the file's final `}` at EOF:

| File | Test span | EOF | Occurrences of `#[cfg(test)]` |
|---|---|---:|---:|
| `render/list.rs` | 1152 → 1811 | 1811 | 1 |
| `render/home.rs` | 1093 → 1423 | 1423 | 1 |
| `render/detail.rs` | 1029 → 1354 | 1354 | 1 |

So the cut is `sed -n '<start>,$p'` into the test file, then `sed -i '<start>,$d'` on the origin, then append the `#[cfg(test)] #[path = "…"] mod tests;` declaration. **Verify before cutting** — re-confirm the start line and the single-occurrence property; both drift with any edit to the file.

**A3 is the one commit with genuinely new content** (the carrier struct, two signatures, two derived-local lines, the rewritten dispatch arms). Even there the two branch *bodies* are span-cuts and are held to the same byte-identity check; only the wrapper lines are new. See §3 Lane A / A3.

---

## 1b. Context discipline: symbol overview, not full reads

Because the moves are mechanical, **the executor never needs to read the bodies it is moving**. Reading `list.rs` in full costs ~18.9k tokens to acquire content the model will not use and must not reproduce.

**What the executor reads:**
- The origin file's **import header** — the `use` block at the top, typically lines 1–30. Needed to write each new file's header.
- A **symbol overview** — `mcp__serena__get_symbols_overview` on the origin file, or a one-line-per-symbol outline:
  ```
  grep -nE '^\s*(pub(\([a-z:() ]*\))? )?(async )?fn |^\s*(pub(\([a-z:() ]*\))? )?(struct|enum|const|type) |^impl ' src/app/render/<origin>.rs
  ```
- **Boundary confirmation** for each symbol being cut (the closing-brace line — `awk`/`sed` on the specific range, or serena's symbol body range; do not print the body).
- **Call sites** for any symbol whose visibility might need widening — `grep -rn '<symbol>' src/` (this plan already did that work; §3's visibility notes are authoritative, the grep is confirmation).

**What the executor does not read:** the bodies of moved functions, the test modules, or `render/tests.rs`.

The exception is **A3**, which must understand the two branch bodies to validate the carrier's field list. It reads them **once** (~6k) and still never re-emits them.

---

## 2. Lane structure and parallelism

Four lanes, but **five agent runs** — Lane A is split into two, because its two halves have opposite cost profiles and mixing them is what pushes a single agent past its context budget (§7a).

| Run | Owns (edits) | Commits | New files | Nature | Parallel? |
|---|---|---|---|---|---|
| **A-mech — `list.rs` moves** | `render/list.rs` | A1, A2 | 2 | mechanical | yes |
| **A3 — `list.rs` decomposition** | `render/list.rs` | A3 | 2 | genuine refactor | **after A-mech lands** |
| **B — `album.rs`** | `render/album.rs` | B1 | 4 | mechanical | yes |
| **C — `home.rs`** | `render/home.rs` | C1, C2 | 4 | mechanical | yes |
| **D — `detail.rs`** | `render/detail.rs` | D1, D2 | 3 | mechanical | yes |

A-mech, B, C, D are file-disjoint and run **fully in parallel** (4 concurrent, under the 6-agent cap). A3 is sequential on A-mech — same file — and starts as a **fresh agent on the merged result**, not a continuation. That split is deliberate: A-mech carries no content in context, A3 is the only run that needs to reason about list-rendering logic, and a fresh start means A3 spends its whole budget on the part that needs a model.

**The single shared file is `src/app/render/mod.rs`** — each lane appends its `mod x;` lines to the declaration block at the top. Same contention #367 L1/L2 had; a **trivial additive union merge** (each lane adds only its own lines, resolve by keeping all).

Recommended integration order: **D, B, C, A-mech, then A3** (cheapest/lowest-risk first, so the `mod.rs` declaration block converges before the riskiest run rebases onto it). Each run rebases on `main` before its review pass.

---

## 3. Per-lane file plans

### Lane A — `render/list.rs` (1,811 → ~400)

Three commits, strictly sequenced, **across two agent runs** (§2): run **A-mech** does A1 + A2 (pure moves, no content in context); run **A3** does the decomposition as its own revertible commit, on a fresh agent after A-mech merges.

#### Commit A1 — test extraction (pure move, zero risk)
| New file | Contents | ~Lines |
|---|---|---:|
| `render/list_tests.rs` | the whole `#[cfg(test)] mod tests` body (L1152–1811). Declared from `list.rs` as `#[cfg(test)] #[path = "list_tests.rs"] mod tests;` | 665 |

The test module opens with `use super::*;` — after A2/A3 move symbols out of `list.rs`, that glob no longer reaches them. Fix with explicit `use super::list_rows::*;` (etc.) **inside the test file**, never by re-exporting in `list.rs` for tests' sake alone. Same warning-hygiene rule as #365 lane C: any `use` in `list.rs` left consumed *only* by the test module gets `#[cfg(test)]`.

#### Commit A2 — free helpers (pure move)
| New file | Symbols moved (from `list.rs`) | ~Lines |
|---|---|---:|
| `render/list_rows.rs` | `COMPACT_BANNER_RULE_ROWS`, `COMPACT_BANNER_GAP_ROWS`, `COMPACT_BANNER_INDENT` (L**14**–30, incl. the 14-line doc block); `enum DisplayRow` (L32); `push_selected_detail_fillers_before` (L40); `push_selected_detail_fillers_after` (L57); `selected_detail_lower_bound` (L78); `build_list_row_spans` (L94); `render_series_detail_background` (L159) | 190 |

All are currently module-private free items; they become `pub(super)`, and `list.rs` imports them via `use super::list_rows::{…}`. `render_series_detail_background` is called from both branch bodies (L698, L957), which end up in the A3 files — same module tree, no problem.

#### Commit A3 — decompose `render_power_list` along list-kind seams

One new file **per list kind**, named after the kind:

| New file | Contents | ~Lines |
|---|---|---:|
| `render/list_letter_groups.rs` | kind 2 — `App::render_power_letter_grouped_rows(…) -> usize`, the L550–865 body verbatim | 340 |
| `render/list_plain.rs` | kind 3 — `App::render_power_plain_rows(…) -> usize`, the L866–1137 body verbatim | 295 |

Kind 1 stays a delegate to `album.rs` and is **not touched**.

**Executor instruction — match the existing delegate's contract exactly** (see §0 Finding 5 for the analysis): each new function is a `pub(super) fn` on `impl App` in its own sibling file, takes `&mut self, f: &mut Frame, …`, returns `usize`, and is called from exactly one arm of the existing `if show_grouped / else if use_letter_groups / else` chain with its return assigned to `final_offset`. After A3 the dispatch should read as three structurally identical one-call arms. The prelude (L290–537, incl. the `n == 0` early return) and the offset-persisting tail (L1139–1148) stay in `list.rs` unchanged.

**Carrier — narrower than the 11-field struct in the first draft.**

**Treat the table below as a starting point, not a specification.** It was assembled by measuring per-branch usage of the locals the first draft had already identified — which is not the same as enumerating the bodies' free variables, and it did in fact miss one (`ungrouped_total`, caught in review). **The A3 executor must re-derive the free-variable set directly from the two spans** — every identifier referenced in the body that is bound in the prelude — and reconcile it against this table, reporting any additions. A missed free variable surfaces as a compile error rather than a silent bug, so this is a cost-not-correctness risk; re-deriving it up front is cheaper than discovering it mid-extraction.

| Prelude local | kind 2 (letter) | kind 3 (plain) | Verdict |
|---|---|---|---|
| `content_area` | used | used | carrier field |
| `items` | used | used | carrier field |
| `cursor` | used | used | carrier field |
| `stored_scroll` | used | used | carrier field |
| `banner_rows` | used | used | carrier field |
| `banner_content_rows` | used | used | carrier field |
| `series_detail_rows` | used | used | carrier field |
| `focused` | used | used | carrier field |
| `active_letter_filter` | used | **unused** | **separate arg, kind 2 only** |
| `ungrouped_total` | used (L569) | **unused** | **separate arg, kind 2 only** |
| `n` | `= items.len()` | `= items.len()` | **drop — derive in callee** |
| `visible` | `= content_area.height as usize` | same | **drop — derive in callee; also delete from the prelude, it becomes dead** |
| `use_letter_groups` | comment only | comment only | **drop — not a real use** |

`ungrouped_total` (bound at L452–457) is read once inside kind 2, at L569:
```rust
let bucket_total = if active_letter_filter.is_some() { usize::MAX } else { ungrouped_total };
```
It has exactly the same shape as `active_letter_filter` — kind 2 only — so `render_power_letter_grouped_rows` takes **two** kind-2-only arguments, and kind 3's signature stays free of both.

So the carrier is **8 fields, not 11**, plus `f`, `layout`, and — for kind 2 only — `active_letter_filter` and `ungrouped_total`:

```rust
// in list_rows.rs
pub(super) struct ListRenderCtx<'a> {
    pub(super) content_area: Rect,
    pub(super) items: &'a [mbv_core::api::MediaItem],
    pub(super) cursor: usize,
    pub(super) stored_scroll: usize,
    pub(super) banner_rows: usize,
    pub(super) banner_content_rows: usize,
    pub(super) series_detail_rows: usize,
    pub(super) focused: bool,
}
```

The two derived locals (`n`, `visible`) are recomputed inside each callee from `items` / `content_area` — one line each, and it keeps the carrier honest. `active_letter_filter` and `ungrouped_total` are plain extra parameters on `render_power_letter_grouped_rows`, keeping kind 3's signature free of two fields it never reads.

If the executor finds that 8 fields plus `f`/`layout` reads acceptably as a flat parameter list (closer to the existing grouped delegate's positional style), **that is an acceptable substitution** — the ruling is "narrowest thing that compiles, cut per kind", not "must be a struct". What is *not* acceptable is passing a kind a value it does not read.

**Borrow-checker check (verified, not assumed):** `items` is an owned local `Vec`, cloned out of `self.libs[..].nav_stack` / `search.results` at L314–345 — **not** a borrow of `self`. So `&'a [MediaItem]` in the carrier coexists with `&mut self` on the extracted methods. No clone or `RefCell` gymnastics needed.

**Resulting `list.rs`**: `use` header + `compact_banner_rows` + `render_series_detail_if_visible` + `render_series_detail_top_border` + `render_power_list` prelude/dispatch/tail ≈ **400 lines**.

### Lane B — `render/album.rs` (1,740 → ~520)

No test module — production split only, and the cleanest lane of the four. All moves are pure.

| New file | Symbols moved | ~Lines |
|---|---|---:|
| `render/album_art.rs` | `INLINE_ALBUM_ART_COLS/ROWS/GAP/RIGHT_PAD/RESERVED` (L16–21); `inline_album_art_cache_key` (L23); `inline_art_box_rect` (L32); `enum ArtAnchorX` (L47); `enum ArtAnchorY` (L54); `align_art` (L64); `App::render_inline_album_art` (L1244); `App::render_inline_artist_collage` (L1283); `App::render_inline_art_cell` (L1390) | 315 |
| `render/album_plan.rs` | `enum GroupedAlbumDisplayRow` (L**85**–102, incl. its `#[derive(Clone)]`); `struct GroupedAlbumDisplayPlan` (L104); `impl GroupedAlbumDisplayRow` (L116, both methods); `App::album_artist_label` (L136); `App::build_grouped_album_display_plan` (L144–463) | 395 |
| `render/album_cursor.rs` | `App::selected_power_music_artist_header` (L464); `clear_artist_header_focus` (L471); `set_artist_header_focus` (L477); `move_power_music_group_display_cursor` (L484); `jump_power_music_group_display_cursor` (L549); `selected_artist_header_album_items` (L598); `artist_header_album_items_for_selection` (L607); `page_power_grouped_album_cursor` (L660–735) | 285 |
| `render/album_detail.rs` | `App::render_power_album_detail` (L**1457**–1739, incl. its doc block and `#[allow(clippy::too_many_arguments)]`) + `INLINE_ALBUM_TITLE_EXTRA_INDENT`, `INLINE_ALBUM_TRACK_EXTRA_INDENT` (L14–15) if only used there — verify at execution | 285 |

**Resulting `album.rs`**: `use` header + `App::render_power_grouped_album_rows` (L736–1243) ≈ **520 lines**.

Visibility notes:
- **Six** of the eight symbols assigned to `album_cursor.rs` carry `pub(in crate::app)` (callers: `input_context_menu.rs`, `artist_header_actions.rs`, `lib_cursor_actions.rs`, `input_lib_power_keys.rs`, `actions.rs`) — **keep those annotations verbatim**; `pub(in crate::app)` is path-absolute, so nesting one level deeper changes nothing. Same treatment #365 gave `selected_album_item`. The remaining two are **private**: `selected_power_music_artist_header` (464), which needs `pub(super)` (see §0 Finding 3), and `set_artist_header_focus` (477), which does not.
- `render_power_grouped_album_rows` is `pub(super)` and called from `music.rs:143` and `list.rs:540` — it **stays in `album.rs`**, signature unchanged (Lane A must not touch it).
- `render_power_album_detail` moves but is only called from `album.rs:1121`; `pub(super)` still reaches.

### Lane C — `render/home.rs` (1,423 → ~340)

Commit C1: `render/home_tests.rs` — L1093–1423, `#[cfg(test)] #[path = "home_tests.rs"] mod tests;` (~331 lines). Its head is `use super::power_home_panel_scroll;` — retarget to the new module path after C2.

Commit C2 (pure moves):
| New file | Symbols moved | ~Lines |
|---|---|---:|
| `render/home_video.rs` | `power_home_panel_scroll` (L**13**, incl. doc block); `MONTHS` (L35); `parse_ymd` (L53); `format_release_date` (L66); `render_home_video_item` (L72–154); `App::render_selected_home_video_detail` (L171); `App::render_power_home_video_list` (L201–331) | 320 |
| `render/home_feed.rs` | `App::render_power_feed_home_video_group_view` (L332–518) | 195 |
| `render/home_hero.rs` | `struct KeepWatchingHeroLayout` (L**157**–168, incl. its 6-line doc block); `App::keep_watching_hero_image_types` (L519); `keep_watching_hero_layout` (L530); `render_keep_watching_hero_image` (L577); `render_keep_watching_hero_meta` (L623–764) | 265 |

**Resulting `home.rs`**: `use` header + `App::render_power_home_list` (L765–1042) + `App::render_power_home_section_pills_row` (L1043–1090) ≈ **340 lines**.

Visibility notes:
- `render_power_home_video_list` and `render_power_feed_home_video_group_view` are dispatched from `power_widgets.rs:555,561`. Both keep `pub(super)`; the new files are siblings under `render`, so `super` is still `render`. **No bump.**
- `render_home_video_item` (private free fn) is called from `home.rs:285` and `home.rs:480`; the L480 call is inside the feed-group view, which moves to `home_feed.rs`. It becomes `pub(super)` in `home_video.rs` — one private→`pub(super)` bump, the minimum that works.
- `render_power_home_list` stays in `home.rs` (dispatched from `power_widgets.rs:546`).

### Lane D — `render/detail.rs` (1,354 → ~515)

Commit D1: `render/detail_tests.rs` — L1029–1354, `#[cfg(test)] #[path = "detail_tests.rs"] mod tests;` (~326 lines). Uses `use super::*` and references `CompactBannerLayout`, which stays in `detail.rs`, so the glob keeps working.

Commit D2 (pure moves):
| New file | Symbols moved | ~Lines |
|---|---|---:|
| `render/detail_series.rs` | `SERIES_DETAIL_DIVIDER_ROWS`, `SERIES_DETAIL_EPISODE_ROWS_ESTIMATE`, `SERIES_DETAIL_OVERVIEW_MAX_LINES`, `SERIES_DETAIL_TRAILING_BLANK_ROWS`, `SERIES_IMAGE_COLS/ROWS/PLACEHOLDER_ROWS` (L**51**–67 — three separate doc blocks at 51–57, 61–63 sit above these consts); `series_meta_line` (L**68**, incl. doc block); `wrap_overview_lines` (L94); `App::series_selection_state` (L396); `App::series_inline_detail_rows` (L415–468) | 155 |
| `render/detail_series_view.rs` | `App::render_series_inline_detail` (L469–845) | 390 |

**Resulting `detail.rs`**: `IMG_COLS`/`IMG_ROWS`, `compact_banner_image_cache_key`, `poster_placeholder_size`, `CompactBannerLayout` + its impl, `power_selected_movie_item`, `power_selected_series_item`, `compact_banner_layout`, `compact_banner_layout_with_overview`, `render_power_compact_detail` ≈ **515 lines**.

Visibility notes:
- `compact_banner_image_cache_key` **must stay in `detail.rs`** — `list.rs:2` imports it by path. This is the only path-based cross-file dependency among the four target files; keeping it put means Lane D needs no coordination with Lane A. (It is 3 lines; splitting it out would buy nothing and cost a cross-lane edit.)
- `series_selection_state` / `series_inline_detail_rows` are `pub(super)`, called from `list.rs:378–379`; still `pub(super)` under `render`. **No bump.**
- `render_series_inline_detail` is `pub(super)`, called from `list.rs:243`. **No bump.**
- `wrap_overview_lines` is used only at L436 and L575, both inside symbols moving to Lane D's new files; it becomes `pub(super)` so `detail_series_view.rs` can reach it.
- `power_selected_movie_item` / `power_selected_series_item` are `pub(crate)` (callers in `lib_cursor_actions.rs`, `input_lib_power_keys.rs`, `input_queue_keys.rs`) — they stay in `detail.rs` anyway.

---

## 4. Cross-lane shared helpers — resolution

The question the issue asked ("what would duplicate across lanes?"). Answer: **nothing needs duplicating, and no lane needs to edit another lane's file**, because of three structural facts plus two guardrails:

1. **Inherent methods don't move by path.** Every `App::*` symbol here can relocate freely; only visibility matters — and per §0 Finding 3, ~20 of them do need widening to `pub(super)`, since splitting a file turns intra-file private calls into cross-module ones.
2. **All new files are `render`'s direct children**, so `pub(super)` means exactly what it means today — no bump ever needs to exceed `pub(super)`. There are **~20 private→`pub(super)` widenings** across the four lanes (inventory in §0 Finding 3; an earlier draft badly undercounted this at two). What matters for parallelism is that **every one of them is on a symbol inside its own lane's file, applied by that lane's own executor. No bump crosses a lane boundary**, so the independence claim is unaffected.
3. **Guardrail**: `compact_banner_image_cache_key` stays in `detail.rs` (the only free item imported across the four target files).
4. **Guardrail**: Lane A does not touch `render_power_grouped_album_rows` in `album.rs` (§0 Finding 5). The kind-1 delegate keeps its current signature.
5. Everything the four files pull from `render`'s other siblings — `effective_sort_str`, `letter_bucket`, `natural_sort_key`, `parse_album_folder_name`, `strip_article`, `POWER_RENDER_FILTER`, `render_power_placeholder`, `render_selected_block_background` — is re-exported from `render/mod.rs` by #365 lane C and untouched here.

No prior sequencing step is required. The `mod x;` additions to `render/mod.rs` are the only contention and merge additively.

---

## 5. Per-lane verification

Every lane, before requesting review:

```
cargo fmt --all -- --check
cargo build
cargo clippy --all-targets -- -D warnings
```

Plus targeted tests (run in background per AGENTS.md):

| Lane | Targeted test command |
|---|---|
| A | `cargo test -p mbv app::render::list` and `cargo test -p mbv app::input_power_movie_detail` |
| B | `cargo test -p mbv app::input_power_music_track_focus` and `cargo test -p mbv app::render` |
| C | `cargo test -p mbv app::render::home` |
| D | `cargo test -p mbv app::render::detail` and `cargo test -p mbv app::render::list` (list depends on detail's series/banner helpers) |

Full `cargo test` once at integration, after all lanes merge — not per lane.

**Zero-warning bar** (from #365 lane C's fixup): `cargo check --all-targets` must be clean of *warnings*, not just errors. Expect to gate a handful of now-test-only imports behind `#[cfg(test)]` in the residual files after test extraction. Budget for this explicitly; it was the review finding that came back on #365 lane C.

**Behavior-preservation evidence.** The mechanical protocol (§1a) upgrades this from a soft signal to a hard check. "`git diff --stat` shows near-equal insertions/deletions" was only ever circumstantial — equal counts are consistent with a silently altered line. Under span-cuts, byte-identity is directly provable, and **that is now the required evidence**:

```
# for each moved span, against the pre-commit state of the origin
git show <parent>:src/app/render/<origin>.rs | sed -n '<start>,<end>p' > /tmp/before
sed -n '<newstart>,<newend>p' src/app/render/<new>.rs                 > /tmp/after
diff /tmp/before /tmp/after      # MUST be empty
```

Every moved span in commits A1, A2, B1, C1, C2, D1, D2 must produce an empty `diff`. A non-empty diff means the model touched content it was told not to, and the commit is rejected.

**Legitimate differences in these commits:**
1. the new files' `use` headers;
2. the `mod x;` lines in `render/mod.rs`;
3. **visibility annotations, widened as needed** — a one-line change to a symbol's declaration (`fn foo` → `pub(super) fn foo`, and likewise for structs, enums, consts, and struct *fields*);
4. `use` statements in the origin that became unused or test-only (see below).

On (3): **do not gate on a pre-approved list.** An earlier draft said the only legitimate visibility changes were "the enumerated visibility edits in §3", and §3's enumeration was wrong by an order of magnitude — under that rule a *correct* Lane B diff would be rejected. §0 Finding 3 now carries a per-lane inventory (~20 bumps), but treat it as a **forecast, not a whitelist**: the compiler is the authority on which symbols need widening. The executor widens what the build demands, up to but never beyond `pub(super)`, and **enumerates every widening it applied in its final report**. The reviewer checks that list for anything suspicious (a widening past `pub(super)`, or on a symbol with no cross-module caller) rather than diffing it against a plan section.

None of these widenings cross a lane boundary — every one is inside the lane's own file — so this does not weaken lane independence.

On (4): after extraction, `use` statements in the origin file fall into three cases. **Dead** (no remaining consumer) → delete. **Test-only** (consumed solely by the origin's `#[cfg(test)]` module via `use super::*`) → gate behind `#[cfg(test)]`. **Moved** (now consumed only by a new sibling) → delete from the origin and add to that sibling's header. All three are expected and are covered by the `-D warnings` gate.

**A3 is the only commit where byte-identity does not cover the whole diff** — and even there it covers most of it. The two branch bodies (~316 and ~271 lines) are still span-cuts and are held to the empty-`diff` requirement **with two lines excluded** (below). Genuinely new content in A3 is bounded and should be reviewable at a glance:
- the `ListRenderCtx` struct definition
- two `fn` signatures and their closing braces
- two derived-local lines per callee (`n`, `visible`)
- the rewritten dispatch arms in `list.rs`
- deletion of the now-dead `let visible = …` from `list.rs`'s prelude
- the two in-body return edits described next

**Required correction: the branch bodies do not end in their return value.** `final_offset` is declared in the prelude (`list.rs:536`) and assigned **mid-body** — at L651 in kind 2 and L923 in kind 3 — after which each body runs ~210 more lines, ending in a call to `Self::render_series_detail_top_border(…)` (L858 / L1129). A body converted to `-> usize` therefore cannot be a pure span-cut; an earlier draft's "no in-body edits" rule made the extraction impossible as specified.

**Exactly two in-body edits are permitted per extracted body, and no others:**
1. the existing `final_offset = offset;` becomes `let final_offset = offset;` (it is no longer assigning a prelude-scoped binding);
2. a trailing `final_offset` expression is appended immediately before the closing brace.

The byte-identity `diff` for A3's bodies is run **with those two lines excluded** — e.g. filter both sides through `grep -v` on the two known lines, or diff the body ranges that exclude them. Everything else in the two bodies must still match the parent byte for byte.

Additionally, `let visible = content_area.height as usize;` (`list.rs:535`) becomes **dead in `list.rs`** once both bodies leave — it is used 15 times in kind 2 and 14 in kind 3, and nowhere else. Delete it from the prelude; each callee recomputes it (§3 Lane A). Leaving it in place fails `-D warnings`.

Anything outside the list above appearing in A3's diff is scope creep. A3 additionally requires the behavioral check in §8: run `list.rs`'s render-to-buffer tests before and after and diff the rendered output.

---

## 6. Resulting file sizes

| File | Before | After |
|---|---:|---:|
| `render/list.rs` | 1,811 | ~400 |
| `render/list_letter_groups.rs` | — | ~340 |
| `render/list_plain.rs` | — | ~295 |
| `render/list_rows.rs` | — | ~190 |
| `render/list_tests.rs` | — | ~665 |
| `render/album.rs` | 1,740 | ~520 |
| `render/album_plan.rs` | — | ~395 |
| `render/album_art.rs` | — | ~315 |
| `render/album_cursor.rs` | — | ~285 |
| `render/album_detail.rs` | — | ~285 |
| `render/home.rs` | 1,423 | ~340 |
| `render/home_video.rs` | — | ~320 |
| `render/home_hero.rs` | — | ~265 |
| `render/home_feed.rs` | — | ~195 |
| `render/home_tests.rs` | — | ~331 |
| `render/detail.rs` | 1,354 | ~515 |
| `render/detail_series_view.rs` | — | ~390 |
| `render/detail_series.rs` | — | ~155 |
| `render/detail_tests.rs` | — | ~326 |

Every production file lands under 550. The one test file at 665 (`list_tests.rs`) is acceptable by precedent — `render/tests.rs` is 2,627 today.

---

## 7. Rulings and remaining open items

**Ruled (user, this revision):**
- **R1 — Lane A3 is in, cut along list-kind seams.** Confirmed against the code: three mutually exclusive kinds, discriminant named, kind 1 already a delegate whose *contract* the other two must match (§0 Finding 5). Carrier narrowed from 11 fields to 8 + **two** kind-2-only args. A3 is its own commit and its own agent run, sequenced after A1/A2.
- **R2 — `action.rs` / `input_resolver.rs` deferred.** Dropped from this plan; survey preserved in §11 for a follow-up issue. Not filed by the planner.
- **R3 — executor context budget is a first-class constraint.** §1a/§1b protocol, Lane A split into two runs, per-run briefs in §12, budget table in §7a.
- **R4 — Lane D file naming decided:** `detail_series.rs` (measurement) + `detail_series_view.rs` (rendering), as listed. Recorded in §12.4; the executor does not need to ask.
- **R5 — Lane A function naming decided:** `render_power_letter_grouped_rows` / `render_power_plain_rows`. Recorded in §12.5.
- **R6 — `album.rs` residual at ~520 accepted**, with `render_power_grouped_album_rows` (~508) left intact. It is under the ceiling, and decomposing it would put a second A3-shaped change into a lane that is otherwise pure moves. Recorded in §12.2 as decided, so it is not re-raised mid-run.

**Nothing is blocking. All five runs can be dispatched.**

**Corrections applied after the independent critic pass** (the design was verified sound and unchanged; these are accuracy fixes):
- §0 Finding 3 / §4 item 2: the visibility-bump count was wrong by an order of magnitude (two → ~20). Replaced with a per-lane inventory. §5's review gate was rewritten accordingly — as written it would have **rejected a correct Lane B diff**.
- §5 / §12.5: A3's byte-identity requirement was unsatisfiable, because `final_offset` is assigned mid-body (L651/L923), not returned at the end. Two in-body edits are now explicitly permitted and excluded from the diff.
- §3: the carrier table was missing `ungrouped_total` (kind-2-only, L569). The table is now labelled a starting point, with the executor required to re-derive the free-variable set from the spans.
- §1a: added the mandatory "extend start upward through doc comments and attributes" step. Eight span hints in this plan orphaned docs or attributes, two of which (`#[derive(Clone)]`, `#[allow(clippy::too_many_arguments)]`) would have broken the build.
- §7a: recalibrated from ~32–44k to ~38–58k typical; "3× reduction" corrected to ~2×.

---

## 7a. Executor context budget

Lane scoping in §2 is by *resulting file size*, which says nothing about how many tokens an executor burns getting there. Refactors of this size have hit context exhaustion before, so the budget is derived here rather than discovered mid-run.

**Measured floors under the old "read the file, paste verbatim" protocol** (full read + re-emitting moved content + a 419-line plan + agent baseline):

| Lane | Full read | Content re-emitted | Floor | With 2–3× compile/clippy iteration |
|---|---:|---:|---:|---|
| A `list.rs` | 18.9k | ~15.5k (1,485 ln) | ~53k | **106–159k — over threshold** |
| B `album.rs` | 18.0k | ~13k (1,280 ln) | ~50k | **100–150k — over threshold** |
| C `home.rs` | 12.4k | ~11k (1,111 ln) | ~42k | 84–126k |
| D `detail.rs` | 13.8k | ~9k (871 ln) | ~42k | 84–126k |

A and B cross the 125k handoff threshold. The iteration multiplier is not speculative — §5's `-D warnings` gate makes an unused-import cleanup pass **likely**, and it is rated "high" in §8's risk table.

Worth naming plainly, because §3 currently invites the wrong inference: **Lane B is described as "the cleanest lane, all pure moves" and is the second-heaviest in tokens.** Cleanliness and context cost are unrelated. B is expensive precisely *because* it moves the most code — 1,280 lines across four files with no test module to absorb any of it. Do not let "clean" stand in for "cheap" when sizing runs.

**Re-derived under the §1a/§1b protocol.** Moved content never enters context in either direction; the executor reads a symbol overview instead of the file, and writes only wrappers:

**Recalibrated after review.** A first pass at this table assumed a near-frictionless run and landed at ~32–44k. Two review findings pushed it up: the visibility-bump count is **~20, not two** (§0 Finding 3), and each batch costs another build → privacy-error → widen → rebuild cycle; and orphaned doc/attribute lines (§1a step 1a) produce breakages — one of them a `-D warnings` failure — that are individually cheap but arrive in clusters. The numbers below assume those costs rather than hoping against them.

| Run | Baseline | Plan slice | Overview + boundaries + call sites | Written content | Build/clippy/test iteration | **Typical** | **Pessimistic** |
|---|---:|---:|---:|---:|---:|---:|---:|
| A-mech | 14k | 2k | 3k | 2k | 12–20k | **~38k** | ~55k |
| B | 14k | 2k | 4k | 4k (14 bumps, incl. 5 struct fields) | 20–35k | **~58k** | ~80k |
| C | 14k | 2k | 3k | 3k (6+ bumps, incl. 4 struct fields) | 15–25k | **~44k** | ~62k |
| D | 14k | 2k | 3k | 2.5k (8 bumps) | 14–22k | **~40k** | ~58k |
| A3 | 14k | 3k | 10k (both bodies + free-variable re-derivation) | 3k | 20–35k | **~58k** | ~80k |

Roughly a **2× reduction** — not the 3× first claimed. The lanes that were over threshold are now comfortably under it, which was the point.

**Runs at or above ~55k typical: B and A3 (~58k each, ~80k pessimistic).** Both stay under the 125k threshold with real headroom, but neither has room for a second surprise:
- **B is the heaviest mechanical run** — most lines moved *and* the largest bump batch (14, five of them struct fields needing individual edits). Its "cleanest lane, all pure moves" description is still true and still irrelevant to cost.
- **A3** carries the only content reasoning in the issue, plus the free-variable re-derivation the carrier table now requires.
- Contingency for either: split by output file — B into (`album_plan.rs` + `album_cursor.rs`) then (`album_art.rs` + `album_detail.rs`); A3 into one run per list kind, clean because the kinds are disjoint by construction (§0 Finding 5). Contingency, not plan.

Three ways to blow the budget regardless of protocol, all avoidable:
- Letting `sed -n` print span content to the terminal instead of redirecting to a file (§1a step 2). This silently reintroduces the full re-emit cost.
- Reading the target file "just to be safe" before cutting. The symbol overview is sufficient; §3's visibility notes are already resolved.
- Running full `cargo test` per lane instead of the targeted commands in §5. Full-suite output is large and, per §5, is an integration-time step.

---

## 8. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| **Executor exhausts context mid-run**, leaving a half-applied refactor | was high for A/B, now low | §1a/§1b protocol drops every run to ~32–44k typical (§7a). A-mech/A3 split keeps the one model-heavy run isolated. Contingency if A3 approaches budget: sub-split by kind, one run per list kind. |
| A3 changes rendering subtly (off-by-one in offsets, scrollbar state) | medium | `list.rs`'s tests are the densest in the repo — 8 render-to-buffer assertions covering letter-grouped, plain, series-detail and banner cases. Run them before *and* after A3 and diff the rendered buffers. A3 is its own commit and its own agent run, so it reverts independently of A1/A2. |
| A3's per-kind carrier tempts the executor into "while I'm here" tidying inside the moved bodies | medium → low | Now structural, not disciplinary: the bodies are span-cut and must `diff` empty against the parent, excluding the two permitted return edits (§5). |
| Doc comments / attributes orphaned by cutting from the reported symbol line | **high** | §1a step 1a: extend every start upward through contiguous `///` and `#[…]` lines. Two known cases (`#[derive(Clone)]`, `#[allow(clippy::too_many_arguments)]`) break the build and will be caught; six doc-block cases are **silent** and only this step catches them. |
| Privacy-error batches larger than the brief forecast, stalling a run | medium | ~20 bumps are forecast (§0 Finding 3) but the compiler is authoritative. §1a's escape hatch authorizes widening to `pub(super)` without asking; §7a budgets the rebuild cycles. |
| Post-extraction unused-import warnings break the `-D warnings` bar | high | Expected; budget a `#[cfg(test)]`-gating pass per run (exactly the #365 lane C fixup). This is the main driver of the iteration multiplier in §7a — budgeted, not discovered. |
| `render/mod.rs` declaration-block conflicts across lanes | high, trivial | Additive union merge; integrate in the order given in §2. |
| Test files' `use super::*` stops resolving after production symbols move | high, mechanical | Compiler catches it immediately; fix with explicit imports in the test file, never by re-exporting for tests' sake alone. |
| `sed -i` span deletion applied in ascending order, corrupting later ranges | medium | §1a step 3: delete **highest line number first**. Byte-identity check (§5) catches it if missed. |
| Plan's line-number hints have drifted; executor cuts the wrong span | medium | §1a step 1: resolve every boundary by symbol name in the working tree before cutting. Spans in §3 are hints only. Byte-identity check catches a bad cut. |
| Borrow checker rejects the carrier's `&'a [MediaItem]` alongside `&mut self` | low | Verified: `items` is an owned local clone, not a `self` borrow. |
| Silent scope creep into `render/tests.rs` or into `album.rs` from Lane A | low | Explicit guardrails (§1, §4). |
| ~~Verbatim re-typing silently alters a moved line~~ | **retired** | Eliminated by §1a — moved content never passes through the model. This was the single largest correctness risk in earlier drafts. |

---

## 9. Handoff

**This document is a reference, not a handoff packet.** At ~8.7k tokens it is itself a meaningful fraction of an executor's budget, and most of it is irrelevant to any single run. **Do not paste this plan into an executor.**

Each run gets **only its own standalone brief from §12** (~2k), which is self-contained: it inlines the guardrails, protocol steps, spans, visibility notes, and verification that run needs. Nothing in a brief requires the executor to open this file.

Five runs, each in an isolated git worktree (`worktrees` skill), branch `refactor/368-split-<run>`:

| Run | Brief | Prerequisite | Model-heavy? |
|---|---|---|---|
| A-mech | §12.1 | none | no |
| B | §12.2 | none | no |
| C | §12.3 | none | no |
| D | §12.4 | none | no |
| A3 | §12.5 | A-mech merged | **yes** |

A-mech, B, C, D launch together. A3 launches as a **fresh agent** once A-mech is merged — not a continuation of A-mech, which would carry that run's spent context for no benefit. A3's brief is the only one that includes §0 Finding 5 verbatim: the seam analysis is the substance of that run and must not be re-derived.

Every run's diff goes to an independent `code-reviewer` pass before merge; **no run self-merges**. The reviewer's first check is the byte-identity `diff` in §5 — it is objective and disposes of most of the diff before any judgement is applied.

---

## 10. ADR

**Decision.** Split the four large `src/app/render/*.rs` view files into cohesive sibling modules under `render/`, using plain `mod x;` declarations in `render/mod.rs`, relocating symbols **by mechanical span-cut rather than model reproduction**, and widening visibility only as far as `pub(super)` where the split makes a previously intra-file call cross-module (~20 symbols; see §0 Finding 3). Extract the three remaining inline `#[cfg(test)]` modules first, as separate commits. In `list.rs`, additionally decompose `render_power_list` **along its list-kind seams** — one sibling file per kind of list rendered — as a separate, revertible commit.

**Drivers.** Files of 1,300–1,800 lines exceed the project's 800-line ceiling and the 200–400-line target; #365 established the pattern but its scope only reached `render/mod.rs`; four disjoint files permit real parallelism; 1,317 of the excess lines are test code that a zero-risk move eliminates; and `render_power_list` already contains an explicit, mutually exclusive three-way dispatch that names its own seams.

**Alternatives considered.**
1. *Convert each view into a directory module* (`render/list/mod.rs` + children). Rejected: diverges from the flat-sibling convention #365/#367 established across `src/app/`, and would move rather than split the naming problem.
2. *`#[path]` siblings declared from each view file* (the issue's literal wording), making each lane touch zero shared files. Rejected: `#[path]` is reserved for test files by established convention (#367 §0), and the `render/mod.rs` contention it avoids is a trivial additive merge. Consistency wins.
3. *Extract shared row/header/scrollbar widgets across all four views into one common module.* Rejected: the survey found no genuine cross-view duplication — each view's row rendering is shaped by its own display-row enum. It would be speculative abstraction and would serialize all four lanes behind one shared file.
4. *Split `list.rs` by line count / "first half, second half".* Rejected in favour of the per-kind cut: the kinds are already an exhaustive, mutually exclusive set with a named discriminant, so cutting there produces files that are individually comprehensible and a dispatch that reads as three symmetric arms.
5. *Refactor `render_power_grouped_album_rows` to share a signature with the two new functions.* Rejected: it lives in Lane B's file (cross-lane edit), and album lists genuinely lack the banner/series-detail inputs the other two kinds need — forced symmetry would mean passing dead arguments.
6. *Stop at test extraction only.* Rejected as insufficient: `album.rs` has no tests to extract and `list.rs` would remain at 1,151.
7. *Have the executor read each file and re-type moved symbols ("paste verbatim").* Rejected on two independent grounds: it puts lanes A and B at 100–160k tokens, over the 125k handoff threshold (§7a), and it routes ~5,000 lines of working code through the model where any line can be silently altered. Replaced by shell span-cuts (§1a), which are ~3× cheaper and make verbatim-ness provable via an empty `diff` rather than a reviewer's attention.

**Why chosen.** Maximum size reduction for minimum semantic risk: 3 of 4 lanes and 2 of 3 Lane A commits are pure moves the compiler fully verifies, visibility widening is bounded at `pub(super)` and fully compiler-enforced, and the one genuine code change is isolated in its own revertible commit and its own agent run, cut along a seam the code already declares, behind the repo's densest render test suite. The mechanical protocol makes "move-only" a checkable property rather than a claim, and keeps every run at roughly a third of the context threshold.

**Consequences.** ~15 new files in `src/app/render/` (flat, ~28 files total in that directory) — navigable by prefix (`album_*`, `home_*`, `list_*`, `detail_*`) but the directory is getting wide; a future directory-module conversion becomes more attractive and should be decided repo-wide rather than per-issue. `render/tests.rs` remains at 2,627 lines and becomes the largest file in `render/` — untouched here, tracked separately. `ListRenderCtx` introduces the first explicit render-context struct in `render/`; if the pattern proves out, `home.rs` / `album.rs` could adopt it later. The three list kinds become individually greppable by filename, which should make future list work (a fourth kind, or a change scoped to one kind) cheaper.

**Follow-ups.**
- File the deferred `action.rs` / `input_resolver.rs` issue (§11). Note it is a pure span-cut under §1a and should cost well under 30k tokens.
- The §1a/§1b protocol is reusable and arguably belongs in `docs/agents/` rather than buried in one issue's plan — every future "split a large file" issue wants it. Consider promoting it after this issue validates it.
- Decide the flat-siblings-vs-directory-modules question repo-wide once `src/app/` settles (it is now ~65 flat files).
- `render/tests.rs` (2,627) has no owning issue.
- `render_power_grouped_album_rows` (~508) and `render_series_inline_detail` (~377) are the largest remaining single functions in `render/` after this work.

---

## 11. Deferred — `action.rs` and `input_resolver.rs` (survey findings, for a follow-up issue)

Per user ruling R2 these are **out of scope for #368**. The survey work is recorded here so it is not lost; **the planner has not filed the issue**.

Both files are **purely a test-extraction problem** — no production split is needed for either, which inverts the issue's "lower priority / closer to the 800–1,200 range" framing. They are in fact the cheapest work identified anywhere in this survey.

| File | Now | Production code ends at | After extracting tests |
|---|---:|---|---:|
| `src/app/action.rs` | 1,219 | L535 — `enum Command`, `playback_command_for_key`, `PlaybackHelpBinding`, `PLAYBACK_HELP_BINDINGS`, `help_command_for_key`, `power_album_track_command_for_key`, `App::dispatch` | **536** |
| `src/app/input_resolver.rs` | 910 | L236 — `KeyChord`, `InputContext`, `KeyResolution`, `InputSnapshot`, `help_resolve`, `resolve_key`, `App::input_snapshot`, `ContextEntry`, `CONTEXT_STACK` | **237** |

Suggested shape for the follow-up:
- `action.rs` L536–1219 → `src/app/action_tests.rs` (~684), declared `#[cfg(test)] #[path = "action_tests.rs"] mod tests;`
- `input_resolver.rs` has **two** `#[cfg(test)]` modules (L237–342 unit-level, L343–910 `handle_key` end-to-end) → `src/app/input_resolver_tests.rs` (~106) and `src/app/input_resolver_handle_key_tests.rs` (~568), each with its own `#[path]` declaration. Splitting them keeps both test files under 600.

Naming matches the existing `actions_tests.rs` / `input_*_tests.rs` convention. Neither file requires a `mod` line in `src/app/mod.rs` (test modules are declared inline with `#[path]`), so the work touches **no shared file** and is trivially parallel with anything else in flight. It also finishes #365 step 1's unfinished business.

---

## 12. Standalone executor briefs

Each subsection is self-contained. Hand an executor **exactly one** of these and nothing else from this file.

### 12.0 Shared preamble (inline this into whichever brief you send)

> You are performing a **move-only refactor** of one file in `/home/slatkin/Dev/mbv`, in an isolated git worktree. Work on branch `refactor/368-split-<run>`.
>
> **Do not read the target file in full.** You never need the bodies of the code you are moving. Read only: the file's `use` header (top ~30 lines), a symbol outline, the closing-brace line of each symbol you cut, and any call site you are told to confirm.
>
> Symbol outline command:
> ```
> grep -nE '^\s*(pub(\([a-z:() ]*\))? )?(async )?fn |^\s*(pub(\([a-z:() ]*\))? )?(struct|enum|const|type) |^impl ' <file>
> ```
>
> **Before you start**, confirm `render/tests.rs` holds no path-qualified reference into your file (it is 2,627 lines and out of scope; you must not edit it):
> ```
> grep -nE 'super::(list|album|home|detail)::' src/app/render/tests.rs
> ```
> Expected: no matches. If your file *does* appear, stop and report — a symbol you are moving may be reachable by path from there, and your brief's visibility notes are incomplete.
>
> **Moves are mechanical. Never retype moved code.** For each cluster:
> 1. Resolve each symbol's true start/end line **by symbol name** in the current tree. Line numbers below are hints recorded at commit `9a3e915` and may have drifted — never cut on trust.
> 2. **Extend each start upward** through contiguous preceding `///`, `//!`, and `#[…]` lines, stopping at the first blank line. Symbol-range tools report the *item* line and exclude its docs and attributes; cutting from the reported line silently strips doc blocks and, where an attribute is involved, breaks the build. Your brief flags the known cases, but apply this to **every** symbol — the hints may have drifted.
> 3. `sed -n '<start>,<end>p' <origin> >> <newfile>` — redirect to the file, never print span content to the terminal. Append spans in **ascending source order**; `sed -n 'p'` preserves trailing newlines so consecutive spans concatenate cleanly.
> 4. `sed -i '<start>,<end>d' <origin>` — **highest line number first** so earlier ranges keep their numbering.
> 5. Hand-write only: the new file's `use` header, the `mod <new>;` line in `src/app/render/mod.rs`, and visibility edits.
>
> **Visibility.** Splitting a file turns intra-file private calls into cross-module calls, so expect a batch of privacy errors on the first build — this is normal and your brief's list is a **forecast, not a whitelist**. When the compiler reports one: widen that declaration to `pub(super)` (one line, no body change) and continue; you do not need to ask. For structs and enums, **fields and variants may each need widening individually** — also normal. Never widen beyond `pub(super)`, and never edit a call site to route around a privacy error. If a fix appears to need anything more — a signature change, a body edit, moving a symbol not in your brief, or touching a file outside your lane — **stop and report**; that means the plan is wrong about something, which is more valuable than a workaround. **List every widening you applied in your final report.**
>
> **Before committing, prove byte-identity for every moved span:**
> ```
> git show <parent>:<origin> | sed -n '<orig_start>,<orig_end>p' > /tmp/before
> sed -n '<new_start>,<new_end>p' <newfile>                       > /tmp/after
> diff /tmp/before /tmp/after      # MUST be empty
> ```
> A non-empty diff means you altered content you were told not to. Fix it before committing.
>
> **Rules:** no logic changes, no signature changes, no drive-by cleanups. Never edit a call site to satisfy visibility — widen the moved item instead. No new dependencies. Do not create `render/layout.rs` (collides with `src/app/layout.rs`). Do not touch `render/tests.rs`. Do not touch any file other than your origin, your new files, and the `mod` block of `render/mod.rs`.
>
> **New siblings are declared as plain `mod x;` in `src/app/render/mod.rs`** — `#[path]` is used only for test files. All new files are children of `render`, so `pub(super)` keeps meaning "visible within `render`", exactly as today.
>
> **Verify before review:** `cargo fmt --all -- --check`, `cargo build`, `cargo clippy --all-targets -- -D warnings`, plus your brief's targeted tests. Run tests in the background. Do **not** run the full `cargo test` suite — that is an integration-time step.
>
> **Expect an unused-import cleanup pass.** After extraction, each `use` in the origin falls into one of three cases: **dead** (no consumer left) → delete; **test-only** (used solely by the origin's `#[cfg(test)]` module through `use super::*`) → gate behind `#[cfg(test)]`; **moved** (now used only by one of your new siblings) → delete from the origin and add to that sibling's header. `cargo check --all-targets` must be free of *warnings*, not just errors.
>
> Do not merge your own work. Report: files changed, spans moved, byte-identity results, visibility edits made, and any boundary that had drifted from the hints.

### 12.1 Run A-mech — `render/list.rs`, commits A1 + A2

Two commits, in order. Target: `list.rs` 1,811 → ~985 lines.

**Commit A1 — test extraction.** `list.rs` has exactly one `#[cfg(test)]` occurrence, at line **1152**; its `mod tests {` block runs to the file's final `}` at EOF (**1811**). Confirm both facts, then:
```
sed -n '1152,$p' src/app/render/list.rs > src/app/render/list_tests.rs
sed -i '1152,$d' src/app/render/list.rs
```
Append to `list.rs`:
```rust
#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
```
`list_tests.rs` opens with `use super::*;`. After A2 that glob no longer reaches the moved helpers — add explicit `use super::list_rows::*;` **inside the test file**. Never re-export from `list.rs` purely to satisfy tests.

**Commit A2 — free helpers → `render/list_rows.rs` (~190 lines).** Move these symbols (hints: L**14**–187 — the region opens with a 14-line doc block above `COMPACT_BANNER_RULE_ROWS`; resolve each by name and extend upward per §1a step 1a):
`COMPACT_BANNER_RULE_ROWS`, `COMPACT_BANNER_GAP_ROWS`, `COMPACT_BANNER_INDENT`, `enum DisplayRow`, `push_selected_detail_fillers_before`, `push_selected_detail_fillers_after`, `selected_detail_lower_bound`, `build_list_row_spans`, `render_series_detail_background`.

All are currently module-private free items → make each `pub(super)` (a one-line edit to each signature; do not touch bodies). Add `use super::list_rows::{…}` to `list.rs`.

All are currently module-private free items → each becomes `pub(super)`.

**Do not** decompose `render_power_list` — that is a separate run (A3). Leave it entirely alone. **Do not** touch `render_power_grouped_album_rows`; it lives in `album.rs`, which another run owns.

**Targeted tests:** `cargo test -p mbv app::render::list` and `cargo test -p mbv app::input_power_movie_detail`.

### 12.2 Run B — `render/album.rs`

One commit. No test module in this file. Target: 1,740 → ~520 lines. **This is the heaviest run by volume moved (~1,280 lines) despite being all pure moves** — follow the mechanical protocol strictly; do not read bodies.

Four new files (hints at `9a3e915`; resolve every boundary by symbol name):

| New file | Symbols | ~Lines |
|---|---|---:|
| `render/album_art.rs` | `INLINE_ALBUM_ART_COLS/ROWS/GAP/RIGHT_PAD/RESERVED` (L16–21), `inline_album_art_cache_key` (L23), `inline_art_box_rect` (L32), `enum ArtAnchorX` (L47), `enum ArtAnchorY` (L54), `align_art` (L64), `App::render_inline_album_art` (L1244), `App::render_inline_artist_collage` (L1283), `App::render_inline_art_cell` (L1390) | 315 |
| `render/album_plan.rs` | `enum GroupedAlbumDisplayRow` (**L85**, incl. `#[derive(Clone)]`), `struct GroupedAlbumDisplayPlan` (L104), `impl GroupedAlbumDisplayRow` (L116, both methods), `App::album_artist_label` (L136), `App::build_grouped_album_display_plan` (L144–463) | 395 |
| `render/album_cursor.rs` | `App::` — `selected_power_music_artist_header` (L464), `clear_artist_header_focus` (L471), `set_artist_header_focus` (L477), `move_power_music_group_display_cursor` (L484), `jump_power_music_group_display_cursor` (L549), `selected_artist_header_album_items` (L598), `artist_header_album_items_for_selection` (L607), `page_power_grouped_album_cursor` (L660–735) | 285 |
| `render/album_detail.rs` | `App::render_power_album_detail` (**L1457**, incl. its doc block and `#[allow(clippy::too_many_arguments)]` at L1466); also `INLINE_ALBUM_TITLE_EXTRA_INDENT` / `INLINE_ALBUM_TRACK_EXTRA_INDENT` (L14–15) **only if** they have no other user — grep first | 285 |

Note `album_art.rs` and `album_detail.rs` take **disjoint spans** (a header region plus a tail region) — concatenate in listed order, and delete from the origin highest-first.

**Visibility — expect ~14 widenings; this is the heaviest run for bumps.** Forecast (the compiler is the authority; widen whatever it demands, up to `pub(super)`):

`render_power_grouped_album_rows` stays behind and calls across the new boundary into: `album_artist_label` (136), `build_grouped_album_display_plan` (144), `selected_power_music_artist_header` (464), `render_inline_album_art` (1244), `render_inline_artist_collage` (1283) — all private today. Plus `enum GroupedAlbumDisplayRow` (85, ~25 refs from residual) and its `row_target` (124); `struct GroupedAlbumDisplayPlan` (104) **and all five of its private fields** — `order`, `rows`, `display_cursor`, `selected_artist_header_valid`, `selected_block_bounds`, each read from residual `album.rs`; and consts `INLINE_ALBUM_ART_ROWS` (17), `INLINE_ALBUM_ART_RESERVED` (20).

Field-by-field widening on `GroupedAlbumDisplayPlan` is expected and correct — it shows up as visibility-only hunks, not a logic change.

**Other visibility notes:**
- **Six** of the eight symbols in `album_cursor.rs` carry `pub(in crate::app)` — keep those annotations **verbatim**; `pub(in crate::app)` is path-absolute, so nesting a level deeper changes nothing. The other two are private: `selected_power_music_artist_header` (464) **needs `pub(super)`** (residual `album.rs` calls it), and `set_artist_header_focus` (477) does not (its callers move with it).
- `render_power_album_detail` is `pub(super)`, called only from within `album.rs`; still reaches.
- **`render_power_grouped_album_rows` stays in `album.rs`, signature unchanged.** It is called from `music.rs:143` and `list.rs:540`. Leaving it puts `album.rs` at ~520 lines — that is the **accepted, decided** outcome, not an oversight. Do not decompose it, and do not raise it as a question.

**Targeted tests:** `cargo test -p mbv app::input_power_music_track_focus` and `cargo test -p mbv app::render`.

### 12.3 Run C — `render/home.rs`

Two commits. Target: 1,423 → ~340 lines.

**Commit C1 — test extraction.** One `#[cfg(test)]` occurrence at line **1093**, closing at EOF (**1423**). Confirm, then `sed -n '1093,$p'` → `render/home_tests.rs`, `sed -i '1093,$d'` on the origin, append `#[cfg(test)] #[path = "home_tests.rs"] mod tests;`. The test file's head is `use super::power_home_panel_scroll;` — retarget it to the new module path after C2.

**Commit C2 — three new files:**

| New file | Symbols | ~Lines |
|---|---|---:|
| `render/home_video.rs` | `power_home_panel_scroll` (**L13**, incl. doc block), `MONTHS` (L35), `parse_ymd` (L53), `format_release_date` (L66), `render_home_video_item` (L72–154), `App::render_selected_home_video_detail` (L171), `App::render_power_home_video_list` (L201–331) | 320 |
| `render/home_feed.rs` | `App::render_power_feed_home_video_group_view` (L332–518) | 195 |
| `render/home_hero.rs` | `struct KeepWatchingHeroLayout` (**L157**, incl. 6-line doc block), `App::keep_watching_hero_image_types` (L519), `keep_watching_hero_layout` (L530), `render_keep_watching_hero_image` (L577), `render_keep_watching_hero_meta` (L623–764) | 265 |

**Visibility — expect 6+ widenings.** The Keep Watching hero cluster moves to `home_hero.rs` while `render_power_home_list` stays behind and calls into it: `keep_watching_hero_image_types` (519), `keep_watching_hero_layout` (530), `render_keep_watching_hero_image` (577), `render_keep_watching_hero_meta` (623), and `power_home_panel_scroll` (13) — all private today. Also `struct KeepWatchingHeroLayout` **and its four private fields** (`title_lines`, `show_name`, `overview_lines`, `height`), which residual `home.rs` destructures around L870.

**Other visibility notes:**
- `render_power_home_video_list` and `render_power_feed_home_video_group_view` keep `pub(super)` — dispatched from `power_widgets.rs:555,561`, and `super` is still `render`. **No bump.**
- `render_home_video_item` is a private free fn called from two places, one of which (`home.rs:480`) moves into `home_feed.rs`. Bump it private → `pub(super)` in `home_video.rs`. This is the only visibility change in the run.
- `render_power_home_list` and `render_power_home_section_pills_row` **stay in `home.rs`** (the former is dispatched from `power_widgets.rs:546`).

**Targeted tests:** `cargo test -p mbv app::render::home`.

### 12.4 Run D — `render/detail.rs`

Two commits. Target: 1,354 → ~515 lines.

**Commit D1 — test extraction.** One `#[cfg(test)]` occurrence at line **1029**, closing at EOF (**1354**). Confirm, then `sed -n '1029,$p'` → `render/detail_tests.rs`, `sed -i '1029,$d'`, append `#[cfg(test)] #[path = "detail_tests.rs"] mod tests;`. It uses `use super::*` and references `CompactBannerLayout`, which stays in `detail.rs` — the glob keeps working, no import fix needed.

**Commit D2 — two new files:**

| New file | Symbols | ~Lines |
|---|---|---:|
| `render/detail_series.rs` | `SERIES_DETAIL_DIVIDER_ROWS`, `SERIES_DETAIL_EPISODE_ROWS_ESTIMATE`, `SERIES_DETAIL_OVERVIEW_MAX_LINES`, `SERIES_DETAIL_TRAILING_BLANK_ROWS`, `SERIES_IMAGE_COLS/ROWS/PLACEHOLDER_ROWS` (**L51**–67 — doc blocks at 51–57 and 61–63 sit above these consts), `series_meta_line` (**L68**, incl. doc block), `wrap_overview_lines` (L94), `App::series_selection_state` (L396), `App::series_inline_detail_rows` (L415–468) | 155 |
| `render/detail_series_view.rs` | `App::render_series_inline_detail` (L469–845) | 390 |

**Naming is decided**: `detail_series.rs` + `detail_series_view.rs` as listed. No need to ask.

**Visibility — expect 8 widenings.** Everything moving to `detail_series.rs` is read by `render_series_inline_detail` over in `detail_series_view.rs`, and all of it is private today: `wrap_overview_lines` (94), `series_meta_line` (72), `SERIES_DETAIL_DIVIDER_ROWS` (58), `SERIES_DETAIL_EPISODE_ROWS_ESTIMATE` (59), `SERIES_DETAIL_TRAILING_BLANK_ROWS` (64), `SERIES_IMAGE_COLS` (65), `SERIES_IMAGE_ROWS` (66), `SERIES_IMAGE_PLACEHOLDER_ROWS` (67).

**Other visibility notes:**
- `series_selection_state`, `series_inline_detail_rows`, `render_series_inline_detail` are already `pub(super)` and called from `list.rs`; still reach. **No bump.**
- **`compact_banner_image_cache_key` must stay in `detail.rs`.** `list.rs:2` imports it by path (`use super::detail::compact_banner_image_cache_key;`) — it is the only path-based cross-file dependency in this issue. Moving it would break another run's file. It is 3 lines; leave it.
- `power_selected_movie_item` / `power_selected_series_item` are `pub(crate)` with callers outside `render` — they stay in `detail.rs`.

**Targeted tests:** `cargo test -p mbv app::render::detail` **and** `cargo test -p mbv app::render::list` (list depends on this file's series/banner helpers).

### 12.5 Run A3 — decompose `render_power_list` (fresh agent, after A-mech merges)

The only run that reasons about content. Rebase on merged `main`; `list.rs` should be ~985 lines with `render_power_list` dominating it.

**The seam analysis is settled — do not re-derive it.** `render_power_list` dispatches on two prelude booleans, `show_grouped` and `use_letter_groups`, which are **mutually exclusive by construction** (`use_letter_groups` is defined as `!show_grouped && …`). They yield exactly three list kinds:

| # | Kind | Condition | Branch |
|---|---|---|---|
| 1 | Grouped albums | `library_tab > 0 && is_viewing_album_folders(lib_idx)` | already a one-call delegate to `album.rs` |
| 2 | Letter-grouped | `!grouped && library_tab > 0 && (total ≥ 50 or letter pill active) && collection_type != "music" && not searching` | ~316 lines inline |
| 3 | Plain | everything else | ~271 lines inline |

Kind 3 is a deliberate catch-all — it absorbs the Home "Continue Watching" tab (`library_tab == 0`), search result sets, small libraries, and non-album music levels, all of which render identically. **Do not split kind 3 further.** The `n == 0` early return (placeholder: "Indexing music library…" / "Loading…" / "(empty)") fires *before* the dispatch and is **not** a kind — leave it in the prelude.

**Deliverable — two new files, one per kind:**
- `render/list_letter_groups.rs` — `App::render_power_letter_grouped_rows(…) -> usize`, kind 2's body
- `render/list_plain.rs` — `App::render_power_plain_rows(…) -> usize`, kind 3's body

**Match the existing kind-1 delegate's contract exactly**: a `pub(super) fn` on `impl App` in a sibling file, taking `&mut self, f: &mut Frame, …`, **returning `usize`** (the final scroll offset) assigned to `final_offset`, called from one arm of the existing `if / else if / else`. After this commit the dispatch reads as three structurally identical one-call arms. The prelude and the offset-persisting tail stay in `list.rs` untouched.

**Do not change `render_power_grouped_album_rows`'s signature to match.** It lives in `album.rs` (another run's file), and album lists genuinely lack the banner/series-detail inputs the other two kinds need — forced symmetry would mean passing dead arguments.

**Carrier — 8 fields. Treat this as a starting point, not a specification.** It was assembled by measuring usage of the locals a previous pass had identified, which is not the same as enumerating the bodies' free variables — and it did miss one (`ungrouped_total`, caught in review). **Re-derive the free-variable set directly from the two spans** — every identifier the body references that is bound in the prelude — and reconcile against this list, reporting anything you add. A miss is a compile error, not a silent bug, so this is about cost, not correctness. Do not add fields "for symmetry":
```rust
// in list_rows.rs
pub(super) struct ListRenderCtx<'a> {
    pub(super) content_area: Rect,
    pub(super) items: &'a [mbv_core::api::MediaItem],
    pub(super) cursor: usize,
    pub(super) stored_scroll: usize,
    pub(super) banner_rows: usize,
    pub(super) banner_content_rows: usize,
    pub(super) series_detail_rows: usize,
    pub(super) focused: bool,
}
```
- **Two kind-2-only arguments**, passed to `render_power_letter_grouped_rows` and absent from kind 3's signature: `active_letter_filter`, and `ungrouped_total` (bound at L452–457, read once at L569 in `let bucket_total = if active_letter_filter.is_some() { usize::MAX } else { ungrouped_total };`).
- `n` (`= items.len()`) and `visible` (`= content_area.height as usize`) are derived → **not** carrier fields; recompute them as the first lines of each callee.
- A flat parameter list instead of the struct is acceptable if it reads better. Passing a kind a value it does not read is not.

**Borrow-checker note (verified):** `items` is an owned local `Vec` cloned out of `self.libs[..]`, not a borrow of `self`, so `&'a [MediaItem]` coexists with `&mut self`. No clones or `RefCell` needed.

**Visibility — 2 widenings.** `App::render_series_detail_if_visible` (216) and `App::render_series_detail_top_border` (257) are private and stay in `list.rs`, but the moved bodies call them (the latter at L858 and L1129). Both need `pub(super)`.

**The two branch bodies are still span-cuts, with exactly two in-body edits each.** `final_offset` is declared in the prelude (L536) and assigned **mid-body** — L651 in kind 2, L923 in kind 3 — after which each body runs ~210 more lines, ending in `Self::render_series_detail_top_border(…)` (L858 / L1129). So a body cannot become `-> usize` by pure span-cut. Permitted, and **only** these:
1. `final_offset = offset;` → `let final_offset = offset;`
2. a trailing `final_offset` expression appended just before the closing brace.

Run the byte-identity diff against the parent **with those two lines excluded**; everything else in both bodies must match byte for byte.

**Also delete `let visible = content_area.height as usize;` (L535) from `list.rs`'s prelude** once both bodies have moved — it is used 15 times in kind 2 and 14 in kind 3 and nowhere else, so it becomes dead and fails `-D warnings`.

**The complete set of new/changed lines permitted in this commit:** the `ListRenderCtx` definition; the two `fn` signatures and closing braces; two derived-local lines per callee; the two in-body return edits above; the deletion of the dead `visible` binding; the rewritten dispatch arms; and the two visibility widenings. Anything else in the diff is scope creep.

**Behavioral evidence required:** `list.rs`'s tests are the densest in the repo — 8 render-to-buffer assertions covering letter-grouped, plain, series-detail and banner cases. Capture their rendered output **before** your change and **after**, and diff it. Byte-identity of the bodies is necessary but not sufficient here; the buffer diff is what proves the extraction preserved rendering.

**Targeted tests:** `cargo test -p mbv app::render::list` and `cargo test -p mbv app::input_power_movie_detail`.

Function naming is **decided**: `render_power_letter_grouped_rows` / `render_power_plain_rows`, echoing `render_power_grouped_album_rows`.
