## MODIFIED Requirements

### Requirement: Feeds Wide arrangement is canonical
The Feeds Service/tab Wide panel SHALL use the canonical one-column `WideMediaList` and preserve the accepted #623 rail framing, surface treatment, and selected-row alignment.

#### Scenario: Wide and Narrow use approved variants
- **WHEN** the panel crosses the Wide breakpoint
- **THEN** only the named Wide variant changes placement; Narrow uses `InlineMediaBrowser` as applicable, without changing FeedEntry identity or watched/group state.

### Requirement: Other two-column policy is unchanged
This slice SHALL NOT alter non-hero two-column arrangements outside Home and Feeds.

#### Scenario: Unrelated library layout remains stable
- **WHEN** a non-hero library is rendered outside the migrated Home or Feeds destinations
- **THEN** its existing two-column policy and geometry remain unchanged.
