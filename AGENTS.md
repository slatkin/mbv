# mbv

Rust terminal media client for Emby, Audiobookshelf, Feeds. Embeds mpv; playback
runs Bare, via the Stay-alive process, or packaged `mbvd` Player owner.

## Start here

* Read `CONTEXT.md` before naming domain concepts (its *Avoid* terms are wrong);
  add new terms with the change; ask before renaming/colliding.
* Architecture work: `docs/adr/` = accepted decisions (the why);
  `openspec/specs/` = current behaviour. In-progress: read full
  `openspec/changes/<name>/`; its delta specs overlay main specs until archived;
  don't edit archived changes.
* `docs/invariants/` = properties the code must maintain but that no type
  enforces (why they matter, how they're upheld today, where they still
  fail) — read before touching queue/progress/playback-lifecycle state;
  add one when you find or fix this kind of bug rather than leaving the
  lesson only in a commit message.
* Durable plans in OpenSpec markdown, not chat; commit plans/specs/docs with
  code; sync applied deltas into `openspec/specs/`; archive when done.
* **Probe the real service before planning API work.** Any feature whose design
  depends on what a remote API returns — Emby, Audiobookshelf, feeds, any
  remote endpoint — must be checked against the real service before the design
  is finalized, with the observed responses recorded in the plan (per task:
  a manual check note, never a live test). If what comes back does not support
  the design, the plan is reopened and fixed before any code is written.
* Change source-of-truth types before callers; ask only about material
  design/product choices.
* Commit or undo your changes; never leave a dirty worktree.
* Do NOT modify ~/.config/mbv/config.toml unless asked by the user.

## Repository map

* `src/app/shell/` — interactive shell + TuiRealm `Model`: `App`, mount/focus,
  runtime lifecycle, projections, dispatch, effects, and the single tick/draw
  path (`shell/run/mod.rs`, `shell/draw.rs`).
* `src/app/components/` — Interactive Components + typed `Msg`s; `media_list/`
  embedded list controls; `mouse/` pointer primitives.
* `src/app/render/` — `screens/` prepare content, `arrangements/` place it,
  `components/` paint it, `theme/` semantic roles.
* `src/app/input/router.rs`, `input/key_policy.rs`, and `input/resolver.rs` — the central
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

The root composes Panels. The Library panel owns the Wide/Narrow skeleton and
destinations supply typed Selector row, List controls row, list, Hero header,
and Workspace content; no destination lays out or paints the panel and no base
frame underpaints it. The Queue and playback Panels own their corresponding
surfaces.

* `MediaList<Target>` owns each logical media-row flow: provider-neutral rows,
  stable-target selection, cursor/scroll, row-local behavior, and retained
  geometry. It is embedded, never mounted, focused, subscribed, or given a
  `ComponentId`; the destination retains Service content and typed translation.
* `WideMediaList<Target>` is the fixed-row one-column presentation over the
  owner, including Queue. The Library panel keeps this one canonical fixed-row
  presentation active in every geometry; its `MediaListCarrier<Target>` never
  switches presentation owners, and geometry changes clamp the viewport in place.
* The Library panel derives its skeleton from shared geometry; a destination
  cannot choose a presentation arm or add a slot. In non-Wide geometry, a
  hero-bearing browser opens a Library Hero overlay on demand.
* Rows are provider-neutral selectable `Item`s with stable opaque targets plus
  non-selectable `Heading`/`Spacer`; parents retain provider content, workspaces,
  effects, persistence, and message translation.
* Geometry transitions reuse the canonical owner; ordinary refresh preserves and
  clamps local state.

1 owner, 1 painter per surface per breakpoint; no second loop or fallback
painter. Contract: `openspec/specs/canonical-media-lists/spec.md`.

## Input and rendering boundaries

* Keyboard precedence only in `src/app/input/router.rs`, ordered policy in
  `src/app/input/key_policy.rs`, and chord conversion in `input/resolver.rs`:
  `UiRoot` picks `Command`/`Swallow`/`FallThrough`; focused components handle
  local semantic chords. Shell compatibility/fall-through handlers may remain
  for explicitly unmigrated commands, but they do not become a second router
  or precedence policy.
