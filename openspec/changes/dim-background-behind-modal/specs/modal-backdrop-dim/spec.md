## ADDED Requirements

### Requirement: Blocking modals dim the background
Whenever a centered, blocking modal overlay (confirm modal, save-playlist dialog, multiselect popup, or library-routes popup) is rendered, the system SHALL darken the previously-rendered content across the full terminal frame before drawing the modal's own border and content on top, so the modal reads as a focused layer above visibly dimmed content.

#### Scenario: Confirm modal is shown
- **WHEN** the confirm modal is open and a frame is rendered
- **THEN** the cells outside the modal's bordered box are rendered darker than they would be without the modal open, and the modal box itself renders at full brightness

#### Scenario: Save-playlist dialog is shown
- **WHEN** the save-playlist name-entry dialog is open and a frame is rendered
- **THEN** the cells outside the dialog's bordered box are rendered darker than they would be without the dialog open, and the dialog box itself renders at full brightness

#### Scenario: Multiselect popup is shown
- **WHEN** a multiselect popup (hidden libraries, hidden latest, feed view libraries, or my languages) is open and a frame is rendered
- **THEN** the cells outside the popup's bordered box are rendered darker than they would be without the popup open, and the popup box itself renders at full brightness

#### Scenario: Library-routes popup is shown
- **WHEN** the library-routes popup is open and a frame is rendered
- **THEN** the cells outside the popup's bordered box are rendered darker than they would be without the popup open, and the popup box itself renders at full brightness

### Requirement: Docked panels and context menu are unaffected
Docked panels (remote sessions, playlists, help, settings) and the small anchored context menu SHALL NOT trigger the background dim treatment, since they are not blocking modals.

#### Scenario: Settings panel is shown alone
- **WHEN** the settings panel is open and no blocking modal is open
- **THEN** the frame renders with no dim treatment applied
