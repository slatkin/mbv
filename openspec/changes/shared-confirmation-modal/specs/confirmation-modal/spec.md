## ADDED Requirements

### Requirement: Shared confirmation modal component
The system SHALL provide a single shared confirmation-modal overlay component that renders a centered, bordered dialog with a title, a message, and a key-binding hint line, and that is reused by every blocking yes/no confirmation prompt in the app.

#### Scenario: Modal renders centered with title, message, and hint
- **WHEN** any confirmation is active (`confirm_modal` is `Some`)
- **THEN** the app renders one centered `Rect` sized to fit within the terminal area, clears the area beneath it, and draws the modal's title, message, and confirm/cancel hint text inside a bordered block

#### Scenario: Only one confirmation modal is shown at a time
- **WHEN** a confirmation modal is already active
- **THEN** triggering another confirmation replaces the active modal's state rather than stacking a second modal on top of it

### Requirement: Confirmation modal visual style matches current overlay design language
The confirmation modal SHALL use the same rounded-border block styling already used by the context-menu and multiselect overlays (`BorderType::Rounded`, border color `palette::IRIS`), rather than the previous `palette::YELLOW`-bordered style.

#### Scenario: Modal border uses the shared overlay color
- **WHEN** the confirmation modal is rendered
- **THEN** its block border is `BorderType::Rounded` styled with `palette::IRIS`, matching `context_menu.rs` and `multiselect.rs`

### Requirement: Clear-queue confirmation uses the shared modal
The system SHALL present the "clear queue" confirmation through the shared confirmation modal instead of a status-bar toast, preserving its existing key bindings and effect.

#### Scenario: Confirming clear queue
- **WHEN** the user presses `c` to clear a non-empty local queue and then presses `y`, `Y`, or `Enter`
- **THEN** the queue is cleared exactly as before, and the confirmation is presented via the shared modal (not `self.status` toast text) until answered

#### Scenario: Cancelling clear queue
- **WHEN** the clear-queue confirmation modal is active and the user presses any key other than `y`/`Y`/`Enter`
- **THEN** the modal is dismissed and the queue is left unchanged

### Requirement: Remove-now-playing-item confirmation uses the shared modal
The system SHALL present the "remove now-playing item from queue" confirmation through the shared confirmation modal instead of a status-bar toast, preserving its existing key bindings and effect.

#### Scenario: Confirming removal of the active queue item
- **WHEN** the user requests removal of the now-playing queue item and then confirms via `y`/`Y`/`Enter`
- **THEN** the item is removed and playback stops exactly as before, and the confirmation is presented via the shared modal until answered

### Requirement: Rescan-library confirmation uses the shared modal
The system SHALL present the "rescan library" confirmation through the shared confirmation modal instead of a status-bar toast, preserving its existing key bindings and effect.

#### Scenario: Confirming a library rescan
- **WHEN** the user triggers a rescan of a library and then confirms via `y`/`Y`/`Enter`
- **THEN** the rescan is triggered for the same library exactly as before, and the confirmation is presented via the shared modal until answered

### Requirement: Save-playlist confirmations use the shared modal
The system SHALL present the unsaved-playlist-changes prompt and the save-playlist overwrite prompt through the shared confirmation modal, replacing their prior bespoke rendering, with no change to their existing key bindings or outcomes.

#### Scenario: Unsaved playlist changes prompt renders via the shared modal
- **WHEN** the user attempts to leave a playlist with unsaved changes
- **THEN** the save/discard/cancel prompt is rendered using the shared confirmation modal component instead of `render_dirty_playlist_modal`'s bespoke rendering

#### Scenario: Overwrite-existing-playlist prompt renders via the shared modal
- **WHEN** saving a playlist whose name collides with an existing playlist
- **THEN** the overwrite/cancel prompt is rendered using the shared confirmation modal component instead of its prior bespoke rendering
