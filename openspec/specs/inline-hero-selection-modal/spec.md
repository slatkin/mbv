# inline-hero-selection-modal Specification

## Purpose
Provides a modal overlay that lists constituent items (seasons, tracks, episodes, chapters) for the selected inline-hero surface, activated by Enter and dismissed by Esc, reusing the existing modal-frame vocabulary.

## Requirements

### Requirement: Enter opens a constituent-list modal on inline-hero surfaces

When a surface with constituent items (TV seasons/episodes, Music tracks, podcast episodes, audiobook chapters) is selected in the inline-hero presentation, pressing Enter SHALL open a modal listing those items. The modal SHALL reuse the existing modal-frame vocabulary used by confirm, multiselect, and context-menu overlays. The inline hero SHALL remain visible behind the modal, unchanged in shape or content. Surfaces without constituent items (Movies, Feeds entries, Home items without children) SHALL not open a modal on Enter; Enter SHALL perform the surface's existing activation behavior instead.

#### Scenario: Enter on a surface with constituent items

- **WHEN** the user presses Enter on a selected inline-hero surface that has constituent items
- **THEN** a modal opens listing those items
- **AND** the inline hero remains visible behind the modal with its shape unchanged

#### Scenario: Enter on a surface without constituent items

- **WHEN** the user presses Enter on a selected inline-hero surface that has no constituent items
- **THEN** the surface's existing activation behavior is performed
- **AND** no modal opens

### Requirement: The constituent-list modal supports item selection and cancellation

The modal SHALL list constituent items with their names and any available metadata (duration, episode number, track number). The user SHALL navigate the list with existing movement keys. Pressing Enter on a listed item SHALL select and activate that item according to the surface's existing playback or selection behavior. Pressing Esc or Backspace SHALL cancel the modal and return focus to the library browser without activating any item. The modal SHALL NOT alter the inline hero's content or the library's scroll position on cancellation.

The modal SHALL retain the provider-native identity of the source that opened it and SHALL derive its list from current provider/cache state rather than snapshotting rows at open time. It SHALL represent loading, empty, and ready states explicitly. When matching data completes while the modal is open, the modal SHALL update in place and preserve the cursor by stable constituent-item identity where possible.

#### Scenario: User selects a constituent item

- **WHEN** the user navigates to an item in the modal and presses Enter
- **THEN** that item is selected and activated according to the surface's existing behavior
- **AND** the modal closes

#### Scenario: User cancels the modal

- **WHEN** the user presses Esc or Backspace while the modal is open
- **THEN** the modal closes
- **AND** focus returns to the library browser at the same scroll position
- **AND** no constituent item is activated

#### Scenario: Modal lists items with metadata

- **WHEN** the modal is open for a surface with constituent items
- **THEN** each item shows its name and available metadata (duration, episode number, or track number)
- **AND** the list is scrollable if items exceed the modal's visible area

#### Scenario: Modal data completes while open

- **WHEN** a constituent modal opens before its provider data is available
- **THEN** it shows the shared loading state
- **AND** matching completion replaces loading with ready rows without closing the modal
- **AND** an empty result shows the shared empty state

#### Scenario: Mouse activates a constituent surface

- **WHEN** the user double-clicks the selected parent hero in the inline presentation
- **THEN** the same modal opens as for Enter
- **AND** no invisible wide-only child focus is activated

### Requirement: The modal uses the shared modal-frame presentation

The constituent-list modal SHALL use the same frame, backdrop, and focus treatment as other modal overlays. The modal's size SHALL accommodate the item list without excessive empty space. The modal SHALL be centered over the terminal area.

Every modal SHALL use one bounded width/height policy, one row format, and the shared pill presentation. Each surface SHALL project its already-defined pill model through that presentation; it SHALL NOT define modal-local geometry or styling. The pill row SHALL paint exactly one row with one parent-background spacer and one-row hit targets.

#### Scenario: Modal renders with shared frame

- **WHEN** the constituent-list modal is rendered
- **THEN** it uses the same frame border, backdrop dimming, and centered placement as the confirm and context-menu modals

#### Scenario: Modal renders a screen's defined pills

- **WHEN** a constituent modal has pills defined by its source screen
- **THEN** those labels, IDs, and selection render through the shared pill component
- **AND** the surface does not introduce a pill style, size, spacer, or hit-target variant
