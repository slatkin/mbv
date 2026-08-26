## Why

mbv's interactive TUI is an implicit, bespoke framework spread across the global
`App` struct, the `App::run` loop, the `CONTEXT_STACK` first-match input
resolver, surface-specific `impl App` input/action modules, mutable render
adapters, and the completed-frame `AppLayout` hit map. The completed
design-system migration standardised painting but deliberately left interactive
ownership — cursors, scrolling, forms, overlays, async presentation state, event
handling, and hit geometry — distributed through `App`.

Replacing those mechanisms with new mbv-specific machinery would only build a
second local framework. ADR 0022 (accepted) instead adopts **TuiRealm** as mbv's
TUI application framework. This change is the single, complete-conversion OpenSpec
that ADR 0022 and `docs/architecture/interactive-tui-component-map.md` call for:
it resolves the outstanding integration questions and converts every interactive
surface in `docs/architecture/interactive-surface-ledger.md`.

## What Changes

- **BREAKING (build):** Declare `rust-version = "1.88"` in `[workspace.package]`.
  TuiRealm 4.1 requires it; mbv currently declares no MSRV. Verified: tuirealm
  4.1.0 depends on `ratatui ^0.30` and `crossterm ^0.29`, matching mbv's existing
  `ratatui 0.30.2` / `crossterm 0.29.0`, so no render-substrate churn is required.
- Add the `tuirealm = "4.1"` dependency (default features cover `crossterm` and
  the `derive` macros); adopt its
  `Application<ComponentId, Msg, UserEvent>` registry, `AppComponent`/`Component`
  contracts, mount/unmount/focus/subscription lifecycle, input listener, and
  custom `UserEvent` ports.
- Introduce `src/app/components/` as the home of Interactive Components (TuiRealm
  `AppComponent`s). Each owns one independently routed surface's private state,
  event interpretation, local updates, rendering, viewport, and render-derived hit
  geometry, and emits a typed `Msg` only for work crossing its authority boundary.
- Convert the shell so the application Model retains only shell/runtime authority
  (terminal lifecycle, Service/worker lifecycle, Player + canonical queue, ctrl/
  shared-data protocols, persistence, external effects) plus the TuiRealm
  `Application` — not another global UI state store.
- Convert every row of the interactive-surface ledger from `legacy` to `migrated`:
  root/overlay routing, playback chrome, Queue, the Library parent and all its
  destination children (Home, Emby browsers, TV, Music, ABS books/podcasts,
  Feeds, inline library Search, inline album-track), the full overlay stack
  (global Search, Settings + setup forms + nested popups, Sessions, Playlists +
  save dialog, Help, context menu, selection/confirm/daemon-lost/re-anchor
  modals), and playback prompts.
- **Remove** the legacy interaction framework: `CONTEXT_STACK` interaction
  dispatch, `AppLayout` as a global interaction/hit-routing authority, duplicated
  mouse-coordinate paths, `impl App` interaction handlers and render adapters for
  migrated surfaces, and all temporary migration adapters/state mirrors.
  Render-only layout state may remain where painting still requires it.
- Add compiler- and `ast-grep`-based enforcement (`rules/interactive-component-
  boundary/*.yml`, a new `architecture-boundaries.yml` CI job) that rejects a
  parallel legacy framework and `&mut App` in Interactive Component paths.
- **Non-goals / preserved contracts:** no daemon, Local-daemon, `mbvd`, ctrl,
  shared-data, provider, playback, or canonical-queue behaviour change. ADR 0002
  keyboard precedence, responsive behaviour, images-disabled behaviour, and
  existing render characterization remain regression contracts. Completing mouse
  interaction for Music, blocking modals, and playback prompts is deferred beyond
  the alpha migration; the removed global mouse router and duplicated coordinate
  framework must not be reintroduced when that work resumes. Search's existing
  correctness bugs are out of scope and must not be silently changed. No parallel
  custom component/dispatcher/focus/effect/Flux framework is created.

## Capabilities

### New Capabilities
- `interactive-component-framework`: every independently interactive surface is a
  TuiRealm `AppComponent`; TuiRealm owns mounting, focus, subscriptions, event
  delivery, and component rendering; the authority boundary between Interactive
  Components and the shell Model; preservation of ADR 0002 input precedence and
  simultaneous mouse targets via TuiRealm focus/subscriptions; component-owned hit
  geometry; the complete-conversion gate; and static/CI enforcement.

### Modified Capabilities
- `ui-design-system`: supersede the requirement that hit-target ownership stays
  with `AppLayout`/input resolution and that no arrangement/component hit-map
  migration is introduced. On completion, hit geometry is owned by the interactive
  component that painted it; the render ownership, semantic-theme, and bespoke
  boundaries are otherwise unchanged.

## Impact

- **Dependencies:** adds `tuirealm 4.1` (+ `tuirealm_derive`); declares workspace
  MSRV 1.88.
- **Code:** `src/app/` broadly — `app_struct.rs`, `mod.rs` (`App::run`),
  `input*.rs`, `input_resolver.rs` (`CONTEXT_STACK`), `action.rs`, `layout.rs`
  (`AppLayout`), `run_loop_drains.rs`, all `impl App` interaction/render adapters,
  and new `src/app/components/`.
- **Docs/records:** `docs/adr/0022-*`, the architecture map and interactive-surface
  ledger (rows flip to `migrated`), `CONTEXT.md` vocabulary, and this change's spec
  deltas.
- **Tooling/CI:** `sgconfig.yml` gains `rules/interactive-component-boundary/`;
  `.github/workflows/architecture-boundaries.yml` adds the
  `interactive-component-boundary` job pinning `ast-grep` 0.44.1.
