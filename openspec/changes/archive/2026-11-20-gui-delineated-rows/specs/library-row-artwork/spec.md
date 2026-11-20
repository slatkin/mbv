# Spec Delta

## Purpose

Defines the art that a GUI Library top-level row shows in its thumb: where the
image comes from, the states the thumb can be in, and how the Client core
fetches and caches row-sized images so that long lists stay fast and correct.

## ADDED Requirements

### Requirement: A top-level row shows its item's Hero image

Every top-level Library row SHALL show an art thumb. The thumb SHALL show the same image as the Hero of that row's item, from the same artwork source. A Music artist row SHALL show the artist's own image when its artist resolves to a Service artist: through the artist identity the Service names, else through the artist the level's tracks name for that artist name, else through the closest Service artist found by name. The row SHALL NOT show the artist's logo or backdrop in place of that image. A Shows Upcoming row, which has no Hero, SHALL show its series' image. A Home row of an Audiobookshelf item SHALL show the same cover as that item's row in its own destination. Nested rows SHALL NOT show a thumb.

#### Scenario: An Upcoming premiere shows its series image

- **WHEN** the Shows Upcoming list paints a premiere row
- **THEN** its thumb shows the image of that premiere's series

#### Scenario: A series row shows the series image

- **WHEN** the Shows list paints a series row
- **THEN** its thumb shows the image that the series' Hero shows

#### Scenario: An episode under a series has no thumb

- **WHEN** a series row is open and its episode rows paint
- **THEN** the episode rows show no thumb

#### Scenario: An artist the Service never names

- **GIVEN** a Music artist row whose artist no Service identity, level track, or name search resolves
- **WHEN** the row paints
- **THEN** its thumb is in the no-source state

### Requirement: The thumb fits the image by its shape

The thumb SHALL keep one fixed landscape box on every top-level row. A landscape image SHALL fill the box. A portrait image SHALL fill the box, cropped. A square image SHALL show whole, with no crop. The shape SHALL come from the image's own pixels once the image is loaded.

#### Scenario: A book cover

- **WHEN** an Audiobooks row's portrait cover loads
- **THEN** the cover fills the thumb, cropped

#### Scenario: A podcast cover

- **WHEN** a Podcasts row's square cover loads
- **THEN** the whole cover shows in the thumb, with no crop

### Requirement: The thumb shows its art state

The thumb SHALL be in one of four states: image, loading, missing (the source gave no image), or no source (the row's kind has no art source). The missing and the no-source states SHALL show the item's media kind, audio or video. A row in the no-source state SHALL make no image request.

#### Scenario: A feed entry

- **WHEN** a Feeds row paints
- **THEN** its thumb is in the no-source state and shows the entry's media kind
- **AND** the Client core makes no image request for the row

#### Scenario: The Service has no image

- **WHEN** the fetch for an Emby movie row's image returns no image
- **THEN** the thumb is in the missing state and shows the video media kind

#### Scenario: The image is on its way

- **WHEN** a row's image fetch is in flight
- **THEN** the thumb is in the loading state

### Requirement: The thumb carries the row's progress

A top-level row with progress SHALL show it along the bottom edge of its thumb, in every art state. A row with no progress SHALL show no progress mark. The row SHALL show no progress mark outside its thumb.

#### Scenario: A partly watched episode

- **WHEN** a Continue watching row at 40% paints
- **THEN** its thumb shows 40% progress along its bottom edge
- **AND** no progress mark shows under the row

#### Scenario: A series row

- **WHEN** a Shows series row paints
- **THEN** its thumb shows no progress mark

### Requirement: A row with little metadata is complete

A top-level row SHALL be complete with its title alone. With no subtitle, the title SHALL be the row's only text line. With no trailing text, the trailing place SHALL stay empty and the title SHALL NOT take its room. A row without art or metadata SHALL keep the same height as every other top-level row.

#### Scenario: A row with only a title

- **WHEN** a row with a title, no subtitle, no trailing text, no progress, and no art source paints
- **THEN** it shows its thumb in the no-source state and its title, at the same height as the rows around it

### Requirement: Row images are fetched at row size

The Client core SHALL fetch a row thumb's image from the Service at a size near the thumb's painted size, separately from the Hero's image, and SHALL cache it under its own key. An Emby request SHALL carry the image tag that the Service declares for that image when it is known, and the cache key SHALL hold that tag. An Audiobookshelf cover request SHALL ask for a row-sized width, and its cache key SHALL hold the library item's last update time when it is known.

#### Scenario: The server art changes

- **GIVEN** a row's Emby image is cached under its image tag
- **WHEN** the Service declares a new tag for that image
- **THEN** the row's next paint requests the image under the new tag

#### Scenario: A row image is cached on disk

- **GIVEN** a row's image is in the disk cache under its current key
- **WHEN** the row paints again after a restart
- **THEN** the image shows with no Service request

#### Scenario: The Hero image is not reused at full size

- **GIVEN** the Hero image of a row's item is cached
- **WHEN** the row paints and its own row image is not cached
- **THEN** the Client core requests the row-sized image

### Requirement: Only rows in view fetch, behind the Hero

The Client core SHALL fetch row images only for the rows the window reports in view, plus a small overscan, and only after navigation stops. A row that leaves the view before its fetch starts SHALL NOT be fetched. The Hero's image fetch SHALL start before any row image fetch that waits.

#### Scenario: A fast scroll

- **WHEN** the user scrolls through a long list without stopping
- **THEN** no row image is fetched until the scroll stops
- **AND** after the scroll stops, only the rows in view and the overscan fetch

#### Scenario: A pick during row fetches

- **GIVEN** row image fetches wait to start
- **WHEN** the user picks a row and its Hero needs an image
- **THEN** the Hero's fetch starts before the waiting row fetches

### Requirement: Row images keep their own memory budget

Row images SHALL use a memory budget separate from the Hero, the queue card, and the other image surfaces. Filling the row budget SHALL NOT evict an image of another surface.

#### Scenario: A long scroll keeps the Hero image

- **GIVEN** the Hero shows a cached image
- **WHEN** the user scrolls through more rows than the row budget holds
- **THEN** the Hero image stays cached
