## ADDED Requirements

### Requirement: Audiobookshelf destinations fork once by media type
An Audiobookshelf destination SHALL resolve its `media_type` (podcast or book) exactly once, at the point its `TabSelection::AudiobookshelfLibrary(usize)` is resolved. Downstream browse state, renderers, input handlers, help, and context-menu behavior for that destination SHALL branch on the resolved kind and SHALL NOT re-read `media_type` on a subsequent action.

#### Scenario: User selects a book library tab
- **WHEN** the user selects an Audiobookshelf tab whose library `media_type` is `book`
- **THEN** mbv SHALL dispatch book browse behavior for every subsequent action on that tab without re-checking `media_type`

#### Scenario: User selects a podcast library tab
- **WHEN** the user selects an Audiobookshelf tab whose library `media_type` is `podcast`
- **THEN** mbv SHALL dispatch podcast browse behavior for every subsequent action on that tab without re-checking `media_type`

#### Scenario: Book and podcast tabs are both present
- **WHEN** Home, one or more Emby libraries, one or more Audiobookshelf book libraries, one or more Audiobookshelf podcast libraries, and Feeds are visible
- **THEN** each Audiobookshelf tab SHALL retain the browse kind resolved for it regardless of tab order or navigation between tabs