* Mouse (ADR 0024): subscriptions decide eligibility pre-delivery, following
  surfaces painted in latest frame (or topmost overlay); mounted parent owns
  gesture state, resolves only geometry it painted; embedded lists resolve own
  rows; there is no global hit map for component surfaces. `TabPanel` owns the
  tab regions it paints and resolves them; the shell does not supply a second
  component-surface routing path. Never discard a losing message after its
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

* **No bespoke scripting.** Do not add shell/awk/python checkers, self-tests for
  them, CI jobs or Makefile targets that wrap them, and never use one as a proof
  mechanism. Verify through the conventional unit-testing framework
  (`cargo nextest run -p <pkg>`); a proof that needs a new script needs the
  user to ask for it explicitly, per request.
* check: `cargo check -p <package>`
* test: `cargo nextest run -p <package>` locally (prefer nextest); CI runs `cargo test --release -- --test-threads=4` (fd-budget throttling, see `build.yml` comment); `cargo llvm-cov` should be used to help ensuring proper test covrerage.
* **Unit tests are mocks only — no live tests, no smoke tests.** Unit tests must not
  construct real externals: no live mpv handle (`init_mpv`/`test_mpv`), no real config
  or state directories, no live servers. Mock the boundary; keep tests deterministic
  and hermetic. mbv is a single-user system — live/smoke coverage is inappropriate;
  anything that genuinely needs the real external is a manual check, not a test.
* **Tests that force real sleeps are a smell — do not add them.** A test that pays
  wall-clock time (`thread::sleep`, timeouts, retry backoff) to prove its point is
  testing the clock, not the code. Restructure so the property is asserted without
  waiting: inject the outcome, not the delay (a mock that returns the terminal state
  directly, a zeroed timeout, a seam that observes attempts instead of sleeping
  through them). A slow test that cannot be made fast without changing prod timing
  is either asserting the wrong thing or needs deleting, not keeping.
* Fixture-varying test families use named `#[case]` tables (via the `rstest`
  dev-dependency); `#[case]` is never a mechanism for generating many thin tests,
  and conversions are opportunistic and file-by-file.
* lint: `cargo clippy --workspace --all-targets -- -D warnings`
* **No lint suppression without explicit user approval, per instance.** Never add
  `#[allow(...)]`, `#[expect(...)]`, `#![allow(...)]`, `#[cfg_attr(..., allow(...))]`,
  or loosen `clippy.toml`/`[lints]` thresholds to silence a warning. A lint is a
  code-quality signal: fix the cause (params struct, delete dead code and the
  tests that only exercise it, remove the unused import). If you believe a
  suppression is genuinely warranted, stop and ask.
* format: `cargo fmt`
* errors: custom domain error types (e.g. `AudiobookshelfError`); do not introduce `anyhow`/`thiserror`/`eyre`
* module layout: one file per module via `mod`, never `include!`/`#[path]` to splice a module across files (removed entirely by modularize-mbv-core-layout); a family with tests gets a directory with `mod.rs` plus `tests/` alongside it
* async: sync-first; `tokio` is edge-only (`src/mpris.rs`, `zbus`) — do not spread it
* sharing: prefer owned data + `Msg` identities over new `Arc`/`Rc`; shell owns domain state, components own local UI state
* anything web related: `ketch` not curl
* docs/concept discovery (ADRs, openspec, CONTEXT.md): `qmd query "..."` (collection `mbv`); `rg` only for exact strings

Rustfmt: stock edition-2021, max-width-100; run per Rust change, accept all
reflow, never revert fmt output; `cargo fmt --all -- --check` = read-only
verification.

TUI changes: narrowest component/state + buffer tests. Mounting/focus/
subscription/routing changes need real `Application::tick()` integration tests
(`src/app/tests/tick_integration/`) through the shell sync pass — direct
`Component::on` tests do not verify composition. Also check relevant
Narrow and Wide presentations, one-painter ownership, and hit geometry when
painting moves. Prove each Panel paints its own complete placement and that
absent Panels are not mounted with empty areas.
