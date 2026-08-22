## Purpose

Provides a modal overlay that lists constituent items (seasons, tracks, episodes, chapters) for the selected inline-hero surface, activated by Enter and dismissed by Esc, reusing the existing modal-frame vocabulary.

## ADDED Requirements

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

### Requirement: The modal uses the shared modal-frame presentation

The constituent-list modal SHALL use the same frame, backdrop, and focus treatment as other modal overlays. The modal's size SHALL accommodate the item list without excessive empty space. The modal SHALL be centered over the terminal area.

#### Scenario: Modal renders with shared frame

- **WHEN** the constituent-list modal is rendered
- **THEN** it uses the same frame border, backdrop dimming, and centered placement as the confirm and context-menu modals
