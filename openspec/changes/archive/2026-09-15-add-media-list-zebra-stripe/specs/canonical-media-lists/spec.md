## ADDED Requirements

### Requirement: Wide presentation supports optional zebra striping

The Wide presentation SHALL accept an optional zebra-stripe policy on its paint policy. The policy SHALL carry a focused and an unfocused secondary background colour. When zebra striping is enabled, the painter SHALL apply the secondary background colour to every odd-numbered visible selectable `Item` row, counting only selectable `Item` rows in the visible window by screen-row order (zero-indexed, so the first visible item is even, the second is odd and striped, etc.). Headings and Spacers SHALL always paint with no zebra background regardless of the policy. When zebra striping is disabled (the default), row backgrounds SHALL be unchanged from today's behaviour. The selected row SHALL always use the selected-row background, never the zebra background.

#### Scenario: Zebra stripes alternate among selectable items
- **WHEN** a Wide presentation has zebra striping enabled and renders five visible selectable `Item` rows
- **THEN** the 1st, 3rd, and 5th visible items paint with no secondary background
- **AND** the 2nd and 4th visible items paint with the secondary background colour matching the current focus state

#### Scenario: Headings and spacers are excluded from zebra counting
- **WHEN** a visible Heading or Spacer row appears between two selectable `Item` rows
- **THEN** the Heading or Spacer has no zebra background
- **AND** the item-only counter does not increment for the structural row

#### Scenario: Selected row overrides zebra
- **WHEN** the selected row falls on a zebra-striped position
- **THEN** the selected-row background is used, not the zebra background

#### Scenario: Zebra is screen-row-parity based
- **WHEN** the list scrolls by one row
- **THEN** the second visible selectable item is always striped regardless of its source-row index

#### Scenario: Zebra is disabled by default
- **WHEN** a Wide presentation is configured without a zebra-stripe policy
- **THEN** all unselected rows paint with no explicit background, matching today's behaviour
