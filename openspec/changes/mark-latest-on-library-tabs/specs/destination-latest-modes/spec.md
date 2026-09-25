# Spec Delta

## MODIFIED Requirements

### Requirement: Latest markers follow their destination pills
The launch window SHALL remain the interval strictly after the previous client launch through the current client launch. Every eligible Latest pill SHALL show an Iris new-content marker when any item has a valid provider added/published timestamp in that interval. The destination's tab in the tab bar SHALL show the same Iris marker whenever that destination's Latest pill marker would be shown, evaluated from the destination's already-loaded Latest items even while another tab is active; adding the tab marker SHALL NOT change any tab's width. The first launch SHALL establish the baseline without markers. Selecting Latest, including when already selected before its items arrive, SHALL acknowledge that destination's marker, on both its pill and its tab, for the rest of the run, keyed by Service-qualified destination identity; refresh and asynchronous replacement SHALL NOT restore it. Items dated after the launch instant SHALL NOT mark during that run.

#### Scenario: New content is visible at its destination
- **WHEN** a library or Feeds receives Latest items dated inside the launch window and its Latest mode has not been visited
- **THEN** the destination's Latest pill shows the marker and the Continue destination has no corresponding pill

#### Scenario: New content is visible on the library tab
- **WHEN** a library or Feeds has loaded Latest items dated inside the launch window, its Latest mode has not been visited, and a different tab is active
- **THEN** that destination's tab in the tab bar shows the Iris marker
- **AND** tabs of destinations with no in-window items, and the Home tab, show no marker
- **AND** every tab keeps the same width it has without a marker

#### Scenario: Visiting clears marker across refresh
- **WHEN** a user selects a marked Latest pill, or its selected mode receives items asynchronously
- **THEN** the marker clears on both the pill and the destination's tab and remains cleared through subsequent refresh in that run

#### Scenario: Visiting the tab alone does not clear the marker
- **WHEN** a user switches to a marked destination's tab whose entry selection is not Latest, and does not select its Latest pill
- **THEN** the tab and pill markers remain shown
