# Spec Delta

## Purpose

Defines what the Emby homevideos feed view's All and folder pills contain, how pill membership is decided, and when their contents load, so the view lists a library's full video history with per-folder pills instead of filtering around watched state.

## ADDED Requirements

### Requirement: The All pill lists every video regardless of watched state

The group view's All selection SHALL list every video under the library root, watched or unwatched, without filtering on played state. The list SHALL order rows oldest-first by date added, regardless of played state.

#### Scenario: Watched video appears in All

- **WHEN** a video under the library root has been marked played
- **THEN** it appears in the All list

#### Scenario: Ordering ignores played state

- **WHEN** the All list loads from a library with both watched and unwatched videos
- **THEN** rows order oldest-first by date added, and played state does not reorder or drop any row

#### Scenario: Loose root videos appear in All

- **WHEN** a video sits directly under the library root rather than inside any folder
- **THEN** it appears in the All list

### Requirement: Every folder holding videos gets a pill

The pill row SHALL include one pill for every folder directly under the library root that contains at least one video, regardless of watched state. A folder containing no videos SHALL NOT get a pill.

#### Scenario: Watched-only folder keeps its pill

- **WHEN** every video inside a folder is marked played
- **THEN** the folder keeps its pill and its list shows those played videos

#### Scenario: Empty folder gets no pill

- **WHEN** a folder directly under the library root contains no videos
- **THEN** no pill appears for that folder

### Requirement: Pill membership is decided without fetching the library's videos

The view SHALL decide pill membership from folder-level video counts carried by, or derived alongside, the library's folder listing. The view SHALL NOT fetch the library's videos merely to classify folders into pills.

#### Scenario: Pills appear without a full-library video fetch

- **WHEN** the folder listing completes
- **THEN** the pill row appears without the view having fetched all videos under the root

### Requirement: Pill contents load lazily on selection

The view SHALL fetch a folder pill's videos when that pill is selected, not before. The All list SHALL load when All is selected. An unloaded selection SHALL show its loading state until its fetch returns.

#### Scenario: Folder pill loads on selection

- **WHEN** the user selects a folder pill for the first time
- **THEN** the view fetches that folder's videos and lists them, and no fetch for that folder happened before the selection

#### Scenario: All loads on selection

- **WHEN** the user selects All
- **THEN** the view fetches the library's videos for the All list at that point if it has not already

#### Scenario: Refresh keeps the lazy shape

- **WHEN** the user refreshes the view after pill contents have loaded
- **THEN** the loaded selection refetches its contents, and unloaded pills remain unloaded

### Requirement: Marking played keeps the row

Marking a video played SHALL NOT remove it from the group view's lists, whether marked from the video's context menu, from a multi-selection, or by the video's playback completing. A later refresh SHALL update the video's played state in place.

#### Scenario: Context-menu mark played keeps the row

- **WHEN** the user marks the selected video played from its context menu
- **THEN** the video stays in the list and shows its played state after refresh

#### Scenario: Playback completion keeps the row

- **WHEN** a video's playback completes and its played state is recorded
- **THEN** the video stays in the group view's lists

### Requirement: The New pill stays independent of the group lists

The group view SHALL keep the destination Latest behavior defined in `destination-latest-modes` for its New pill. The group view's lists SHALL NOT change what the New pill shows, and the New pill SHALL NOT depend on any group list being loaded.

#### Scenario: New pill works before any group load

- **WHEN** the view opens on the New pill and no group list has loaded yet
- **THEN** the New pill still shows its Latest content
