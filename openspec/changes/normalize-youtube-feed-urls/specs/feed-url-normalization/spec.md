## Purpose

Resolve a user-provided YouTube channel URL to the canonical RSS feed URL when
adding a feed subscription, so the user can paste the channel page URL instead
of hand-converting it to the `feeds/videos.xml` form.

## ADDED Requirements

### Requirement: YouTube channel URLs resolve to their RSS feed on subscribe

When a user adds a feed subscription, the system SHALL resolve a recognized
YouTube channel URL to its canonical RSS feed URL
(`https://www.youtube.com/feeds/videos.xml?channel_id=<id>`) before fetching
the feed, and SHALL persist the resolved URL as the subscription's URL. URLs
that are not recognized YouTube channel URLs SHALL be used unchanged.

Recognized YouTube host forms include `youtube.com`, `www.youtube.com`, and
`m.youtube.com`.

#### Scenario: Channel URL containing the channel id

- **WHEN** the added URL is `https://www.youtube.com/channel/UC54SLBnD5k5U3Q6N__UjbAw`
- **THEN** the subscription is stored with URL
  `https://www.youtube.com/feeds/videos.xml?channel_id=UC54SLBnD5k5U3Q6N__UjbAw`
- **AND** no network request is made to resolve it

#### Scenario: Handle URL requiring a lookup

- **WHEN** the added URL is `https://www.youtube.com/@ChineseCookingDemystified`
- **THEN** the system fetches the channel page and reads the RSS feed URL it
  advertises
- **AND** the subscription is stored with that resolved
  `feeds/videos.xml?channel_id=…` URL

#### Scenario: Custom and legacy channel URL forms

- **WHEN** the added URL is a `youtube.com/c/<name>` or `youtube.com/user/<name>`
  channel URL
- **THEN** it is resolved by the same channel-page lookup as a handle URL

#### Scenario: A URL that is already a YouTube RSS feed

- **WHEN** the added URL is already
  `https://www.youtube.com/feeds/videos.xml?channel_id=…`
- **THEN** it is stored unchanged and used directly

#### Scenario: Non-YouTube feed URL

- **WHEN** the added URL is any non-YouTube RSS or Atom feed URL
- **THEN** it is stored and fetched unchanged, with no resolution attempted

### Requirement: Resolution failure aborts the add

When a recognized YouTube channel URL cannot be resolved to an RSS feed URL —
because the channel page cannot be fetched or does not advertise a feed URL —
the system SHALL abort the add with an error surfaced to the user and SHALL NOT
save the subscription. The system SHALL NOT fall back to storing the
unresolved channel URL.

#### Scenario: Channel page cannot be resolved

- **WHEN** a handle or custom YouTube URL is added
- **AND** the channel page cannot be fetched or no feed URL is found in it
- **THEN** an error is shown to the user
- **AND** no subscription is added to the configuration
