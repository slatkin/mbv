# Compact Banner Image Pre-warming Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Implementation note (2026-07-19):** the first attempt at this plan ran in a worktree
> branched moments before PR #285 (#263's banner-growth work, including the
> `CompactBannerLayout` struct and `compact_banner_layout()` two-pass split this plan
> assumes) finished auto-merging into `main`. That attempt correctly implemented the
> same design against the *pre-#285* code shape (a single `render_power_compact_detail`
> function, one `cmp_primary` call site, no `CompactBannerLayout` struct) and opened
> PR #288 — which then conflicted with `main` once #285 landed. The branch was reset to
> `origin/main` (post-#285) and Tasks 1–3 below were reapplied by hand against the
> current, correct code shape (`compact_banner_layout()`, two `cmp_primary` call sites)
> instead of attempting an automated rebase across a file #285 had substantially
> rewritten. The task descriptions below are the original plan, written for and matching
> what actually landed.

**Goal:** Pre-fetch the Power View compact movie banner's poster image for nearby movies
around the list cursor, so the image is already cached by the time the cursor reaches
them — closing the gap where every other list-image surface in the app already
prefetches neighbors (`fetch_list_card_image_when_idle`) but the compact banner
(`CompactBannerLayout`, #263) only ever fetches the currently-selected item, cold, on
every cursor move.

**Design doc:** `docs/superpowers/specs/2026-07-19-compact-banner-image-prewarm-design.md`
— read it first for the full problem statement and the reasoning behind the design
choices below. This plan implements that design; if anything here seems to contradict
it, the design doc wins and this plan needs to be corrected, not silently overridden.

**Architecture:** `render_power_list()` (`src/app/render/power/list.rs`) already gathers
the current level's `items: Vec<MediaItem>` and `cursor: usize` before computing the
compact banner's row budget (`compact_banner_rows`). This plan adds a prefetch loop
right after that point: for a movies library, walk a window of
`PREFETCH_AHEAD`/`PREFETCH_BEHIND` items around the cursor (matching the constants
`render/power/card.rs` already uses for the home-card carousel), skip the currently
selected item and any non-leaf-`Movie` entries, and call the existing
`fetch_list_card_image_when_idle()` helper (already idle-gated — no new gating logic)
for each, using the same cache-key format `compact_banner_layout()` already uses today
(`"{item_id}:cmp_primary"`). To keep that cache-key format from drifting between the two
call sites, it's factored into a small shared helper first.

**Tech Stack:** Rust, no new dependencies. Reuses existing `App` methods
(`fetch_list_card_image_when_idle`, `power_selected_movie_item`) and the existing
`MediaItem` type — no new types.

## Global Constraints

- Work in a fresh isolated worktree, per `superpowers:using-git-worktrees` — do not work
  in the shared main checkout at `/home/slatkin/Dev/mbv` (it has a tracked pre-commit
  hook, `core.hooksPath = .githooks`, that blocks direct commits on `main`; this doesn't
  affect feature-branch work in an isolated worktree, but don't touch the main checkout
  regardless). Branch name: `issue-287-banner-image-prewarm`.
- This is issue **#287** on `github.com/slatkin/mbv`. Reference it in commit messages as
  `(#287)`.
- No `Co-Authored-By:` trailers in commit messages.
- `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace
  --all-targets`, and `cargo fmt --all -- --check` must all be clean before each commit
  in this plan. `cargo fmt --all -- --check` is a hard CI gate in this repo — run it
  before committing, not after pushing.
- Do not touch `NAV_IMAGE_FETCH_IDLE_DELAY` (`src/app/images.rs`) or its value — out of
  scope per the design doc, deliberately deferred to a follow-up decision after this
  change ships.
- Do not change any other list-image surface's prefetch behavior (episodes, home,
  album/season grids, legacy table, the home-card carousel itself) — this plan only adds
  a new prefetch loop for the compact movie banner; it does not modify existing ones.
- When you need to find a function by name, use Serena's `find_symbol` or `grep -n "fn
  <name>"` — do not hardcode line numbers, these files are large (`list.rs` and
  `detail.rs` are each 500+ lines) and shift easily.

---

### Task 1: Factor the compact-banner image cache-key format into a shared helper

**Files:**
- Modify: `src/app/render/power/detail.rs` — currently has two separate inline
  `format!("{}:cmp_primary", item.id)` call sites (one inside `compact_banner_layout`,
  one inside a second function further down the file — `grep -n "cmp_primary"
  src/app/render/power/detail.rs` to find both).

