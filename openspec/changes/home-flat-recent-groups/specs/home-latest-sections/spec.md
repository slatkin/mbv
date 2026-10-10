## MODIFIED Requirements

### Requirement: Home is presented as Continue with no Selector row
The Home destination's tab SHALL be labelled `Continue`. It presents one flat list with group headers, plus the hero. The headers are `Continue watching` and `Recently added`, and under `Recently added`, the age headers `New` and `Recent`. Headers use the same style as the group headers of the TV and Feeds lists. Headers are not selectable. Home has no tree, no expand or collapse, no Latest selector, no Selector row, and no pill bar. A header with no items beneath it is omitted. When there are no items at all, the list shows its empty placeholder. A previously saved Home Latest selection falls back to the first Home item on launch. Missing Recently added content does not hide the Continue tab or prevent Emby Continue Watching from loading.

#### Scenario: Home after migration
- **WHEN** the user opens the Continue tab with eligible libraries and feed subscriptions
- **THEN** the tab is labelled `Continue`, shows the `Continue watching` header over its items and the `Recently added` header over its age groups, and has no Latest pills, Selector row, pill bar, or tree indentation

#### Scenario: Legacy Home selection
- **WHEN** the saved launch location names a Home Latest section
- **THEN** the Continue tab opens on its first item without failing or selecting another destination

#### Scenario: Only one root has items
- **WHEN** Emby is unavailable and Feeds has entries from the last 14 days
- **THEN** Home shows only the `Recently added` header with its age groups and no empty `Continue watching` header

#### Scenario: Cursor skips headers
- **WHEN** the user moves the cursor down past the last `Continue watching` item
- **THEN** the cursor lands on the first item under `Recently added`, not on a header

### Requirement: Continue watching holds at most ten items
The `Continue watching` group SHALL list at most 10 Emby resumable items, in the order Emby returns them.

#### Scenario: More than ten resumable items
- **WHEN** Emby has 15 resumable items
- **THEN** `Continue watching` lists the first 10

### Requirement: Recently added combines every Latest-bearing destination
The `Recently added` group SHALL combine items from every destination that offers a `Latest` pill. Each source SHALL use that destination's own Latest rules. The group SHALL interleave items from all sources newest first by provider added or published date, inside each age group. An item without a valid date SHALL be omitted. A destination with no Latest pill, such as an Emby Music library or an Audiobookshelf book library, SHALL contribute nothing.

#### Scenario: Items from three Services interleave
- **WHEN** an Emby movie was added three days ago, a podcast episode published four days ago, and a feed entry published five days ago
- **THEN** `Recent` lists the movie, then the podcast episode, then the feed entry

#### Scenario: Ineligible library contributes nothing
- **WHEN** an Emby Music library received an album yesterday
- **THEN** that album does not appear under `Recently added`

#### Scenario: TV follows its Latest rule
- **WHEN** a TV library has a newly added episode that the user already played
- **THEN** that episode does not appear under `Recently added`, as it does not appear in that library's Latest

### Requirement: Recently added rows show date and container context
Each `Recently added` item row SHALL show its provider date in the canonical right gutter. It SHALL show its container context and title in the shared split title-parts format. Feed entries SHALL use the configured subscription display name when one is available. These are the same facts the row shows on its destination's Latest list.

#### Scenario: Episode row
- **WHEN** a TV episode added on 9 October appears under `Recently added`
- **THEN** its row shows the show name as context, the episode title, and `Oct 09` in the gutter

### Requirement: Home actions resolve the row's own section and Service identity
Every Home action SHALL address the selected row by its section and its Service-qualified identity. The same item SHALL be able to appear under both `Continue watching` and `Recently added`, and each occurrence SHALL be separately selectable. A `Recently added` row's play, enqueue, and played-state actions and its single-row context menu SHALL behave as the same item's row on its destination's Latest list. Remove from Continue Watching SHALL apply only to `Continue watching` rows. Activating an item SHALL play it. Marking items SHALL work as in other flat lists, and acting on the marks SHALL use display order.

#### Scenario: Same item in both roots
- **WHEN** an in-progress episode is also newly added
- **THEN** it appears under both `Continue watching` and `Recently added`, and selecting one occurrence does not select the other

#### Scenario: Podcast row context menu
- **WHEN** the user opens the context menu on a podcast episode under `Recently added`
- **THEN** the menu offers the same entries as that episode's row on its podcast library's Latest list

#### Scenario: Remove is Continue-only
- **WHEN** the user presses Delete on a `Recently added` row
- **THEN** nothing is removed from Continue Watching

#### Scenario: Mixed-Service multi-selection
- **WHEN** a multi-selection holds an Emby movie and a feed entry
- **THEN** the context menu offers Play and Add to Queue only

#### Scenario: Marked items act in display order
- **WHEN** the user marks two items under `New` and enqueues the selection
- **THEN** both items are added to the queue in the order the list shows them

### Requirement: Home restoration lands on the first item
On launch, Home SHALL select the first item row of the list, not a header. It SHALL NOT restore a previously selected row.

#### Scenario: Restart on Home
- **WHEN** mbv restarts with Home as the restored tab and Continue Watching has items
- **THEN** the first `Continue watching` item is selected

## ADDED Requirements

### Requirement: Recently added is grouped by age with the Feeds rule
`Recently added` SHALL group its items by their age now, with the same rule as the Feeds list. An item 0 or 1 days old SHALL be under `New`. An item 2 to 13 days old SHALL be under `Recent`. An item 14 days old or older SHALL NOT be shown. The groups do not overlap and appear in this order: `New`, `Recent`. An empty group header SHALL be omitted. Age SHALL NOT depend on the previous client launch.

#### Scenario: Group assignment
- **WHEN** items are dated 1, 4, 10, and 20 days ago
- **THEN** the 1-day item is under `New`, the 4-day and 10-day items are under `Recent`, and the 20-day item is absent

#### Scenario: First launch
- **WHEN** mbv has no recorded previous launch and an item was added yesterday
- **THEN** that item is under `New`

#### Scenario: Only New has items
- **WHEN** every item is 0 or 1 days old
- **THEN** `Recently added` shows the `New` header and no `Recent` header

## REMOVED Requirements

### Requirement: Home branches collapse and items act
**Reason**: Home is a flat list with group headers. It has no roots or branches, so nothing expands or collapses.
**Migration**: Item activation, marking, and display-order actions move into "Home actions resolve the row's own section and Service identity".

### Requirement: Recently added is limited to fourteen days and grouped by age
**Reason**: The age groups now follow the Feeds rule, measured from now, with `New` and `Recent` only. The launch-relative `New` branch and the `Last 7 days` and `Last 14 days` branches no longer exist.
**Migration**: See "Recently added is grouped by age with the Feeds rule".
