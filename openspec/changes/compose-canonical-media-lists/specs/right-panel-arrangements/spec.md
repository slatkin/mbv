## ADDED Requirements

### Requirement: Responsive hero presentations compose canonical browsers

Every named hero-bearing right-panel destination SHALL compose `WideMediaList` inside its Hero-on-left right rail and `InlineMediaBrowser` inside its selected-row-replacement presentation. The arrangement SHALL supply the child rectangles and shared breakpoint decision. Pills and provider-specific hero or workspace content SHALL remain adjacent parent content and SHALL NOT be absorbed into the media-list control.

This requirement applies to Home, the generic Emby hero-bearing catalog browser, Movies, TV Series browsing, grouped Music album browsing, the Emby homevideos feed view, the Emby podcast channel list, Audiobookshelf Podcast show browsing, Audiobookshelf Book browsing, and Feeds. It does not change the existing two-column policy for non-hero browsers and does not apply the Hero-on-left or Inline presentation contract to Queue.

#### Scenario: A destination uses Hero-on-left

- **WHEN** a named hero-bearing destination meets the shared width and minimum-height conditions
- **THEN** the arrangement places its provider-specific hero or workspace in the left pane
- **AND** it places pills followed by one `WideMediaList` in the right rail
- **AND** no Inline selected-row replacement is painted in that rail

#### Scenario: A destination uses selected-row replacement

- **WHEN** a named hero-bearing destination does not meet both shared Wide geometry conditions
- **THEN** the arrangement places pills followed by one `InlineMediaBrowser`
- **AND** it reserves no separate hero pane

#### Scenario: A non-hero browser uses two columns

- **WHEN** a non-hero browser covered by the existing column policy reaches its two-column breakpoint
- **THEN** it retains that policy
- **AND** the `WideMediaList` one-column contract does not override it

#### Scenario: Audiobookshelf Podcasts is Wide

- **WHEN** an Audiobookshelf Podcast destination enters Hero-on-left
- **THEN** it uses the shared right-pane arrangement delivered by the canonical Music/Audiobookshelf slice
- **AND** its provider-specific episode workspace remains in the left pane

#### Scenario: A parent has additional selectors

- **WHEN** Feeds, Music, Home, or another named destination provides group, watched, section, bucket, or letter selectors
- **THEN** the parent places those selectors in the arrangement's pill area
- **AND** the canonical media-list control receives the remaining list rectangle without owning the selector policy
