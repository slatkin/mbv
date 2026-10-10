# home-latest-sections Specification

## Purpose

Defines Home's tab content as a Continue Watching destination: Home presents Continue Watching across libraries and retains no Latest selector, Latest rows, or Latest markers, which now live at each eligible destination under `destination-latest-modes`.

## Requirements

### Requirement: Home is presented as Continue with no Selector row
The Home destination's tab SHALL be labelled `Continue`. It presents one flat list with group headers, plus the hero. The headers are `Continue watching` and `Recently added`. `Recently added` has no age sub-headers. Headers use the same style as the group headers of the TV and Feeds lists. Headers are not selectable. Home has no tree, no expand or collapse, no Latest selector, no Selector row, and no pill bar. A header with no items beneath it is omitted. When there are no items at all, the list shows its empty placeholder. A previously saved Home Latest selection falls back to the first Home item on launch. Missing Recently added content does not hide the Continue tab or prevent Emby Continue Watching from loading.

#### Scenario: Home after migration
- **WHEN** the user opens the Continue tab with eligible libraries and feed subscriptions
- **THEN** the tab is labelled `Continue`, shows the `Continue watching` header over its items and the `Recently added` header over its items in one flat list, and has no Latest pills, Selector row, pill bar, or tree indentation

#### Scenario: Legacy Home selection
- **WHEN** the saved launch location names a Home Latest section
- **THEN** the Continue tab opens on its first item without failing or selecting another destination

#### Scenario: Only one root has items
- **WHEN** Emby is unavailable and Feeds has entries from the last 14 days
- **THEN** Home shows only the `Recently added` header over its items and no empty `Continue watching` header

#### Scenario: Cursor skips headers
- **WHEN** the user moves the cursor down past the last `Continue watching` item
- **THEN** the cursor lands on the first item under `Recently added`, not on a header

### Requirement: Continue watching holds at most ten items
The `Continue watching` group SHALL list at most 10 Emby resumable items, in the order Emby returns them.

#### Scenario: More than ten resumable items
- **WHEN** Emby has 15 resumable items
- **THEN** `Continue watching` lists the first 10

### Requirement: Recently added combines every Latest-bearing destination
The `Recently added` group SHALL combine items from every destination that offers a `Latest` pill. Each source SHALL use that destination's own Latest rules. The group SHALL interleave items from all sources newest first by provider added or published date. An item without a valid date SHALL be omitted. A destination with no Latest pill, such as an Emby Music library or an Audiobookshelf book library, SHALL contribute nothing.

#### Scenario: Items from three Services interleave
- **WHEN** an Emby movie was added three days ago, a podcast episode published four days ago, and a feed entry published five days ago
- **THEN** `Recently added` lists the movie, then the podcast episode, then the feed entry

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
- **WHEN** the user marks two items under `Recently added` and enqueues the selection
- **THEN** both items are added to the queue in the order the list shows them

### Requirement: Recently added loads at launch independent of destinations
Home SHALL load each Service's `Recently added` items when that Service becomes ready and on Home refresh. This SHALL not require visiting any destination or loading its full Latest list. Each Service's items SHALL appear when they arrive. A failing or absent Service SHALL contribute nothing without blocking the others. Arriving content SHALL keep the selected row when it is still present.

#### Scenario: One Service is down
- **WHEN** Emby is unreachable and Audiobookshelf and Feeds are ready
- **THEN** `Recently added` lists the podcast and feed items

#### Scenario: Late arrival keeps selection
- **WHEN** the user has selected a `Continue watching` item and Audiobookshelf items arrive
- **THEN** the same item stays selected

### Requirement: Home restoration lands on the first item
On launch, Home SHALL select the first item row of the list, not a header. It SHALL NOT restore a previously selected row.

#### Scenario: Restart on Home
- **WHEN** mbv restarts with Home as the restored tab and Continue Watching has items
- **THEN** the first `Continue watching` item is selected

### Requirement: Recently added is one flat list within the 14-day cutoff
`Recently added` SHALL show its items as one flat list under a single header, with no age sub-headers. An item 0 to 13 days old SHALL be shown. An item 14 days old or older SHALL NOT be shown. The header SHALL be omitted when no item is shown. Age SHALL NOT depend on the previous client launch.

#### Scenario: Cutoff
- **WHEN** items are dated 1, 4, 10, and 20 days ago
- **THEN** the 1-day, 4-day, and 10-day items are shown in one list under `Recently added`, and the 20-day item is absent

#### Scenario: First launch
- **WHEN** mbv has no recorded previous launch and an item was added yesterday
- **THEN** that item is shown under `Recently added`

#### Scenario: Only stale items
- **WHEN** every item is 14 or more days old
- **THEN** `Recently added` shows no header and no items
