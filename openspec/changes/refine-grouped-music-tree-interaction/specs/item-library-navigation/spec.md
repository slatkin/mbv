# Spec Delta

## ADDED Requirements

### Requirement: Grouped Music navigation lands and never silently no-ops

A queue or Search sidebar navigation to a Music item in a Grouped Music library SHALL land the tree on the resolved album with that album's Workspace content available, and SHALL select the navigated track in that Workspace once its tracks are available. The landing SHALL apply its own grouped-catalog state and re-anchor a previously retained Music destination rather than depending on an unrelated later event. A navigation that cannot resolve, activate, or apply a landing SHALL surface the existing library-error feedback and leave the current view unchanged; it SHALL NOT silently do nothing.

#### Scenario: Navigated music track lands in the grouped tree

- **WHEN** the user chooses "Go to Library" on a queued Music track in a library whose Grouped Music tree is active
- **THEN** the Music tab is active with the resolved album selected in the tree
- **AND** the album-track Workspace is available with the navigated track selected once its tracks arrive

#### Scenario: Retained grouped Music destination follows the navigation

- **WHEN** the Grouped Music tree for the target library was already mounted and showing another album
- **THEN** the tree re-anchors to the resolved album and no stale selection remains

#### Scenario: Unresolvable music navigation reports a failure

- **WHEN** a music navigation cannot resolve its album or cannot apply a landing
- **THEN** the existing library-error feedback is shown
- **AND** the current view is unchanged rather than silently no-opped
