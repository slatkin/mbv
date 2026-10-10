# Spec Delta

## MODIFIED Requirements

### Requirement: Home is presented as Continue with no Selector row
The Home destination's tab SHALL be labelled `Continue`. It presents one tree list with two collapsible roots, `Continue watching` and `Recently added`, plus the hero. It has no Latest selector, no Selector row, and no pill bar. A root with no items is omitted. When both roots are empty, the list shows its empty placeholder. A previously saved Home Latest selection falls back to the Home tree on launch. Missing Recently added content does not hide the Continue tab or prevent Emby Continue Watching from loading.

#### Scenario: Home after migration
- **WHEN** the user opens the Continue tab with eligible libraries and feed subscriptions
- **THEN** the tab is labelled `Continue`, shows the `Continue watching` and `Recently added` roots, and has no Latest pills, Selector row, or pill bar

#### Scenario: Legacy Home selection
- **WHEN** the saved launch location names a Home Latest section
- **THEN** the Continue tab opens on the Home tree without failing or selecting another destination

#### Scenario: Only one root has items
- **WHEN** Emby is unavailable and Feeds has entries from the last 14 days
- **THEN** Home shows only the `Recently added` root and no empty `Continue watching` root

## ADDED Requirements

### Requirement: Continue watching holds at most ten items
The `Continue watching` root SHALL list at most 10 Emby resumable items, in the order Emby returns them.

#### Scenario: More than ten resumable items
- **WHEN** Emby has 15 resumable items
- **THEN** `Continue watching` lists the first 10

### Requirement: Recently added combines every Latest-bearing destination
The `Recently added` root SHALL combine items from every destination that offers a `Latest` pill. Each source SHALL use that destination's own Latest rules. The root SHALL interleave items from all sources newest first by provider added or published date. An item without a valid date SHALL be omitted. A destination with no Latest pill, such as an Emby Music library or an Audiobookshelf book library, SHALL contribute nothing.

#### Scenario: Items from three Services interleave
- **WHEN** an Emby movie was added two days ago, a podcast episode published one day ago, and a feed entry published three days ago
- **THEN** `Recently added` lists the podcast episode, then the movie, then the feed entry

#### Scenario: Ineligible library contributes nothing
- **WHEN** an Emby Music library received an album yesterday
- **THEN** that album does not appear under `Recently added`

#### Scenario: TV follows its Latest rule
- **WHEN** a TV library has a newly added episode that the user already played
- **THEN** that episode does not appear under `Recently added`, as it does not appear in that library's Latest

### Requirement: Recently added is limited to fourteen days and grouped by age
`Recently added` SHALL hold only items dated within 14 days before the current client launch, or later. It SHALL group them into child branches that do not overlap, in this order: `New`, `Last 7 days`, `Last 14 days`. `New` holds items dated after the previous client launch. `Last 7 days` holds the remaining items up to 7 days old. `Last 14 days` holds the remaining items. An empty branch SHALL be omitted. On the first launch, which has no previous launch, there SHALL be no `New` branch.

#### Scenario: Branch assignment
- **WHEN** the previous launch was 2 days ago and items are dated 1, 4, 10, and 20 days ago
- **THEN** the 1-day item is under `New`, the 4-day item under `Last 7 days`, the 10-day item under `Last 14 days`, and the 20-day item is absent

#### Scenario: Previous launch older than fourteen days
- **WHEN** the previous launch was 30 days ago
- **THEN** `New` holds every item from the last 14 days and `Last 7 days` and `Last 14 days` are omitted

#### Scenario: First launch
- **WHEN** mbv has no recorded previous launch
- **THEN** `Recently added` shows only `Last 7 days` and `Last 14 days`

### Requirement: Recently added rows show date and container context
Each `Recently added` item row SHALL show its provider date in the canonical right gutter. It SHALL show its container context and title in the shared split title-parts format. Feed entries SHALL use the configured subscription display name when one is available. These are the same facts the row shows on its destination's Latest list.

#### Scenario: Episode row
- **WHEN** a TV episode added on 9 October appears under `Recently added`
- **THEN** its row shows the show name as context, the episode title, and `Oct 09` in the gutter

### Requirement: Home branches collapse and items act
Every Home root and age branch SHALL be selectable and expandable, and SHALL start expanded at launch. Activating a root or branch SHALL toggle its expansion. Activating an item SHALL play it. Collapse state SHALL last only for the current run. Marking a root or branch SHALL mark the items beneath it.

#### Scenario: Collapse a branch
- **WHEN** the user activates the expanded `Last 14 days` branch
- **THEN** its items are hidden and the branch stays selected

#### Scenario: Mark a branch and enqueue
- **WHEN** the user marks the `New` branch and enqueues the selection
- **THEN** every item under `New` is added to the queue in display order

### Requirement: Home actions resolve the row's own section and Service identity
Every Home action SHALL address the selected row by its section and its Service-qualified identity. The same item SHALL be able to appear under both roots, and each occurrence SHALL be separately selectable. A `Recently added` row's play, enqueue, and played-state actions and its single-row context menu SHALL behave as the same item's row on its destination's Latest list. Remove from Continue Watching SHALL apply only to `Continue watching` rows.

#### Scenario: Same item in both roots
- **WHEN** an in-progress episode is also newly added
- **THEN** it appears under both roots, and selecting one occurrence does not select the other

#### Scenario: Podcast row context menu
- **WHEN** the user opens the context menu on a podcast episode under `Recently added`
- **THEN** the menu offers the same entries as that episode's row on its podcast library's Latest list

#### Scenario: Remove is Continue-only
- **WHEN** the user presses Delete on a `Recently added` row
- **THEN** nothing is removed from Continue Watching

#### Scenario: Mixed-Service multi-selection
- **WHEN** a multi-selection holds an Emby movie and a feed entry
- **THEN** the context menu offers Play and Add to Queue only

### Requirement: Recently added loads at launch independent of destinations
Home SHALL load each Service's `Recently added` items when that Service becomes ready and on Home refresh. This SHALL not require visiting any destination or loading its full Latest list. Each Service's items SHALL appear when they arrive. A failing or absent Service SHALL contribute nothing without blocking the others. Arriving content SHALL keep the selected row when it is still present.

#### Scenario: One Service is down
- **WHEN** Emby is unreachable and Audiobookshelf and Feeds are ready
- **THEN** `Recently added` lists the podcast and feed items

#### Scenario: Late arrival keeps selection
- **WHEN** the user has selected a `Continue watching` item and Audiobookshelf items arrive
- **THEN** the same item stays selected

### Requirement: Home restoration lands on the first item
On launch, Home SHALL select the first item row of the tree, not a root or branch row. It SHALL NOT restore a previously selected row.

#### Scenario: Restart on Home
- **WHEN** mbv restarts with Home as the restored tab and Continue Watching has items
- **THEN** the first `Continue watching` item is selected
