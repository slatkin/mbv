# Spec Delta

## ADDED Requirements

### Requirement: Home remains a Continue Watching destination
Home SHALL present Continue Watching without any Latest selector or Latest rows. A previously saved Home Latest selection SHALL fall back to Continue Watching on launch; the absence of Latest SHALL NOT hide the Home tab or prevent Emby Continue Watching from loading.

#### Scenario: Home after migration
- **WHEN** the user opens Home with eligible libraries and feed subscriptions
- **THEN** Home shows Continue Watching and no Latest pills or rows

#### Scenario: Legacy Home selection
- **WHEN** the saved launch location names a Home Latest section
- **THEN** Home opens on Continue Watching without failing or selecting another destination

## REMOVED Requirements

### Requirement: Latest pills cover Emby, Audiobookshelf podcast libraries, and Feeds
**Reason**: Latest pills now belong to their destinations, not Home.
**Migration**: Use each eligible destination's Latest pill; Home retains Continue Watching.

### Requirement: The last-selected Latest pill is restored on launch
**Reason**: Home no longer has a Latest selector.
**Migration**: A saved Home Latest selection falls back to Continue Watching; destination selector persistence follows each destination's existing rules.

### Requirement: Latest pills populate and refresh independently of Emby's connection state
**Reason**: Home no longer loads Latest sections.
**Migration**: Audiobookshelf podcast and Feeds Latest remain independent of Emby in their own destinations.

### Requirement: Feeds pill reflects the Feeds tab's combined, newest-first entries
**Reason**: The Feeds Latest pill lives in Feeds.
**Migration**: Feeds Latest uses the loaded combined entries, unaffected by the current watched filter.

### Requirement: `hidden_latest` hides pills by name across providers
**Reason**: All eligible destinations expose Latest; the option is sunset.
**Migration**: Existing `hidden_latest` configuration values are ignored and are dropped on configuration save; `hidden_libraries` remains separate.

### Requirement: Selecting and playing a Latest item works uniformly by item provider
**Reason**: Latest item actions move out of Home.
**Migration**: Play or enqueue from the corresponding destination's Latest mode, without touching another destination's selection.

### Requirement: Selected non-Emby Latest item shows a hero detail matching Emby's structure
**Reason**: Selected detail moves to the destination's own Library panel.
**Migration**: Podcast and Feed Latest present detail through their existing destination hero policy.

### Requirement: Home rows render the container context part
**Reason**: Home no longer has Latest rows; Continue Watching remains a Home concern.
**Migration**: Destination Latest rows use the shared title-parts format; Continue Watching keeps its existing presentation.

### Requirement: Home Latest pills identify content new since the previous client launch
**Reason**: The marker moves with each Latest pill.
**Migration**: Destination Latest pills retain the launch-window marker; Home Continue Watching is never marked.

### Requirement: Visiting a Home Latest pill clears its new-content marker
**Reason**: Latest visitation now occurs in the destination.
**Migration**: Selecting a destination Latest pill acknowledges its source for the current run.

### Requirement: Home Latest rows show their provider dates in the canonical gutter
**Reason**: Latest rows move out of Home.
**Migration**: Destination Latest rows retain the canonical date gutter; Continue Watching is unchanged.
