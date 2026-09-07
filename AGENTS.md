# mbv

Rust terminal media client for Emby, Audiobookshelf, Feeds. Embeds mpv; playback
runs Bare, via Local daemon (Stay-alive), or packaged `mbvd` Player owner.

## Start here

* Read `CONTEXT.md` before naming domain concepts (its *Avoid* terms are wrong);
  add new terms with the change; ask before renaming/colliding.
* Architecture work: `docs/adr/` = accepted decisions (the why);
  `openspec/specs/` = current behaviour. In-progress: read full
  `openspec/changes/<name>/`; its delta specs overlay main specs until archived;
  don't edit archived changes.
* Durable plans in OpenSpec markdown, not chat; commit plans/specs/docs with
  code; sync applied deltas into `openspec/specs/`; archive when done.
* Change source-of-truth types before callers; ask only about material
  design/product choices.
* Commit or undo your changes; never leave a dirty worktree.

## Repository map

* `src/app/shell*.rs` — interactive shell + TuiRealm `Model`: `App`, mount/focus,
  runtime lifecycle, projections, dispatch, effects, and the single tick/draw
  path (`shell_run.rs`, `shell_draw.rs`).
* `src/app/components/` — Interactive Components + typed `Msg`s; `media_list/`
  embedded list controls; `mouse/` pointer primitives.
* `src/app/render/` — `screens/` prepare content, `arrangements/` place it,
  `components/` paint it, `theme/` semantic roles.
* `src/app/router.rs`, `key_policy.rs`, and `input_resolver.rs` — the central
  keyboard policy and chord resolution; do not add another routing site.
* `src/local_daemon.rs` — Local-daemon bootstrap; rest of `src/` = TUI binary.
* `crates/mbv-core/` — runtime, Services, providers, config, protocols, canonical
  queue, source prep, mpv projection; no UI/feed fetch.
* `crates/mbvd/` — packaged daemon, persistence, sockets.

## Interactive architecture

```text
App/runtime -> shell sync/push -> Interactive Component -> Render Component
App/runtime <- shell handles typed Msg with resolved target <- component update
```

* Shell `Model` owns terminal/worker lifecycle, Services, Player/queue
  authority, persistence, protocols, external effects, TuiRealm `Application`;
  projects owned presentation models; not a 2nd store of component-local UI
  state. `App` remains the shell's domain/effect state and base-frame geometry
  composer; `Model::draw_frame` composes that frame once, then mounted
  components paint their owned surfaces.
* Mounted `AppComponent` (`src/app/components/`) owns cursor, scroll, local
  focus/selection, filters, drafts, viewport, event interpretation, `view()`,
  hit geometry; mutates local state directly; typed `Msg` only for work outside
  its authority.
* Components never receive `App`, Service clients, credentials, `Config`,
  `PlayerProxy`, protocol objects, integration locks, channels. Msgs = semantic
  intent + stable opaque identities, never raw events/coordinates for shell
  re-resolution.
* Projection 1-way: `sync_*`/`push_*` carry shell-owned content, not cursor/
  scroll/selection mirrors; local movement driving persistence/effects sends
  the resolved value from the component — shell never recomputes; re-anchor
  only on discrete navigation/responsive transition.
* Destination components stay mounted while their Service library is in the
  catalog (local state survives tab/breakpoint changes); mounted/focused/active/
  painted distinct; overlays mount/unmount via TuiRealm focus stack.
* Every boundary-crossing request variant: exhaustive dispatch arm or documented
  no-op — never wildcard-hidden.

## Playback and queue authority

* A Composed or Bound queue has one canonical ordered sequence of `QueueItem`
  slots. Use stable `QueueSlotId` for occurrences; do not recreate parallel
  Emby/Feed/Audiobookshelf lists or use content identity to address a slot.
* The Player owner is authoritative for Bound queue state, active slot, and
  playback lifecycle. mpv is a source/output projection (and may materialize
  only the active file), never queue authority.
