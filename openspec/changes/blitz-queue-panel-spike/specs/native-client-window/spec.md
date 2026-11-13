# Spec Delta

## Purpose
Defines the native window Client spike: a Client of the Owner that presents only the Queue panel in a native window, with its state and artwork supplied in process by the Client core. It exists to measure the native path against the GUI Client, so it is not a full GUI Client.

## ADDED Requirements

### Requirement: The native window is a Client of the Owner process

The native window SHALL be a Client of the Owner process under the same rules as the GUI Client. It SHALL find, start, and attach to the Owner process as the GUI Client does. It SHALL declare itself as a Client that does not take part in Pin swap. It SHALL NOT hold the owner lock, SHALL NOT become a Player owner, and SHALL NOT run playback.

#### Scenario: The window opens with the Owner running

- **WHEN** the native window starts and an Owner process is running
- **THEN** the window attaches to that Owner as a Client and shows the Owner's queue

#### Scenario: The window opens with no Owner running

- **WHEN** the native window starts and no Owner process is running
- **THEN** the window starts the Owner from the sibling `mbv` program and attaches to it

### Requirement: The native window paints only the Queue panel

The native window SHALL show the Queue panel only. The Queue panel SHALL show the queue list, the scrubber, the playback artwork, and the playhead. The window SHALL NOT show any other panel, overlay, or control.

#### Scenario: The window shows the queue

- **WHEN** the Owner's queue holds items
- **THEN** the window lists those items in queue order and marks the active item

#### Scenario: The window shows the scrubber for the playing item

- **WHEN** an item is playing
- **THEN** the window shows the scrubber with the item's chapter marks and the elapsed and remaining times

### Requirement: The window takes its state from the Client core in process

The window SHALL take each value it shows from the section batches that the Client core sends in process. A batch that changes no displayed value SHALL change nothing the window shows. The window SHALL NOT read the Owner, the Emby server, or any network endpoint itself.

#### Scenario: A batch changes the active item

- **WHEN** the core sends a batch whose queue section moves the active slot
- **THEN** the window shows the new active item and its playback state in the same update

#### Scenario: A batch changes nothing shown

- **WHEN** the core sends no batch, or a batch whose sections match what the window shows
- **THEN** the window shows the same values as before

### Requirement: The playhead advances in the window between anchors

While the playing item is not paused, the window SHALL advance the scrubber position from the last playhead anchor, with no new section batch. While paused, the window SHALL hold the position. A new anchor that moves the position away from the prediction SHALL move the scrubber to the anchor's position at once.

#### Scenario: Playback runs with no new batch

- **WHEN** playback runs and the core sends no new anchor
- **THEN** the scrubber position advances with the wall clock

#### Scenario: Playback pauses

- **WHEN** the core sends an anchor that marks playback paused
- **THEN** the scrubber position stops advancing and shows the anchor's position

#### Scenario: A seek moves the playhead

- **WHEN** the core sends an anchor whose position differs from the prediction by more than the playhead tolerance
- **THEN** the scrubber moves to the anchor's position without waiting for the next sample

### Requirement: Artwork comes from the Client core's image cache in process

The window SHALL show artwork that the core's image cache holds. It SHALL request each image through the in-process artwork path and SHALL NOT make a network request for artwork. A reference the cache does not hold SHALL show the bundled placeholder image.

#### Scenario: The playing item has cached artwork

- **WHEN** the playing item's artwork is in the core's image cache
- **THEN** the window shows that image and makes no network request for it

#### Scenario: The playing item has no cached artwork

- **WHEN** the core's image cache does not hold the item's artwork
- **THEN** the window shows the bundled placeholder image

### Requirement: Closing the window ends the core run

The window's close SHALL end the Client core run. The Owner process SHALL keep its own lifetime and SHALL NOT stop because the window closed, except as its own stay-alive rules decide.

#### Scenario: The user closes the window

- **WHEN** the user closes the native window
- **THEN** the Client core run ends and the window exits without a crash or panic
