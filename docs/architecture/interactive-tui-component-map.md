# Interactive TUI Component Map

This map describes the current TuiRealm composition. ADR 0022 defines the
framework and Panel composition; ADR 0023 defines keyboard precedence; ADR 0024
defines mouse delivery. The stable ownership inventory is
`docs/architecture/interactive-surface-ledger.md`.

## Root composition

```text
Root
|-- Tab panel
|-- Library panel
|   |-- typed destination content owners (embedded, not mounted)
|   |-- Wide or Narrow library skeleton
|   `-- Library playback panel (when Queue column is hidden)
|-- Queue panel
|-- Queue playback panel (when Queue column is visible)
|-- Status bar panel
|-- pane boundaries
`-- overlay stack
```

The root computes paint-free placement and composes the Panels. Every Panel
paints its complete placement, including its fill. There is no legacy base frame,
underpaint, shell-painted Panel surface, or hand-ordered destination painter.
Panels absent from the current Panel mode are not mounted with empty placement.

## Library panel workflow

The Library panel owns the Wide and Narrow skeleton, breakpoint predicate,
Selector row, List controls row, list box, Hero header, overview box, Workspace,
all placement, and all slot hit geometry. A destination produces typed content
only. The same content shape has the same presentation for every destination;
Emby is the reference presentation and Audiobookshelf/Feeds conform.

The embedded `MediaList<Target>` owner provides rows, stable targets, cursor,
scroll, row-local behavior, and retained geometry. `MediaListCarrier<Target>`
keeps one canonical fixed-row browser presentation over that owner in every
Panel geometry; it is not a focus target, subscription, or root identity, and it
never switches presentation owners. Geometry changes clamp the viewport in
place. In non-Wide geometry, a hero-bearing browser opens a Library Hero overlay
on demand. There is no Grid presentation. Inline Search occupies the Selector
row and list slots when active. The panel derives Wide/Narrow from shared
geometry and derives Hero header shape (Landscape, Portrait, or Square) from
artwork policy. A destination cannot select an arm or add a slot.

## Authority boundaries

The shell owns Services, Player and queue authority, persistence, workers,
protocols, effects, and validated projections. Interactive Components own local
cursor, scroll, focus, drafts, event interpretation, view, and geometry they
paint. Components receive typed projections, never `App`, clients, credentials,
configuration, Player handles, channels, or raw terminal events. Destinations
translate slot messages into typed shell requests.

Keyboard chords are resolved only by `UiRoot`'s Keyboard Router in
`src/app/router.rs` using `src/app/key_policy.rs`; local components interpret
only local semantic chords. Mouse eligibility is derived during shell sync from
currently painted Panels; eligible components subscribe and resolve only their
own retained geometry. No global hit map or coordinate re-resolution exists.

## Verification obligations

Use buffer tests for slot and Panel painting, relational arrangement tests for
placement, and `Application::tick()` integration tests for mount, focus,
subscription, and message ordering. Verify Wide and Narrow, each Panel mode,
one painter per surface, and that absent Panels have no placement. Do not add
conformance matrices that compare destinations or caller-selected presentation
arms; shared typed slots are the enforcement mechanism.