* Owner admission is the capability boundary: media kind, required Service
  setup, and negotiated ctrl transport determine what enters a Bound queue.
  Components may edit Composed content but never perform admission or mutate
  the canonical queue directly; shell/Player paths do that work.

ADRs 0022–0024; `openspec/specs/interactive-component-framework/spec.md`;
`docs/architecture/interactive-surface-ledger.md`.

## Embedded canonical media lists

Destination `AppComponent`s compose reusable plain TuiRealm `Component`s;
children never mounted/focused/subscribed/given `ComponentId`; destination =
sole event boundary, translates provider-specific intents.

* `WideMediaList<Target>` — fixed-height 1-column cursor/scroll/viewport, row
  placement, scrollbar, row geometry; serves Hero rails + Queue fixed rows; not
  a replacement for the non-hero 2-column catalog policy.
* `InlineMediaBrowser<Target>` — Normal/Narrow 1-column list + selected-row
  replacement: fit admission, fallback, scrolling, replacement geometry; not
  Inline Search.
* Rows provider-neutral: selectable `Item`s with stable opaque targets +
  non-selectable `Heading`/`Spacer`; parents keep provider content, workspaces,
  pills, images, effects, persistence, message translation.
* Wide/Inline transitions transfer one `ViewportAnchor` (selected target + row
  offset); ordinary refresh preserves/clamps local state, never adopts shell
  cursor/scroll.

1 owner, 1 painter per surface per breakpoint; no 2nd loop as underpaint/
fallback. Contract: `openspec/specs/canonical-media-lists/spec.md`.

## Input and rendering boundaries

* Keyboard precedence only in `src/app/router.rs`, ordered policy in
  `src/app/key_policy.rs`, and chord conversion in `input_resolver.rs`:
  `UiRoot` picks `Command`/`Swallow`/`FallThrough`; focused components handle
  local semantic chords. Shell compatibility/fall-through handlers may remain
  for explicitly unmigrated commands, but they do not become a second router
  or precedence policy.
* Mouse (ADR 0024): subscriptions decide eligibility pre-delivery, following
  surfaces painted in latest frame (or topmost overlay); mounted parent owns
  gesture state, resolves only geometry it painted; embedded lists resolve own
  rows; there is no global hit map for component surfaces. Shell-painted chrome
  (currently the tab bar) may expose its own paint-time hit geometry and resolve
  the observer message in the shell. Never discard a losing message after its
  component mutated. TuiRealm pinned 4.1 — re-verify ADR 0024's subscription
  assumption before any bump.
* Render order: screens → arrangements → Render Components → Ratatui. Screens =
  typed semantic content only; arrangements = placement/breakpoints; Render
  Components = painting + paint-local geometry in supplied `Rect`; theme =
  semantic roles, raw colour primitives private.
* Screens never call Ratatui, build `Rect`s, split layouts, compute hit targets,
  add painter overrides; rendering never performs Service/image/playback/
  persistence effects.

Before any TUI change follow `.agents/skills/mbv-frontend/SKILL.md`; reuse
existing component/arrangement first; differences → typed content, named
central policy/variant, or documented bespoke Render Component with buffer
coverage.

## Tooling

* check: `cargo check -p <package>`
* test: `cargo nextest run -p <package>` (prefer nextest)
* lint: `cargo clippy --workspace --all-targets`
* architecture: `ast-grep scan`
* size: `make check-code-file-lines`
* format: `cargo fmt`
* anything web related: `ketch` not curl

Rustfmt: stock edition-2021, max-width-100; run per Rust change, accept all
reflow, never revert fmt output; `cargo fmt --all -- --check` = read-only
verification.

TUI changes: narrowest component/state + buffer tests. Mounting/focus/
subscription/routing changes need real `Application::tick()` integration tests
(`src/app/tests_tick_integration*.rs`) through the shell sync pass — direct
`Component::on` tests do not verify composition. Also check relevant
Normal/Narrow and Wide breakpoints, one-painter ownership, and hit geometry
when painting moves. When a surface is migrated, prove the base frame only
reserves its area and does not underpaint the mounted component.
