## ADDED Requirements

### Requirement: Feeds Wide arrangement is canonical
The Feeds Service/tab Wide panel SHALL use the canonical one-column `WideMediaList` and preserve the accepted `restore-feeds-service-wide-list` (umbrella task 1.3a) rail framing, surface treatment, and selected-row alignment.

#### Scenario: Wide and Narrow use approved variants
- **WHEN** the panel crosses the Wide breakpoint
- **THEN** only the named Wide variant changes placement; Narrow uses `InlineMediaBrowser` as applicable, without changing FeedEntry identity or watched/group state.

### Requirement: Shared hero-on-left arrangement owns the status-row reserve
The shared hero-on-left arrangement primitive SHALL reserve the one status-bar row when it computes the hero and list panes, so every hero-on-left destination inherits the reserve from one place. Screens and components SHALL NOT re-derive the reserve (no per-tab `saturating_sub(1)`, `bottom_pad`, or equivalent) on top of the panes the shared primitive returns.

#### Scenario: Panels leave one blank row above the status bar
- **WHEN** any hero-on-left destination (Home, Feeds, and the non-migrated media tabs that share the primitive) renders in the Wide layout
- **THEN** exactly one blank row separates the bottom of the content panels from the status bar, and that reserve is applied by the shared arrangement primitive rather than the screen.

### Requirement: Other two-column policy is unchanged
This slice SHALL NOT alter non-hero two-column arrangements outside Home and Feeds.

#### Scenario: Unrelated library layout remains stable
- **WHEN** a non-hero library is rendered outside the migrated Home or Feeds destinations
- **THEN** its existing two-column policy and geometry remain unchanged.
