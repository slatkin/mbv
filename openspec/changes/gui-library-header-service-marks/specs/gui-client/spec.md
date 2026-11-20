# Spec Delta

## ADDED Requirements

### Requirement: The list header names a Service destination by its Service mark

The GUI Library list header of a Service destination SHALL show the destination's Service mark at the left of its title line and SHALL show no text title. Emby destinations SHALL show the Emby mark. Audiobookshelf destinations SHALL show the Audiobookshelf mark. Home and the Playlists screen SHALL keep their text title. The mark SHALL NOT change the rest of the header.

#### Scenario: A plain Emby library shows the Emby mark

- **WHEN** the user opens a Movies library tab
- **THEN** the list header shows the Emby mark and no "Movies" title text

#### Scenario: An Emby homevideos feed view shows the Emby mark

- **WHEN** the user opens an Emby library tab in feed-group mode whose content comes from YouTube
- **THEN** the list header shows the Emby mark, not the YouTube mark

#### Scenario: Music shows the Emby mark

- **WHEN** the user opens a Music destination
- **THEN** the list header shows the Emby mark and no "Artists" title text, and the collection dropdown stays at the right end of the line

#### Scenario: Shows shows the Emby mark under every view

- **WHEN** the user picks New in the Shows view dropdown
- **THEN** the list header shows the Emby mark, no title text repeats the view, and the view dropdown stays at the right end of the line

#### Scenario: Audiobookshelf destinations show the Audiobookshelf mark

- **WHEN** the user opens an Audiobooks or a Podcasts destination
- **THEN** the list header shows the Audiobookshelf mark and no title text

#### Scenario: Home keeps its title

- **WHEN** the user opens Home
- **THEN** the list header shows the "Home" title and no Service mark

### Requirement: The Feeds mark follows the checked subscriptions

The Feeds tab's list header SHALL show a Feed platform's mark when the selection is not All and every checked FeedSubscription comes from that one Feed platform. In every other case it SHALL show the RSS mark. The mark SHALL change when the selection changes. A FeedSubscription's Feed platform SHALL come from its URL alone, with no network request.

#### Scenario: Only YouTube subscriptions are checked

- **GIVEN** the Feeds tab has two YouTube subscriptions and one podcast RSS subscription on a generic host
- **WHEN** the user checks both YouTube subscriptions and nothing else
- **THEN** the list header shows the YouTube mark

#### Scenario: All is selected

- **WHEN** the Feeds selection is All
- **THEN** the list header shows the RSS mark, also when every subscription comes from YouTube

#### Scenario: Two platforms are checked

- **WHEN** the user checks one YouTube subscription and one SoundCloud subscription
- **THEN** the list header shows the RSS mark

#### Scenario: A generic subscription is checked

- **WHEN** the user checks only a subscription whose URL shows no known Feed platform
- **THEN** the list header shows the RSS mark

#### Scenario: A PeerTube subscription on any instance

- **WHEN** the user checks only a PeerTube channel feed served from a self-hosted PeerTube instance
- **THEN** the list header shows the PeerTube mark

#### Scenario: The mark follows a selection change

- **GIVEN** only YouTube subscriptions are checked and the header shows the YouTube mark
- **WHEN** the user also checks a podcast RSS subscription on a generic host
- **THEN** the list header shows the RSS mark

## MODIFIED Requirements

### Requirement: The collection dropdown sits in the list header and no pill row remains

The GUI Library panel SHALL show no pill row and no selector row between the top bar and the list in any destination. The dropdown trigger SHALL sit in the list header on the title's line, at the right end of the line, and SHALL show that it opens a menu. The open dropdown SHALL open under its trigger, over the list.

#### Scenario: Podcasts header

- **WHEN** the Podcasts destination paints
- **THEN** the list header's title line shows the Service mark at the left and the dropdown trigger at the right end, and nothing else
- **AND** no row of pills or selector controls shows between the top bar and the list

#### Scenario: No destination paints a pill row

- **WHEN** any destination paints, Home, Shows, and Audiobooks included
- **THEN** no pill row shows above the list

#### Scenario: The dropdown opens under the trigger

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens directly under the trigger, over the list rows
