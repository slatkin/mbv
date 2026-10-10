# Spec Delta

## ADDED Requirements

### Requirement: Destination Latest lists load lazily
Every eligible destination SHALL fetch its full Latest list only when that list is selected, and on refresh while it is selected. This includes restoring a destination at launch on its `Latest` pill. No Service SHALL fetch a destination's full Latest list at startup for a destination that is not selected. Feeds Latest makes no fetch of its own, because it is derived from the Feeds entries already loaded for the Feeds tab.

#### Scenario: Podcast library not opened
- **WHEN** mbv starts with two Audiobookshelf podcast libraries and Home as the restored tab
- **THEN** neither podcast library's Latest list is fetched until the user selects its `Latest` pill

#### Scenario: Restored on Latest
- **WHEN** mbv restarts with an Emby Movies library as the restored tab
- **THEN** that library's Latest list is fetched at launch, because restoration selects `Latest`

## MODIFIED Requirements

### Requirement: Latest markers follow their destination pills
The launch window SHALL remain the interval strictly after the previous client launch through the current client launch. Every eligible Latest pill SHALL show an Iris new-content marker when any of that destination's Home `Recently added` items has a valid provider added/published timestamp in that interval. The destination's tab in the tab bar SHALL show the same Iris marker whenever that destination's Latest pill marker would be shown, evaluated from Home's `Recently added` data even while another tab is active and before the destination's Latest list has loaded; adding the tab marker SHALL NOT change any tab's width. The first launch SHALL establish the baseline without markers. Selecting Latest, including when already selected before its items arrive, SHALL acknowledge that destination's marker, on both its pill and its tab, for the rest of the run, keyed by Service-qualified destination identity; refresh and asynchronous replacement SHALL NOT restore it. Items dated after the launch instant SHALL NOT mark during that run.

#### Scenario: New content is visible at its destination
- **WHEN** a library or Feeds has Home `Recently added` items dated inside the launch window and its Latest mode has not been visited
- **THEN** the destination's Latest pill shows the marker and the Continue destination has no corresponding pill

#### Scenario: New content is visible on the library tab
- **WHEN** a library or Feeds has Home `Recently added` items dated inside the launch window, its Latest mode has not been visited, and a different tab is active
- **THEN** that destination's tab in the tab bar shows the Iris marker
- **AND** tabs of destinations with no in-window items, and the Home tab, show no marker
- **AND** every tab keeps the same width it has without a marker

#### Scenario: Unvisited Emby library marks at launch
- **WHEN** an Emby Movies library that has not been opened this run received a movie inside the launch window
- **THEN** its tab shows the Iris marker without its Latest list having been fetched

#### Scenario: Visiting clears marker across refresh
- **WHEN** a user selects a marked Latest pill, or its selected mode receives items asynchronously
- **THEN** the marker clears on both the pill and the destination's tab and remains cleared through subsequent refresh in that run

#### Scenario: Visiting the tab alone does not clear the marker
- **WHEN** a user switches to a marked destination's tab whose entry selection is not Latest, and does not select its Latest pill
- **THEN** the tab and pill markers remain shown

#### Scenario: Window older than fourteen days
- **WHEN** the previous launch was 30 days ago and a library's only in-window item is 20 days old
- **THEN** that library shows no marker, because Home `Recently added` holds only the last 14 days
