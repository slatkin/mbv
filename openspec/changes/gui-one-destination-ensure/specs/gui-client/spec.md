# Spec Delta

## ADDED Requirements

### Requirement: Every Library destination loads its content from the frame

The GUI Client SHALL load the active Library destination's content from its frame, with no tab click, for every destination kind: Music, Shows, plain Emby libraries, Audiobooks, Podcasts, Feeds, and Home. For each destination the loads SHALL run in one order: the root list, then the selection, then the content that depends on the selection. A destination that has no work for a step SHALL still take that step as a no-op.

#### Scenario: A restored Shows or Emby library loads its root list
- **GIVEN** the GUI Client starts with a Shows or plain Emby library as its saved destination
- **WHEN** the window paints its first frame for that destination
- **THEN** the library's root list load starts with no tab click
- **AND** the default selection's detail loads once the root list lands

#### Scenario: A restored Audiobooks or Podcasts library loads its catalog
- **GIVEN** the GUI Client starts with an Audiobooks or Podcasts library as its saved destination and its catalog is not loaded
- **WHEN** the window paints its first frame for that destination
- **THEN** the catalog load starts with no tab click
- **AND** the selection's loads start after the catalog lands

#### Scenario: A repaint starts no second load
- **GIVEN** a destination's root load is in flight, or its content is loaded
- **WHEN** the frame paints again
- **THEN** no second request for the same content starts

#### Scenario: A failed load is not retried by a repaint
- **GIVEN** a destination's load failed
- **WHEN** the frame paints again
- **THEN** the load is not started again
- **AND** a press on the destination's tab or on its failed collection starts it again

#### Scenario: A tab press and a frame load the same content
- **WHEN** the user presses a destination's tab
- **THEN** the destination loads the same content, in the same order, as the frame does for a restored destination
