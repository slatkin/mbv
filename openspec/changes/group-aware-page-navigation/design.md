# Design — group-aware page navigation

## Context

`MediaList::delegate_operation` maps `Page(±1)` to `move_selection(±5)` — a fixed selectable-row stride, group-blind. Groups are contiguous `[Spacer, Heading, Item…]` runs built by `letter_grouped_rows`; the cursor indexes only selectable rows, so structural rows can never be selected. See proposal.md for motivation.

## Goals / Non-Goals

- Goals: group-to-group paging on grouped flows, with both directions landing on group starts; one implementation site.
- Non-Goals: changing ungrouped stride, per-destination page configuration, any pill-specific logic, changes to paint-time viewport behavior.

## Decisions

- **Group-awareness lives in the shared owner's `Page` arm** (`MediaList::delegate_operation`), not in destinations or a new operation variant. The owner already holds the full row flow including Headings, and the grouped predicate ("any Heading row") matches the painter's own `grouped_member_striped` test — one definition of "grouped" per module family. Alternatives rejected: a new `PageGroup` operation (a second operation for the same key, callers must know whether the list is grouped — they can't); destination-side snapping (duplicates owner state knowledge outside the one owner).
- **Adjacency resolves by display-row walk.** From the current selectable row: scan forward/backward to the next Heading, then to the first selectable row past it. The scale is pages-of-groups, not offsets, so an index-walk is the simplest correct resolution; rows are at most a few thousand and the walk is pointer-local.
- **Clamps are inherited, not coded.** `move_selection`'s end-clamping already lands "last group → last item" and "first group → first item", because the last/first item of a bounded flow is always the last/first group's boundary. The delta spec's clamp scenarios are regression pins, not new logic.
- **Visual mode inherits unchanged.** `Page` already participates in live-range extension; group-aware landings feed the same extension path. No special case.

## Risks / Trade-offs

- [Paging speed on very long single groups] → a group of 500 items now requires one page per group boundary, not 5-row hops; letter groups are naturally small, and the alternative (hybrid stride) reintroduces the ambiguity this change removes.
- [Ungrouped/grouped boundary drift] → the predicate must match the painter's; both live in the same module family and the boundary table test pins both arms.

## Migration Plan

Single owner change; every grouped destination picks it up on next paint. No persistence, protocol, or config surface. Revert = git revert.

## Open Questions

None.
