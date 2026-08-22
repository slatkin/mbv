## Purpose

Determines what a Google Cast receiver is actually given for a queue item — the media URL
it will fetch and the subtitle tracks it will render — and which items cannot be cast at
all.

## ADDED Requirements

### Requirement: The receiver fetches media directly

mbv SHALL provide the receiver with a media URL that the receiver retrieves itself. mbv
SHALL NOT proxy media bytes and SHALL NOT bind a listening network socket to serve media.
Any credential required to retrieve the media SHALL be carried within the URL.

#### Scenario: Source requires authentication

- **WHEN** an item's media origin requires a credential
- **THEN** mbv supplies a URL that carries that credential
- **AND** SHALL NOT rely on request headers that only mbv would have applied

#### Scenario: Credential cannot be carried in the URL

- **WHEN** an item's media can only be retrieved using a request header
- **THEN** mbv SHALL report the item as uncastable

### Requirement: Emby media URLs are negotiated for the receiver

For Emby items, mbv SHALL request playback information using a device profile describing
the receiver's capabilities, and SHALL use the media URL the server returns. mbv SHALL NOT
decide locally whether the item direct-plays or is transcoded.

#### Scenario: Item is directly playable by the receiver

- **WHEN** the server reports the item is playable under the supplied device profile
- **THEN** mbv provides the direct media URL to the receiver

#### Scenario: Item requires transcoding

- **WHEN** the server reports the item is not playable under the supplied device profile
- **THEN** mbv provides the server-supplied transcoding URL to the receiver

#### Scenario: Playback information cannot be obtained

- **WHEN** the request for playback information fails
- **THEN** mbv SHALL report the item as uncastable and SHALL NOT guess a URL

### Requirement: Feed and podcast items use their existing media URLs

For feed entries and Audiobookshelf podcast episodes, mbv SHALL provide the receiver with
the media URL those sources already resolve to, without negotiating an alternative
rendition.

#### Scenario: Feed enclosure is dispatched

- **WHEN** a feed entry with a media URL is dispatched to a cast target
- **THEN** mbv provides that URL to the receiver unchanged

#### Scenario: Feed entry has no media URL

- **WHEN** a feed entry has no media URL that a receiver could retrieve
- **THEN** mbv SHALL report the item as uncastable

#### Scenario: Receiver rejects the media

- **WHEN** the receiver reports it cannot play the supplied media
- **THEN** mbv surfaces the failure to the user and SHALL NOT silently skip the item

### Requirement: Audiobookshelf books are not castable

mbv SHALL report a multi-file Audiobookshelf book as uncastable and SHALL NOT dispatch it
to a cast target. A book's position is defined across its whole timeline, and a receiver
reports position only within the file it is playing, which cannot be written back as a
book position without corrupting the stored resume point.

#### Scenario: Book is played to a cast target

- **WHEN** the user plays an Audiobookshelf book while a cast target is attached
- **THEN** mbv reports the item as uncastable and does not dispatch it
- **AND** SHALL NOT modify the book's stored position

#### Scenario: Selection mixes books and castable items

- **WHEN** a played selection contains both an Audiobookshelf book and castable items
- **THEN** mbv dispatches the castable items and reports the book as uncastable

### Requirement: Uncastable items are surfaced, not silently dropped

When mbv determines an item cannot be cast, it SHALL tell the user which item and why, and
SHALL NOT substitute a different item or fail silently.

#### Scenario: Item is uncastable

- **WHEN** mbv determines an item cannot be dispatched to the attached receiver
- **THEN** mbv presents a message naming the item and the reason

### Requirement: Text subtitles are delivered as sidecar tracks

mbv SHALL deliver text-based subtitles to the receiver as separate subtitle tracks that
the receiver renders, and SHALL allow the user to select or disable them during playback.
Image-based subtitles SHALL instead be rendered into the video by the server.

#### Scenario: Item has text subtitles

- **WHEN** an item with text-based subtitles is dispatched to a cast target
- **THEN** mbv supplies those subtitles as selectable tracks
- **AND** the user can enable or disable them without restarting playback

#### Scenario: Item has image-based subtitles

- **WHEN** an item with image-based subtitles is dispatched with subtitles enabled
- **THEN** mbv requests a rendition with the subtitles rendered into the video

#### Scenario: Item has no subtitles

- **WHEN** an item has no subtitles
- **THEN** mbv supplies no subtitle tracks and SHALL NOT request a rendition change