**Interfaces:**
- Produces: a new private function, e.g. `fn compact_banner_image_cache_key(item_id:
  &str) -> String`, in `detail.rs` (not `pub`, not `pub(super)` — only used within this
  file). Both existing call sites and the new prefetch loop (Task 2) consume it.

- [ ] **Step 1: Confirm current state**

Run:
```bash
cd <your-worktree>
grep -n "cmp_primary" src/app/render/power/detail.rs
```

Expected: exactly two matches, both of the form `let primary_cache_key = format!("{}:cmp_primary", item.id);`, in two different functions. Read both surrounding functions in full (Serena `find_symbol` with `include_body=true`, or read the file directly) before editing — confirm both use `item.id` (not e.g. a different item's id) as the key input.

- [ ] **Step 2: Add the shared helper**

Add a small free function near the top of `detail.rs` (immediately after the `IMG_COLS`/`IMG_ROWS` constants, before the `CompactBannerLayout` struct, is a reasonable spot — module organization is your judgment call, not prescribed):

```rust
/// Cache key for the compact movie banner's poster image, under which
/// `fetch_card_image`/`fetch_list_card_image_when_idle` store and look up the
/// resized/encoded image state. Shared by the eager fetch in
/// `compact_banner_layout` and the prefetch loop in `list.rs`'s
/// `render_power_list` (#287) so the two can never format the key
/// differently and silently miss each other's cache entries.
fn compact_banner_image_cache_key(item_id: &str) -> String {
    format!("{item_id}:cmp_primary")
}
```

- [ ] **Step 3: Replace both existing inline call sites**

Replace each `format!("{}:cmp_primary", item.id)` with
`compact_banner_image_cache_key(&item.id)`. Do not change anything else about either
function — this is a pure extraction, no behavior change.

- [ ] **Step 4: Build and verify no other cache-key format for this exact string exists**

```bash
cd <your-worktree>
cargo build --workspace 2>&1 | tail -30
grep -rn "cmp_primary" --include="*.rs" src/
```

Expected: clean build; the `grep` now shows three matches — the helper's own
`format!("{item_id}:cmp_primary")` body, and the two call sites now calling the helper
(no more inline `format!` at the call sites themselves).

- [ ] **Step 5: Commit**

```bash
cd <your-worktree>
git add src/app/render/power/detail.rs
git commit -m "refactor: share compact banner image cache-key format (#287)

Extracts the '{item_id}:cmp_primary' cache-key format used by the
compact movie banner's poster image into compact_banner_image_cache_key(),
replacing two separate inline format! call sites. Pure extraction, no
behavior change -- done ahead of #287's prefetch loop so the new call
site can't drift from the existing ones and silently miss the cache."
```

---

### Task 2: Prefetch nearby movies' poster images in `render_power_list`

**Files:**
- Modify: `src/app/render/power/list.rs` — `render_power_list()`. Find the point where
  `banner_rows` is computed (`grep -n "Reserved filler-row count for the compact movie
  banner" src/app/render/power/list.rs`) — the new prefetch loop goes immediately after
  that `let banner_rows: usize = ...` block.

**Interfaces:**
- Consumes: `App::power_selected_movie_item(lib_idx) -> Option<MediaItem>` (existing, in
  `detail.rs`), `App::fetch_list_card_image_when_idle(cache_key: String, item_id:
  String, series_id: String, types: &[&str])` (existing, in `images.rs`),
  `compact_banner_image_cache_key(item_id: &str) -> String` (new, from Task 1 — note
  it's a private free function in `detail.rs`, so it needs `pub(super)` visibility, or
  simply move/duplicate... **do not duplicate it** — change its visibility in Task 1 to
  `pub(super)` if Task 2 needs to call it cross-module. Reconcile this before writing
  Task 2's code: `list.rs` and `detail.rs` are sibling modules under `power/`, so
  `pub(super)` on the helper (visible to `power/`'s parent, i.e. all of `power/`'s
  children including `list.rs`) is the right visibility — go back and fix Task 1's `fn
  compact_banner_image_cache_key` to `pub(super) fn compact_banner_image_cache_key` if
  you wrote it as a bare private `fn`.
- Produces: no new public interface — this is purely a side-effecting loop (triggers
  background fetches), no new state consumed by later tasks.

**Why this shape:** `items`/`cursor` are already in scope at this point in
`render_power_list` (gathered earlier in the same function — see the code around the
`let (items, cursor, stored_scroll, total_count) = ...` block above `banner_rows`).
`self.power_left_tab > 0` at this point means a library (not Home/Continue-Watching) is
selected; `self.power_left_tab - 1` is `lib_idx`. Use
`self.power_selected_movie_item(lib_idx).is_some()` as the gate for "this is a movies
library with a leaf movie selected" — reuse it rather than re-deriving
`collection_type == "movies"` checks, since it's the exact same condition
`compact_banner_rows`/`compact_banner_layout` already use to decide whether a banner
renders at all. If there's no banner, there's nothing to prefetch for.

- [ ] **Step 1: Confirm current state and read the surrounding function**

```bash
cd <your-worktree>
grep -n "Reserved filler-row count for the compact movie banner" src/app/render/power/list.rs
```

Read `render_power_list` in full (Serena `find_symbol` with `include_body=true` on
`impl App[1]/render_power_list` in `src/app/render/power/list.rs`, or read the file
directly around the matched line) — confirm the `items`/`cursor`/`banner_rows` shape
still matches what's described above before writing new code against it.

- [ ] **Step 2: Add the prefetch loop**

Immediately after the `let banner_rows: usize = ...` block (and before the `show_grouped`
computation that follows it), add:

```rust
        // Pre-warm nearby movies' poster images so they're already cached by
        // the time the cursor reaches them (#287) -- mirrors the prefetch
        // window `render_power_card` already uses for the home-card
        // carousel. Only applies when a movie banner is actually showing
        // (i.e. this is a movies library with a leaf Movie selected); if
        // there's no banner, there's nothing to prefetch for.
        if self.power_left_tab > 0 {
            let lib_idx = self.power_left_tab - 1;
            if self.power_selected_movie_item(lib_idx).is_some() {
                const PREFETCH_AHEAD: usize = 3;
                const PREFETCH_BEHIND: usize = 1;
                let start = cursor.saturating_sub(PREFETCH_BEHIND);
                let end = (cursor + PREFETCH_AHEAD + 1).min(items.len());
                let prefetch: Vec<(String, String, String)> = items[start..end]
                    .iter()
                    .enumerate()
                    .filter(|(i, item)| {
                        start + i != cursor && item.item_type == "Movie" && !item.is_folder
                    })
                    .map(|(_, item)| {
                        (
                            compact_banner_image_cache_key(&item.id),
                            item.id.clone(),
                            item.series_id.clone(),
                        )
                    })
                    .collect();
                for (cache_key, item_id, series_id) in prefetch {
                    self.fetch_list_card_image_when_idle(
                        cache_key,
                        item_id,
                        series_id,
                        &["Primary"],
                    );
                }
            }
        }
```

Note: this deliberately collects into a `Vec` before the fetch loop (rather than fetching
inside the same iterator chain that borrows `items`), matching the existing pattern in
`render/power/card.rs`'s own prefetch loop (`grep -n "Collect data first" src/app/render/power/card.rs`
to see the precedent and its comment) — `fetch_list_card_image_when_idle` takes
`&mut self`, and `items` may itself be borrowed from `self.libs[lib_idx]` in some code
paths, so collecting owned `String`s first avoids a borrow conflict.

- [ ] **Step 3: Build**

```bash
cd <your-worktree>
cargo build --workspace 2>&1 | tail -30
```

Expected: `Finished` with no errors. If you get a visibility error on
`compact_banner_image_cache_key`, go back to Task 1 and change it to `pub(super) fn`
(see the Interfaces note above).

- [ ] **Step 4: Commit**

```bash
cd <your-worktree>
git add src/app/render/power/list.rs
git commit -m "feat: pre-warm compact movie banner images for nearby list rows (#287)

render_power_list now prefetches poster images for movies within a
window (3 ahead, 1 behind, matching render_power_card's existing
home-card-carousel prefetch) around the list cursor, via the existing
idle-gated fetch_list_card_image_when_idle helper -- the same
mechanism every other list-image surface in the app already uses. The
currently-selected item's own eager fetch (compact_banner_layout,
uncached until it lands) is unchanged. Only applies when a movies
library has a leaf Movie selected (i.e. the banner is actually
showing); folders and non-movie libraries are unaffected."
```

---

### Task 3: Regression test for the prefetch window

**Files:**
- Modify: `src/app/render/power/list.rs` — add a new `#[test]` inside the existing
  `#[cfg(test)] mod tests { ... }` block. Find a neighboring test that already renders a
  movies-library list with images enabled to model your setup on (`grep -n "fn.*movie"
  src/app/render/power/list.rs` inside the test module, or reuse `push_movie_lib` /
  similar helpers already present in this file's tests — check
  `compact_banner_rows_grows_with_a_longer_overview`'s setup, added by #263, as a
  starting point).

**Interfaces:**
- Consumes: `App::card_image_loading` and/or `App::card_image_states` (existing fields
  used elsewhere in this test module to assert on fetch state — check how existing image
  -related tests in this codebase assert "a fetch was triggered" without a real network
  call; likely by checking `card_image_loading.contains(&cache_key)` after the render,
  since `fetch_card_image` inserts into that set synchronously before spawning the actual
  background fetch thread).
- Produces: nothing consumed by later tasks — last task before final verification.

**Why this shape:** The prefetch loop's observable effect, without a real Emby server, is
that `card_image_loading` (or `card_image_states`, depending on how fast the test
environment's fetch machinery resolves) gains entries for the cache keys of nearby
movies after a single render — even though the cursor never moved to them. Assert on
that directly rather than trying to intercept network calls; this mirrors how
`render/power/card.rs`'s own prefetch loop is (or should be) tested — check for an
existing test of that loop first (`grep -n "prefetch" src/app/render/power/card.rs`
inside its test module) and match its assertion style if one exists, rather than
inventing a new idiom.

- [ ] **Step 1: Write a test asserting the prefetch window is triggered**

Set up a movies-library `App` (images enabled) with at least 5 movies in the list,
position the cursor on item 0, render `render_power_list` once, then assert that
`card_image_loading` (or `card_image_states`) contains entries for the cache keys of
items 1–3 (within the `PREFETCH_AHEAD = 3` window) but does **not** contain an entry for
item 4 (outside the window) or for a non-`Movie` item placed inside the window range (if
your test data includes one) confirming the folder-skip filter works.

Also assert the currently-selected item (item 0) itself still gets its own fetch
triggered — that comes from `compact_banner_layout`'s existing eager fetch (Task 2 does
not change that), so this is a "didn't regress the existing behavior" assertion, not new
coverage.

- [ ] **Step 2: Run it**

```bash
cd <your-worktree>
cargo test --workspace -- render_power_power_list 2>&1 | tail -30
```

(Adjust the test-name filter to whatever you actually named the test.) Expected: PASS.
If it fails, check whether `list_image_fetches_allowed()`'s idle gate (`last_nav_at`) is
blocking the fetch in your test's `App` setup — `make_app_stub()`/similar test
constructors may default `last_nav_at` to "just now," which would make
`list_image_fetches_allowed()` return `false` until `NAV_IMAGE_FETCH_IDLE_DELAY` (150ms)
has actually elapsed. Check how other passing tests of `fetch_list_card_image_when_idle`
-gated behavior handle this (`grep -n "last_nav_at" src/app/images.rs` for the pattern
already used in that file's own tests — likely backdating `last_nav_at` by more than the
delay before rendering, the same way `Task 2`'s design doc note about the debounce
implies).

- [ ] **Step 3: Run the full suite, clippy, and fmt**

```bash
cd <your-worktree>
cargo test --workspace 2>&1 | tail -40
cargo clippy --workspace --all-targets 2>&1 | tail -40
cargo fmt --all -- --check
```

Expected: all tests pass (pre-existing suite plus the new one), zero new clippy
warnings, fmt check clean.

- [ ] **Step 4: Commit**

```bash
cd <your-worktree>
git add src/app/render/power/list.rs
git commit -m "test: cover compact banner image prefetch window (#287)

Regression test for the previous commit: renders a movies-library list
once and asserts nearby movies within the prefetch window have image
fetches triggered (card_image_loading contains their cache keys) while
movies outside the window don't, and the currently-selected item's own
existing eager fetch is unaffected."
```

---

## Final Verification

- [ ] **Full workspace check, one more time, from a clean state**

```bash
cd <your-worktree>
cargo build --workspace 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -50
cargo clippy --workspace --all-targets 2>&1 | tail -40
cargo fmt --all -- --check
git log --oneline -5
git status --short
```

Expected: clean build, all tests pass, no new clippy warnings, fmt check clean, three
new commits (Task 1 refactor, Task 2 feature, Task 3 test) on top of the branch, clean
working tree.

- [ ] **Manual sanity check note (leave for the maintainer, not automatable here)**

This plan's automated tests confirm fetches are *triggered* for nearby items, not that
the visible load-in delay actually improves — that requires a real Emby server and eyes
on the terminal. Leave a note in the PR description asking the maintainer to browse a
movies library in Power View and confirm posters for movies just below/above the cursor
appear noticeably faster than before this change, before merging.

- [ ] **Open the PR**

Reference "Closes #287" in the PR body. Do not push to `main` directly or merge — leave
that for review, per this repo's usual workflow.
