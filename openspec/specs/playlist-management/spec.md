# playlist-management Specification

## Purpose
TBD - created by archiving change playlist-rename-delete. Update Purpose after archive.

## Requirements

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

### Requirement: Play a playlist from the playlists panel

Enter SHALL replace the queue with the playlist's playable items and start playback. In the list view the target is the selected playlist and playback starts at its first item. In the open-playlist view the target is the open playlist and playback starts at the selected item. The queue SHALL stay bound to that playlist. A non-empty queue SHALL show the existing replace-queue confirmation first. Enter SHALL NOT load the playlist without starting playback.

#### Scenario: Enter on a playlist row plays it

- **WHEN** the user presses Enter on a playlist in the list view and the queue is empty
- **THEN** the queue SHALL contain that playlist's playable items in playlist order, with the playlist as its source
- **AND** playback SHALL start at the first item

#### Scenario: Enter on an open-playlist item plays from that item

- **WHEN** the user presses Enter on the third item of an open playlist and the queue is empty
- **THEN** the queue SHALL contain the whole playlist in playlist order, with the playlist as its source
- **AND** playback SHALL start at the third item

#### Scenario: Populated queue asks first

- **WHEN** the user presses Enter on a playlist while the queue has items
- **THEN** the replace-queue confirmation SHALL appear
- **AND** cancelling it SHALL leave the queue and playback unchanged and the panel open

#### Scenario: Empty playlist

- **WHEN** the user presses Enter on a playlist with no playable items
- **THEN** the queue SHALL remain unchanged and the user SHALL be told the playlist has nothing to play

### Requirement: Shuffle a playlist from the playlists panel

`s` in either playlists-panel view SHALL replace the queue with all of the target playlist's playable items in random order and start playback at the first shuffled item. The resulting queue SHALL NOT be bound to the playlist, so its order SHALL never be saved back to the playlist. A non-empty queue SHALL show the existing replace-queue confirmation first.

#### Scenario: Shuffle a playlist row

- **WHEN** the user presses `s` on a playlist in the list view and the queue is empty
- **THEN** the queue SHALL contain every playable item of that playlist in a random order
- **AND** playback SHALL start, and the queue source SHALL be Shuffle, not the playlist

#### Scenario: Shuffle from the open view uses the whole playlist

- **WHEN** the user presses `s` on any item of an open playlist
- **THEN** the shuffled queue SHALL contain every playable item of the open playlist, not only the selected item

#### Scenario: Saved playlist order is untouched

- **WHEN** a playlist is shuffled and the shuffled queue later changes or finishes
- **THEN** the playlist on the server SHALL keep its original order

### Requirement: Enqueue from the playlists panel

`a` SHALL append to the end of the current queue without starting or interrupting playback and without the replace-queue confirmation. In the list view it SHALL append all playable items of the selected playlist in playlist order. In the open-playlist view it SHALL append only the selected item. The current queue's source SHALL NOT change. If that queue is bound to a saved playlist, the appended items SHALL make that playlist dirty, as any other append does.

#### Scenario: Enqueue a whole playlist

- **WHEN** the user presses `a` on a playlist in the list view while another queue is playing
- **THEN** that playlist's playable items SHALL be appended after the existing queue items in playlist order
- **AND** the playing item SHALL keep playing

#### Scenario: Enqueue one item from an open playlist

- **WHEN** the user presses `a` on an item of an open playlist
- **THEN** only that item SHALL be appended to the queue

#### Scenario: Merging into a saved playlist

- **WHEN** the current queue is bound to saved playlist B and the user enqueues playlist A
- **THEN** A's items SHALL be appended to the queue and B SHALL become dirty, so the next save writes the combined list to B

### Requirement: Playlists panel hint bars list the actions

The list-view hint bar SHALL show play, shuffle, enqueue, browse, rename, delete, refresh, and close. The open-playlist hint bar SHALL show play, shuffle, enqueue, back, and close.

#### Scenario: List view hints

- **WHEN** the playlists panel is open in the list view
- **THEN** the hint bar SHALL include `[↵]play [s]shuffle [a]add` with the existing browse, rename, delete, refresh, and close hints

#### Scenario: Open view hints

- **WHEN** a playlist is open in the playlists panel
- **THEN** the hint bar SHALL include `[↵]play [s]shuffle [a]add` with the existing back and close hints

### Requirement: Open-playlist rows match playlist list rows

Each item row in the open-playlist view SHALL be one line high, with the same presentation as a playlist list row. The name SHALL be truncated to fit, never wrapped. Rows at odd absolute indexes SHALL use the playlist zebra stripe, so stripes stay fixed under scroll. The selected row SHALL use the canonical selected-row background and foreground. The item's position number SHALL stay as a muted leading label.

#### Scenario: Long names truncate

- **WHEN** an open playlist contains an item whose name is wider than the panel
- **THEN** that item SHALL occupy one row with its name truncated

#### Scenario: Zebra and selection match the list view

- **WHEN** a playlist is open and its rows are painted
- **THEN** unselected odd rows SHALL have the same stripe background as odd playlist list rows
- **AND** the selected row SHALL have the same background and foreground as a selected playlist list row
