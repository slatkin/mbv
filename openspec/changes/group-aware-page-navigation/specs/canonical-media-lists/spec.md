# canonical-media-lists (delta)

## ADDED Requirements

### Requirement: Grouped lists page group to group

A `Page` operation on a grouped row flow (one containing at least one Heading row) SHALL move the selection to the first selectable item of the adjacent group: the next group's first item for a forward page, the previous group's first item for a backward page. A forward page in the last group SHALL clamp to the row flow's last selectable item; a backward page in the first group SHALL clamp to its first selectable item. The same group-to-group landing SHALL apply whatever shape the grouping buckets take, including the letter buckets a range letter-filter pill produces. A page operation on an ungrouped row flow SHALL move the selection by the fixed row stride it uses today.

#### Scenario: Page down lands on the next group's first item

- **WHEN** the selection is on an item of one letter group in a grouped list
- **AND** a forward page operation reaches the shared owner
- **THEN** the selection moves to the first selectable item of the following group
- **AND** no intermediate item of the skipped group becomes selected

#### Scenario: Page up mirrors onto the previous group's first item

- **WHEN** the selection is on an item of one letter group in a grouped list
- **AND** a backward page operation reaches the shared owner
- **THEN** the selection moves to the first selectable item of the preceding group

#### Scenario: Forward page clamps in the last group

- **WHEN** the selection is in the last group of a grouped list
- **AND** a forward page operation reaches the shared owner
- **THEN** the selection moves to the last selectable item of the row flow
- **AND** that item is the last item of that group

#### Scenario: Backward page clamps in the first group

- **WHEN** the selection is in the first group of a grouped list
- **AND** a backward page operation reaches the shared owner
- **THEN** the selection moves to the first selectable item of the row flow

#### Scenario: Range letter-filter pills page across their groups

- **WHEN** a range letter-filter pill is active and the filtered row flow holds several letter groups
- **THEN** page operations move group to group within the filtered flow
- **AND** the last group of the pill clamps to the filtered flow's last selectable item

#### Scenario: Ungrouped lists keep the row stride

- **WHEN** a row flow holds no Heading rows
- **AND** a page operation reaches the shared owner
- **THEN** the selection moves by the same fixed row stride as before this change
