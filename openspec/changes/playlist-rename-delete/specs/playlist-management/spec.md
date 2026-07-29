## ADDED Requirements

### Requirement: Rename playlist from playlists panel

The system SHALL allow the user to rename a playlist from the playlists panel list view by pressing `n`, editing the name in an inline dialog, and confirming with Enter.

#### Scenario: Rename a playlist successfully

- **WHEN** the user presses `n` on a selected playlist in the list view
- **THEN** a name-input dialog opens, pre-filled with the current playlist name
- **AND** the dialog has a block cursor at the end of the pre-filled text
- **AND** the user can edit the name with character input and Backspace
- **WHEN** the user presses Enter
- **THEN** the system SHALL call `POST /Items/{id}` with the new name
- **AND** on success, the playlist list SHALL refresh and a status message SHALL display "Renamed to '<new name>'"
- **AND** the dialog SHALL close

#### Scenario: Rename dialog rejects empty name

- **WHEN** the user presses Enter with an empty or whitespace-only name in the rename dialog
- **THEN** the system SHALL NOT make an API call
- **AND** the dialog SHALL remain open

#### Scenario: Rename dialog cancelled with Esc

- **WHEN** the rename dialog is open and the user presses Esc
- **THEN** the dialog SHALL close without making any API call
- **AND** the playlist name SHALL remain unchanged

#### Scenario: Rename API error

- **WHEN** the rename API call fails (network error, server error)
- **THEN** the system SHALL display a flash status message with the error text
- **AND** the dialog SHALL close
- **AND** the playlist list SHALL still refresh to show the (unchanged) playlist name

### Requirement: Delete playlist from playlists panel

The system SHALL allow the user to delete a playlist from the playlists panel list view by pressing `d`, confirming the action, and having the playlist removed.

#### Scenario: Delete a playlist with confirmation

- **WHEN** the user presses `d` on a selected playlist in the list view
- **THEN** a confirmation modal SHALL display with title "Delete Playlist" and message "Delete playlist '<name>'?"
- **AND** the hint SHALL show "[y] Confirm    [Esc] Cancel"
- **WHEN** the user presses `y`
- **THEN** the system SHALL call `DELETE /Items/{id}`
- **AND** on success, the playlist list SHALL refresh and a status message SHALL display "Deleted '<name>'"
- **AND** the confirmation modal SHALL close

#### Scenario: Delete confirmation cancelled

- **WHEN** the delete confirmation modal is open and the user presses Esc (or any non-`y` key)
- **THEN** the confirmation modal SHALL close
- **AND** no API call SHALL be made
- **AND** the playlist SHALL remain in the list

#### Scenario: Delete API error

- **WHEN** the delete API call fails (network error, server error)
- **THEN** the system SHALL display a flash status message with the error text
- **AND** the confirmation modal SHALL close
- **AND** the playlist list SHALL still refresh to reflect server state

### Requirement: Key bindings for playlist management

The system SHALL bind `n` for rename and `d` for delete exclusively in the playlist list view and SHALL NOT activate these bindings when browsing inside a playlist.

#### Scenario: Keys active only in playlist list view

- **WHEN** the playlists panel is open and no playlist is being browsed (`playlists_open` is `None`)
- **THEN** pressing `n` SHALL open the rename dialog for the selected playlist
- **AND** pressing `d` SHALL open the delete confirmation modal for the selected playlist

#### Scenario: Keys inactive when browsing a playlist

- **WHEN** the playlists panel is open and a playlist is being browsed (`playlists_open` is `Some`)
- **THEN** pressing `n` or `d` SHALL be ignored

#### Scenario: Hint bar reflects new bindings

- **WHEN** the playlists panel is open in list view (not browsing a playlist)
- **THEN** the hint bar SHALL display "[n]rename [d]delete" alongside existing hints

### Requirement: Async rename and delete operations

The system SHALL perform rename and delete API calls on background threads and SHALL notify the main thread via LibEvent variants.

#### Scenario: Rename completes asynchronously

- **WHEN** a rename is initiated (Enter pressed with non-empty name)
- **THEN** a background thread SHALL be spawned to call `rename_playlist()`
- **AND** on completion, the thread SHALL send `LibEvent::PlaylistRenamed { id, new_name }` to the main thread
- **AND** the main thread SHALL update the playlist list and display a status message

#### Scenario: Delete completes asynchronously

- **WHEN** a delete is confirmed (`y` pressed)
- **THEN** a background thread SHALL be spawned to call `delete_playlist()`
- **AND** on completion, the thread SHALL send `LibEvent::PlaylistDeleted { id, name }` to the main thread
- **AND** the main thread SHALL refresh the playlist list and display a status message
