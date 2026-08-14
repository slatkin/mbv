## MODIFIED Requirements

### Requirement: A Player owner binds only playable items
Owner admission SHALL evaluate every `QueueItem` through canonical media-kind and required-Service classification. An owner SHALL never bind an item whose media kind or required Remote Service capability it cannot play. A daemon Player owner (Local daemon or packaged `mbvd`) SHALL admit Audiobookshelf `QueueItem` variants only when its owner-scoped Audiobookshelf setup is installed and it has negotiated Audiobookshelf transport capability with the submitting client. Existing Composed-to-Bound stripping and explicit-submission behavior SHALL apply at binding without constraining Composed queue editing.

#### Scenario: Audio Feed entry submitted to an audio-only owner
- **WHEN** a Feed entry classified as Audio is submitted to an audio-only owner
- **THEN** it SHALL be eligible for that owner's Bound queue under the same rules as an audio Emby item

#### Scenario: Video Feed entry submitted to an audio-only owner
- **WHEN** a Feed entry classified as Video is explicitly submitted while directly controlling an audio-only owner
- **THEN** it SHALL follow the same local fall-through behavior as a video Emby item
- **AND** SHALL NOT enter the audio-only owner's queue

#### Scenario: Feed MIME is absent
- **WHEN** a Feed entry has no usable enclosure MIME type
- **THEN** its queued snapshot SHALL retain the subscription's `FeedKind` as its canonical media kind

#### Scenario: Owner lacks an item's Remote Service capability
- **WHEN** a queue containing an item from a Remote Service binds to an owner without that Service capability
- **THEN** that item SHALL be unplayable and SHALL NOT enter the owner's Bound queue
- **AND** other playable items SHALL remain eligible

#### Scenario: Audiobookshelf episode submitted to daemon owner with installed setup and transport capability
- **WHEN** an Audiobookshelf podcast episode is submitted to a daemon owner that has installed Audiobookshelf setup and has negotiated Audiobookshelf transport capability
- **THEN** the episode SHALL be eligible for that owner's Bound queue under the same canonical queue semantics as every other admitted QueueItem

#### Scenario: Audiobookshelf episode submitted to daemon owner without installed setup
- **WHEN** an Audiobookshelf podcast episode is submitted to a daemon owner that has no installed Audiobookshelf setup
- **THEN** the submission SHALL fail visibly without Bound queue mutation
