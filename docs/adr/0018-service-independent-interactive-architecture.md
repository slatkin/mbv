# Service-Independent Interactive Architecture

> **Note (2026-08-14):** Packaged `mbvd` Service-independent startup (zero Services, Feed without Emby, filesystem/trusted-LAN ctrl auth, `mbvd --connect emby` admin) is implemented in open PR #529 tracking issue #523, not landed on `main`. On `main`, `crates/mbvd/src/main.rs:117-120` still constructs `EmbyClient` unconditionally and requires cached credentials. Local daemon Service-independence is landed.

## Decision

Interactive `mbv` enters the TUI and establishes its local playback arrangement
before any Remote Service authenticates. Configured Emby and Audiobookshelf
Services initialize independently after the first frame, each as a concrete
singleton runtime with its own setup generation and provider-native browse
state. Feeds remains always present. Browse input, rendering, refresh, help,
and hit testing dispatch from an explicit destination; unlike Service catalogs
are not hidden behind a universal provider or browse trait. They converge only
at genuinely shared boundaries such as QueueItem construction and Player-owner
admission.

The same independence applies to the user-owned Local daemon: it may become a
Player owner without a usable Remote Service. Packaged `mbvd` is outside this
decision and remains Emby-gated until a separate accepted change migrates it.

## Context

Emby authentication previously acted as the application-entry gate and made an
`EmbyClient` the de facto application context. That prevented feed-only use,
turned Emby outages into startup failures, and made a second independently
authenticated Service inherit Emby's lifecycle and browse assumptions. Waiting
for every configured Service would preserve the same coupling, while a broad
provider trait would erase real differences between Emby libraries,
Audiobookshelf podcasts, and client-fetched Feeds.

## Considered Options

- Keep Emby as a mandatory startup gate.
- Enter the TUI only after all configured Remote Services finish connecting.
- Introduce one provider/catalog trait and one shared browse model.
- Initialize concrete Services independently after TUI entry and dispatch
  exhaustively by destination (chosen).

## Consequences

Remote Service failure changes only that Service's state. Adding a new browse
destination requires an exhaustive dispatch audit rather than inheriting Emby
behavior through a default branch. Service-independent startup must not be
claimed for packaged `mbvd` until its own runtime construction is migrated.
