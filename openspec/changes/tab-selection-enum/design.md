## Context

See proposal.md — Why. The current state that shapes the refactor:

- **Field:** `pub(super) library_tab: usize` (`app_struct.rs:148`), convention `0 = Home`, `1..=libs.len() = library index`.
- **Existing API** (`cw_library_tab_actions.rs`): `library_tab_count()`, `set_library_tab(idx)`, `library_tab_next()` / `library_tab_prev()` (cycle via `(x + 1) % n` over the whole strip). `render/mod.rs:274,280` applies a pending selection: `self.library_tab = self.library_tab_pending.min(self.libs.len())` and resets to `0`.
- **Read idioms** (~90 sites): `library_tab - 1` (43, "the current library index"), `library_tab > 0` (29, "a library is selected"), `library_tab == 0` (19, "Home"), `checked_sub(1)` (3).
- **Tests** set `app.library_tab = 1` directly in many places.

## Goals / Non-Goals

**Goals:**
- Make "which Emby library is this tab?" a single typed question with one answer site.
- Keep one green, compiling checkpoint — behavior identical, all existing tests pass unchanged in intent.
- Leave #471 a compiler-enforced seam: adding `Feeds` later forces every decision site to account for it.

**Non-Goals:**
- No `Feeds` variant here (that is #471).
- No behavior change — not cycling order, not rendering, not routing.
- No churn to unrelated tab/library logic.

## Decisions

**1. `TabSelection { Home, Library(usize) }`, not a newtype.**
A `TabSelection(usize)` newtype would keep a library index reachable from any value (still subtractable, still misreadable as a library). The enum makes `Library(usize)` the *only* value carrying a library index — that is what makes "feeds read as a library" unrepresentable once `Feeds` is added. `Library(usize)` holds the 0-based index into `libs` directly (not the +1 tab position), so the `- 1` disappears at the type.

**2. One chokepoint accessor: `library_index(&self) -> Option<usize>`.**
`Home => None`, `Library(i) => Some(i)`. Every `library_tab - 1` / `checked_sub(1)` becomes `self.library_index()`; every `library_tab > 0` becomes `self.library_index().is_some()`; every `library_tab == 0` becomes `self.tab.is_home()` (or `library_index().is_none()`). When #471 adds `Feeds`, this one method returns `None` for it, so no scattered site accidentally treats Feeds as a library — the feeds-specific behavior is added deliberately, at the sites that match on the enum.

**3. Keep the public selection API; change only internals.**
`set_library_tab`, `library_tab_next/prev`, `library_tab_count`, and the render/mouse position mapping stay by signature so their ~6 callers and the tests don't churn semantically. Internally they convert between `TabSelection` and a strip *position* (`0 = Home`, `1.. = Library(pos-1)`) via two private helpers `from_position(pos)` / `to_position()`. Position math thus lives in exactly those two helpers — the only place index arithmetic is allowed to remain — instead of at 90 sites.

**4. Migrate tests via the same API, not the raw field.**
Tests currently poke `app.library_tab = 1`. Replace with the selection setter (or a small test helper) so the field can stay private. Where a test truly needs to assert the raw position, go through `to_position()`.

## Risks / Trade-offs

- **A missed arithmetic site keeps compiling** if it reads a position some other way → after the field type changes, every old `usize` use is a type error, so the compiler enumerates the sites; there is no silent survivor. The green build is the completeness proof.
- **Cycling/position bugs from the enum↔position conversion** → `from_position`/`to_position` are the only math; a round-trip unit test (`from_position(to_position(t)) == t` across Home and each library) plus the unchanged next/prev tests cover it.
- **Scope creep into "improve the tab system"** → out of scope; this is a mechanical type migration. Do not reorder tabs, change cycling, or fold in the Feeds variant.

## Migration Plan

Land as one PR. The intermediate states do not compile (the field type changes), so this is not incrementally shippable mid-refactor — the checkpoint is "compiles + all tests green." No data, wire, or protocol surface is touched. Rollback is a straight revert.
