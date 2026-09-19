# Group-aware page navigation

## Why

In a letter-grouped library list, PageDown/ PageUp use a fixed 5-selectable-row stride, so paging lands at arbitrary rows mid-group. The user navigates by letter group; paging should move group to group, matching how the letter pills and group headings already structure the list.

## What Changes

- On a grouped list (a row flow containing Heading rows), `Page` navigation becomes group-aware:
  - PageDown moves the selection to the first item of the next group, clamped to the list's last item.
  - PageUp moves the selection to the first item of the previous group, clamped to the list's first item (mirror semantics; both directions land on group starts).
- Ungrouped lists keep today's ±5-row stride, unchanged.
- The clamp rule covers range letter-filter pills automatically: a range pill yields several groups; in the pill's last group, the list's last item is that group's last item.
- No pill-specific or destination-specific code path; the letter filter's bucket shape does not affect paging (paging works off Heading boundaries in the row flow).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `canonical-media-lists`: new requirement pinning grouped-list page semantics (group-to-group paging with clamps; ungrouped lists keep the row stride), inside the shared owner so every destination inherits it.

## Impact

- `crates/mbv-core` — none. `src/app/components/media_list/mod.rs`: the `MediaListOperation::Page` arm of `MediaList::delegate_operation` gains group-awareness (the canonical owner already holds the full row flow including Heading rows; the grouped predicate matches the painter's `grouped_member_striped` test).
- Inherited by every grouped destination without destination changes: Emby library browser (≥50-item letter grouping and range letter-filter pills), grouped music album lists, any future Heading-bearing flow.
- Paint-time viewport behavior needs no change: the #731 raise rule already walks up over Heading/Spacer rows when re-anchoring, so a group-start landing keeps its heading visible.
- Tests: `#[case]` boundary table on group jumps both directions, clamps at both ends, and the ungrouped stride regression.
