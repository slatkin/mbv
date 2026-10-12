# Design

## Context

The shared Scroll contract has one owner per route:

| Route | Owner |
| --- | --- |
| Canonical list wheel | `crates/mbv-components/src/media_list/tests.rs:187` `a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` — claim → `Scroll`, selection facts untouched, offset by delta |
| Tree wheel | `crates/mbv-components/src/list/tree_browser/tests/structure.rs:343` `a_wheel_scroll_moves_the_viewport_and_keeps_the_selected_node` |
| Claim gate on retained geometry | `crates/mbv-components/src/emby_library_content/tests.rs:181` `wheel_reach_resolves_the_last_painted_selectable_row_as_an_item_index` (task 4.1) |

The three flagged tests each add only *routing* (the gesture reaches the
shared scroll through that component's slot-event path). No
`tick_integration` wheel test covers those routes, so the component tests are
the only direct routing guards. The Home (`interactive-surface-ledger.md:78`)
and Podcast (`:82`) ledger rows already accept that trade: they cite only the
shared `media_list` test with a delegation sentence.

## Goals / Non-Goals

**Goals:** one owner per contract; every ledger Wheel-proof citation resolves
to a surviving test; net test count −3.

**Non-Goals:** adding new tests, changing production code, touching
`mbv-core`/`mbvd`, writing spec deltas (`skip_specs: true`).

## Decisions

1. **Delete all three tests** (user decision, 2026-10-12). The routing-only
   claims do not justify per-surface copies of a shared assertion shape;
   the shared owners keep the Scroll contract itself.
   *Alternative rejected:* keep one slim routing test per claim path — the
   user chose the cleaner suite; the accepted risk is recorded here and in
   the ledger prose.
2. **`#[case]` folding is ruled out.** Invariant 14 rule 4
   (`docs/invariants/14-test-ownership.md:30`) forbids identical-outcome
   cases, and the three fixtures differ in kind (tree context vs book state).
3. **Ledger rows follow the Home/Podcast pattern.** Wide TV row keeps the
   tree `Scroll` citation; the two `tv_content::tests::wheel_tests::…`
   citations become prose ("routes the wheel into its shared owners —
   carrier list or tree — and adds no step of its own"). Books row cites the
   shared `media_list` test with the Podcast-style sentence.
4. **Orphan check before deleting.** `tv_tree_context` is used by four other
   `tv_content/tests/` files; the `book`/`state_with_books` helpers and the
   `finish_view` shape are used by other tests. Nothing else is deleted.

## Risks / Trade-offs

- A future regression that breaks `TvContent` or `BookContent` wheel routing
  (for example, routing the wheel into a selection move again) has no direct
  test. The shared tests still fail if the carrier-level Scroll contract
  breaks; only the component-local forwarding is unguarded.
