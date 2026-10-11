# Spec Delta

## MODIFIED Requirements

### Requirement: Latest retains destination-independent item actions and metadata
Each Latest row SHALL retain the media identity, title and container context, provider date in the canonical right gutter when valid, and direct play/enqueue behavior of its former Home Latest row. Audiobookshelf episodes SHALL retain their parent show and description in selected detail; Feed entries SHALL retain the configured subscription display name when available. No Latest activation SHALL change an unrelated destination's cursor or filter. Every destination's Latest list, TV included, SHALL follow the same hero and activation rules (see the requirements below).

#### Scenario: Playing from Latest
- **WHEN** a user plays or enqueues a Latest episode, movie, or Feed entry
- **THEN** the action addresses that displayed item by its own Service identity
- **THEN** other destinations' selections remain unchanged

#### Scenario: Row date and context
- **WHEN** a Latest item has a valid added/published date and container context
- **THEN** its row displays that date in the canonical gutter and its container context in the shared title-parts format
- **WHEN** date or context is unavailable
- **THEN** that absent field is omitted rather than fabricated

## ADDED Requirements

### Requirement: Latest rows play directly in non-Wide geometry
In every geometry where the Wide Hero arrangement does not apply, Enter or a double-click on a selected row of any destination's Latest list SHALL perform that row's play activation at once. No Library Hero overlay SHALL open. Mini view SHALL follow the same rule and SHALL NOT open a hero by itself on selection.

#### Scenario: Narrow Movies Latest plays on Enter
- **WHEN** a Movies library shows `Latest` in non-Wide geometry and the user presses Enter on a selected movie
- **THEN** that movie plays
- **AND** no Library Hero overlay opens

#### Scenario: Narrow Feeds Latest plays on double-click
- **WHEN** Feeds shows `Latest` in non-Wide geometry and the user double-clicks an entry
- **THEN** that entry plays
- **AND** no Library Hero overlay opens

#### Scenario: Other pills keep their overlay entry
- **WHEN** a Movies library shows an alphabet range or `All` in non-Wide geometry and the user presses Enter on a selected movie
- **THEN** the Library Hero overlay opens as before

### Requirement: Wide Latest lists show the selected item's hero
In Wide geometry, every destination's Latest list SHALL show the selected media item's hero in the shared Hero pane, at the same pane geometry on every destination.

#### Scenario: TV and Movies Latest share one Wide layout
- **WHEN** a TV library and a Movies library each show `Latest` in Wide geometry at the same terminal size, each with a row selected
- **THEN** both show the selected item's hero in the Hero pane
- **AND** their list and Hero panes occupy the same cells
