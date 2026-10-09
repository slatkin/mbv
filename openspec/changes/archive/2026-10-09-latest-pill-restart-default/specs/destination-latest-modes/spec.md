# Spec Delta

## MODIFIED Requirements

### Requirement: Every eligible destination offers Latest beside its existing selector choices

Every visible Emby library except Music libraries, regardless of collection kind or size, every Audiobookshelf podcast library, and the Feeds tab SHALL offer a `Latest` pill alongside its existing selector choices. Audiobookshelf book libraries and Emby Music libraries SHALL NOT gain a Latest pill. On launch restoration, every destination whose selector offers a `Latest` pill SHALL select `Latest`; its other pill selections are session memory and SHALL NOT persist across a restart. A Grouped Music library's selected group and an Audiobookshelf book library's selected bucket SHALL keep persisting across restart. The selector SHALL support keyboard cycling and mouse selection over the displayed choices.

#### Scenario: Emby libraries retain their choices
- **WHEN** a user opens a Movie, Home Videos, or other visible Emby library
- **THEN** its selector contains Latest as well as its pre-existing choices
- **THEN** the existing entry selection and existing choices remain available

#### Scenario: Music library has no Latest pill
- **WHEN** a user opens a visible Emby Music library
- **THEN** its selector contains no Latest choice

#### Scenario: Podcast and Feeds retain their choices
- **WHEN** a user opens an Audiobookshelf podcast library or Feeds
- **THEN** its selector contains Latest alongside the existing podcast state/show or Feeds group/filter choices
- **THEN** a book library has no Latest choice

#### Scenario: TV keeps its established default
- **WHEN** mbv restarts with a TV library as the restored tab
- **THEN** the library resolves its established count-dependent default (`Latest` above the pill threshold, `All` at or below it) instead of the mode selected in the previous session

#### Scenario: Restart selects Latest
- **WHEN** mbv restarts after a session in which a Latest-bearing destination's letter range, state pill, show pill, or Feeds filter or group was selected
- **THEN** that destination's selector SHALL open on `Latest` when it is the restored tab
- **THEN** the non-Latest pill selection SHALL NOT reappear in any later session

#### Scenario: Music and book pills keep persisting
- **WHEN** mbv restarts after a session in which a Grouped Music group or an Audiobookshelf book bucket was selected on the exit tab
- **THEN** the recorded group or bucket SHALL restore through the launch snapshot
