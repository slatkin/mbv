---
status: accepted
---

# Migrate the Existing TUI Framework to TuiRealm

mbv's interactive TUI is already an implicit framework spread across `App`, the
run loop, `CONTEXT_STACK`, surface-specific input handlers, render adapters, and
`AppLayout`. We will replace that framework with TuiRealm rather than design a
second mbv-specific component framework on top of Ratatui.

Every independently interactive surface will become a TuiRealm `AppComponent`.
TuiRealm's `Application<ComponentId, Msg, UserEvent>` will own mounted component
instances, focus, subscriptions, event delivery, and component rendering. Flat
component IDs are registry addresses; mbv derives parent, visibility, render
order, and focus relationships from typed IDs and parent state.

An Interactive Component owns its private presentation state, event
interpretation, local updates, rendering, viewport, and render-derived hit
geometry. It emits a `Msg` only for work crossing its authority boundary. Runtime
completions enter through TuiRealm `UserEvent`s or minimal shell adapters.

The application Model remains the shell. It owns terminal lifecycle, Remote
Service and worker lifecycle, Player and canonical queue authority, protocols,
persistence, and external effects. Interactive Components do not receive `App`,
Service clients, credentials, `Config`, `PlayerProxy`, protocol objects, channels,
or shared integration locks.

The existing render system remains the visual substrate. Arrangements provide a
child's outer area; its TuiRealm `Component::view` implementation owns internal
placement and delegates painting to existing Render Components. Adopting TuiRealm
does not require replacing those painters with `tui-realm-stdlib` widgets.

Canonical media-list composition uses `WideMediaList` for fixed-row Wide one-column rails and Queue, and `InlineMediaBrowser` for Normal/Narrow selected-row replacement. Inline Search remains the separate `InlineSearchComponent`; non-hero catalog browsers retain their existing two-column policy. The primary child owner/painter map is Home → `HomeComponent`, generic Emby/Movies/homevideos and Emby podcast → `BrowserComponent`, TV Series → `TvWorkspaceComponent` in Wide / `BrowserComponent` in Normal, grouped Music → `MusicWorkspaceComponent`, Audiobookshelf Podcast → `AudiobookshelfPodcastComponent`, Audiobookshelf Books → `AudiobookshelfBookComponent`, Feeds → `FeedsComponent`, and Queue → `QueueComponent`.

This decision does not add a parallel custom `Component` trait, registry,
dispatcher, focus framework, generic effect scheduler, Flux store architecture,
or separate UI crate. TuiRealm supplies the application framework; mbv adds only
domain-specific IDs, messages, user events, presentation models, and shell-effect
handling.

## Completion

Internal checkpoints and temporary adapters may organize the conversion, but a
mixed TuiRealm/legacy architecture is not a completed or mergeable endpoint. The
migration is complete only when every interactive-surface ledger row uses
TuiRealm, component-local state and handlers have left `App`, `CONTEXT_STACK` and
`AppLayout` are removed, and no parallel legacy interaction framework remains.

> **Superseded by decision D16** (see
> `openspec/changes/archive/2026-08-29-migrate-tui-to-tuirealm/design.md`).
> The completion bar is about *authority*, not deletion. `AppLayout` loses global
> interaction and hit-routing authority, but render-only load-bearing layout
> state may remain. The real bar: no component reads geometry it did not itself
> paint, and no keyboard or behaviour path branches on whether a rect was
> painted.

Existing input precedence, responsive behavior, images-disabled behavior, render
characterization, and process-boundary behavior remain regression contracts.
Search is not a proof of concept, and its existing correctness bugs are separate
from this framework migration.

### Residual debt

Known deviations from the authority bar, each with an owner:

- **Residual B — paint-inference keyboard branching.** Tracked by issue #643.
  Keyboard paths branch on
  `is_wide_*_active()` (`src/app/layout.rs:209`, `:213`, `:220`, `:227`), which
  infers layout from the last painted frame and so is wrong for one frame after
  a resize. Consumed at `src/app/input_browse_dispatch.rs:35`,
  `src/app/actions_navigation.rs:205`, `src/app/shell_messages.rs:32`,
  `src/app/audiobookshelf_book_modal_actions.rs:57-59`,
  `src/app/shell_browser.rs:196`, `src/app/shell_inline_search.rs:49`,
  `src/app/shell_inline_search.rs:267`.
- **Residual C — page rows off global geometry.** Page-size rows are computed
  from global layout geometry at `src/app/actions.rs:121`.

## Considered Options

- Continue extracting bespoke hierarchical components: rejected because it would
  create and maintain another application framework.
- Adopt Flux in addition to TuiRealm: rejected because TuiRealm already provides
  one-way `Event -> Msg -> Model` coordination, while another store and dispatcher
  could recreate global `App` state under new names.
- Keep the current framework and only reorganize files: rejected because ownership,
  event delivery, focus, effects, rendering, and geometry would remain bespoke and
  globally coupled.

## Consequences

The implementation design must map ADR 0002 input precedence, simultaneous mouse
targets, shell/runtime completions, and component-owned geometry onto TuiRealm
without recreating replacement framework machinery. TuiRealm 4.1 matches mbv's
Ratatui 0.30 and Crossterm 0.29 dependencies but requires Rust 1.88; mbv must make
that toolchain requirement explicit before implementation.
