# Design

## Context

Baseline as of 2026-09-26 (`cargo nextest list -p mbv`): 2,096 tests, 289 of them `#[case]` expansions.

| Family (module prefix) | Tests | Budget |
|---|---|---|
| `app::tests::tick_integration` | 290 | ≤ 60 |
| `app::tests::*` (all others: queue 103, music_grouping 53, route_state 45, routing_matrix 41, library_navigate_reveal 38, library_position 37, lifecycle 36, feeds 35, podcast 34, …) | 534 | ≤ 200 |
| `app::components::*` (tests 131, library_panel 121, list 60, music_content 45, tv_content 37, podcast_content 30, media_list 25, …) | 651 | ≤ 230 |
| `app::dispatch::*` | 208 | ≤ 110 |
| `app::shell::*` | 145 | ≤ 70 |
| `app::state::*` | 110 | ≤ 60 |
| `app::infra::*`, `app::input::*`, rest of binary | ~158 | ≤ 70 |
| **Total** | **2,096** | **≤ 800** |

Budgets are ceilings, not targets. Ending lower is fine. Exceeding a family budget is allowed only when the excess tests each own a distinct contract. List them in the task's completion note.

The four-layer ownership matrix in `.agents/skills/mbv-frontend/SKILL.md` ("## Tests") already says which layer owns which claim. This change enforces it by deleting.

## Goals / Non-Goals

**Goals:** hit the budgets; one owning test per contract; no remaining test fails only because of a second copy of a claim.

**Non-Goals:** raising coverage, adding snapshot or property infrastructure, touching `mbv-core`/`mbvd`, changing production behaviour, splitting files for the 800-line cap mid-change (that runs once, pre-PR).

## Decisions

1. **Delete by default; keep by exception.** For each test, keep it only if it is the sole owner of a contract at the right layer, or it is a regression test for a real bug (a commit or issue reference in its name or comment counts). Otherwise delete it. Do not deliberate per test about "port vs delete": if the claim is owned elsewhere, delete it. If it is not owned elsewhere but belongs at a lower layer, write a fresh test there. Never translate the old body.
   *Alternative rejected:* per-test review with a keep/port/delete rationale. That spends effort deliberating instead of cutting, and the previous prune showed it stalls.

2. **Tick integration keeps one test per contract, not per screen.** Pick the simplest screen family as the carrier for each contract (mount, focus handoff, subscription delivery, key routing, mouse routing, latest-frame repaint, projection push, cross-boundary effect, breakpoint transition that retains owner state). Per-screen tests survive only for behaviour that exists on that screen alone (e.g. TV hero overlay narrow vs wide workspace open, flat-episode mini view routing).

3. **`#[case]` tables keep only the boundary.** A wide/narrow/mini table survives only for the breakpoints whose expected outcome differs, and only if that difference is not already an arrangement-test contract. First/middle/last-row tables keep at most one interior case plus a real edge that has distinct logic.

4. **Full-loop state tests move down or die.** A `…_through_tick` test whose assertion is about snapshot, marker, reanchor or queue state is deleted when `app::state`/`app::dispatch` already covers the claim. Otherwise it is replaced by one fresh unit test at that layer.

5. **Docs references follow the survivors.** Every test name cited in `docs/architecture/interactive-surface-ledger.md` must exist after the change. Repoint each citation to the surviving owner, or drop it if the contract is no longer claimed. `docs/plans/*` are historical and are not updated.

6. **Work in family-sized units, sequentially.** Each task group is one family so a single agent session stays small. Groups share no files, but `harness.rs`/shared helpers are cleaned last (group 8) to avoid cross-group churn.

## Risks / Trade-offs

- [A deleted test was the only guard on a real contract] → Decision 1's ownership check. The user tests end to end by hand, and git history keeps every deleted test recoverable.
- [Agents under-cut to be safe] → Hard budgets per family; a group is not done until `cargo nextest list` shows the family at or under budget.
- [Unused helpers trigger clippy `dead_code`] → Delete them (AGENTS.md forbids `allow`). The final group sweeps them.
- [Test files shrink below usefulness and become empty modules] → Delete the empty file and its `mod` line.

## Migration Plan

Test-only. Rollback is `git revert` of the group commits.
