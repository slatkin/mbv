# audiobookshelf-podcast-browsing Delta

## MODIFIED Requirements

### Requirement: Downloaded episodes use the shared list row presentation

Downloaded podcast episodes SHALL render as selectable rows in the shared media-list presentation with the
same row height, column geometry, truncation, focused and unfocused colors, cursor styling, played and
in-progress semantic state, and available row budget as the Feeds tab's entry rows. Because a view may span
podcasts, every episode row SHALL be a split row carrying its parent podcast's name in the context role
followed by the episode title, with no duration time. Non-selectable age-group heading and spacer rows
SHALL be inserted into the list flow without changing episode indices or stable episode targeting. The
podcast implementation SHALL substitute podcast-native episode data without converting it to an Emby item.

#### Scenario: Active view has episodes

- **WHEN** the active pill view has matching episodes
- **THEN** the list renders one selectable row per episode with provider-native identities, its parent podcast's name, and the episode title
- **AND** age-group heading and spacer rows render without shifting episode targeting

#### Scenario: Active view is empty or loading

- **WHEN** the active pill view has no matching episodes or its episodes are still loading
- **THEN** the list SHALL show its scoped empty or loading state without disturbing the pill row or hero

#### Scenario: A result arrives after selection moved

- **WHEN** episodes arrive after the user has changed the active pill or selection
- **THEN** mbv SHALL NOT replace the current view's rows with content that no longer matches the active pill
