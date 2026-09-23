# Spec Delta

## ADDED Requirements

### Requirement: A three-line flat presentation shares the ordered-list seam
A reusable three-line flat list SHALL paint one selectable item as a single three-line unit over the shared ordered row flow. Selection, cursor movement, viewport clamping, content replacement, and completed-frame point resolution SHALL use the existing shared list mechanics and stable opaque targets; no separate index-based list state machine SHALL be introduced. The presentation SHALL accept item-specific text for each of its three lines without prescribing a Service-specific title, metadata, status, or badge vocabulary. It SHALL remain embedded in its mounted parent, which retains gesture recognition, chrome, and translation of target-bearing intents.

#### Scenario: Refresh reorders three-line items
- **WHEN** a three-line list refreshes with the selected stable target still present at a different position
- **THEN** that target remains selected and the viewport is clamped to keep it visible
- **AND** activation still names that target, not the prior position

#### Scenario: Pointer resolves painted content only
- **WHEN** the parent recognizes a click on any of an item's three painted lines
- **THEN** point resolution returns that item's stable target from the most recent completed paint
- **AND** changing content or geometry invalidates the previous hit result until repaint completes

### Requirement: Three-line rows have consistent selection, stripes, and adjustable spacing
The three-line presentation SHALL treat all three lines of an item as one unit for zebra striping and the standard selected-row bar. A focused selected item SHALL paint the bar across its three lines; no list-specific accent rail SHALL be needed. Stripes SHALL follow item order rather than screen position, with the first ungrouped item unstriped and subsequent items alternating, remaining stable through scrolling. An item MAY have a configurable number of blank separator lines after it; separators SHALL stay on the surrounding surface fill, carry no target, and never inherit the stripe or selected bar. The row's selectable geometry SHALL exclude separators. Changing separator spacing SHALL not change content identity or row selection.

#### Scenario: Scrolled cards retain their stripe
- **WHEN** the list scrolls so the same item moves to another screen position
- **THEN** all three lines of that item keep their prior stripe treatment
- **AND** its separator stays on the surrounding surface fill

#### Scenario: Selected card overrides stripe
- **WHEN** a striped item is selected on the focused list
- **THEN** the selected-row bar replaces its stripe across all three content lines
- **AND** no bar or stripe extends into the separator

#### Scenario: Separator spacing changes
- **WHEN** the presentation's separator spacing changes while content remains the same
- **THEN** the same stable item remains selected and click resolution follows the newly painted three-line bounds
- **AND** separator lines do not resolve to an item

#### Scenario: Short viewport does not expose a partial item as a target
- **WHEN** the available list height cannot fit an item's three content lines
- **THEN** that item has no selectable painted hit region in that frame
- **AND** the list remains usable when sufficient height returns
