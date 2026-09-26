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
  remote endpoint — must be checked.
* Change source-of-truth types before callers; ask only about material
  design/product choices.
* Commit or undo your changes; never leave a dirty worktree.
* No file over 800 lines at push time: split along responsibility seams and run
  `make check-code-file-lines` just before pushing; never a per-task/CI/acceptance gate.
* Coding standards: `docs/standards/README.md` (Microsoft Pragmatic Rust Guidelines,
  pinned locally, one file per rule). Scan its checklist, then `qmd query` a rule for
  full text; never read every rule.

## Repository map

* `src/app/shell/` — interactive shell + TuiRealm `Model`: `App`, mount/focus,
  runtime lifecycle, projections, dispatch, effects, and the single tick/draw
  path (`shell/run.rs`, `shell/draw.rs`).
* `src/app/components/` — Interactive Components + typed `Msg`s; `media_list/`
  embedded list controls; `mouse/` pointer primitives.
* `src/app/render/` — `screens/` prepare content, `arrangements/` place it,
  `components/` paint it, `theme/` semantic roles.
* `src/app/input/` — the only keyboard routing site (`router.rs` precedence,
  `key_policy.rs` order, `resolver.rs` chords); never add another.
* `src/local_daemon.rs` — Local-daemon bootstrap; rest of `src/` = TUI binary.
* `crates/mbv-core/` — runtime, Services, providers, config, protocols, canonical
  queue, source prep, mpv projection; no UI/feed fetch.
* `crates/mbvd/` — packaged daemon, persistence, sockets.
* `crates/mbv-ids/` — type-safe media identifier newtypes (`ItemId`, `MediaSourceId`, `EmbySessionId`).
* `crates/mbv-keybinds/` — configurable keybinding registry, chord grammar, validation.
* `crates/mbv-net/` — shared HTTP, TLS-agent, socket and retry primitives.
* `crates/mbv-ws/` — Emby websocket client transport.
* `crates/mbv-visualizer/` — PipeWire stereo audio capture worker for the visualizer.
* `crates/mbv-text/` — fuzzy-match acceptance and control-character predicates for text input.

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

ADRs 0022–0024; `openspec/specs/interactive-component-framework/spec.md`;
`docs/architecture/interactive-surface-ledger.md`.

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

## TUI work

Before any TUI change read `.agents/skills/mbv-frontend/SKILL.md`: Panel/slot
composition, embedded canonical media lists, keyboard and mouse routing, render
layering, reuse workflow, and tick-integration test rules. Tripwires that apply
even when you don't think it's TUI work: one owner and one painter per surface
per breakpoint; keyboard precedence only in `src/app/input/`.

## Tooling

* **No bespoke scripting.** No shell/awk/python checkers (or their self-tests, CI
  jobs, Makefile targets) and never one as proof; verify with unit tests unless
  the user explicitly asks for a script, per request.
* check: `cargo check -p <package>`
* test: `cargo nextest run -p <package>` locally (prefer nextest); CI runs `cargo test --release -- --test-threads=4` (fd-budget throttling, see `build.yml` comment); use `cargo llvm-cov` to check coverage.
* **A test owns a contract or does not exist.** Before adding a test, name
  the contract and the layer that owns it (mbv-frontend skill, Tests matrix);
  if another test already covers the claim, extend that test or add nothing.
  `#[case]` tables keep only cases whose expected outcomes differ. A
  regression test cites the issue or commit it guards, in its name or a
  comment. A plan item that only says "add tests" is not sufficient — name
  the contract. Test count and coverage are never goals
  (`docs/invariants/14-test-ownership.md`).
* **Unit tests are hermetic mocks — no live or smoke tests.** No live mpv handle
  (`init_mpv`/`test_mpv`), real config/state dirs, or live servers; mock the
  boundary. Anything needing the real external is a manual check.
* **No real sleeps in tests** (`thread::sleep`, timeouts, retry backoff): inject
  the outcome, not the delay (mock returning the terminal state, zeroed timeout,
  seam observing attempts). A test that can't be fast without changing prod
  timing asserts the wrong thing — delete it.
* **Tests stay simple**: one behaviour per test, no loops/branching in the body,
  setup in helpers. Clippy's complexity threshold (25) is crate-wide, so review
  enforces the stricter bar for tests.
* Fixture-varying test families use named `#[case]` tables (via the `rstest`
  dev-dependency); `#[case]` is never a mechanism for generating many thin tests,
  and conversions are opportunistic and file-by-file.
* lint: `cargo clippy --workspace --all-targets -- -D warnings`
* **No lint suppression without per-instance user approval**: no `allow`/`expect`
  attribute in any form, no loosening `[lints]`, and never edit `clippy.toml`
  (thresholds included) unless the user explicitly asks. Fix the cause
  (params struct, delete dead code and its tests, drop the unused import).
* format: `cargo fmt` per Rust change (stock edition-2021, max-width-100); accept
  all reflow, never revert it; `cargo fmt --all -- --check` = read-only check.
* errors: custom domain error types (e.g. `AudiobookshelfError`); do not introduce `anyhow`/`thiserror`/`eyre`
* module layout: one file per module via `mod`, never `include!`/`#[path]` to splice a module across files; a module with children is `foo.rs` plus a `foo/` directory holding them (tests as `foo/tests.rs`, or `foo/tests.rs` + `foo/tests/` when split); never `mod.rs`
* async: sync-first; `tokio` is edge-only (`src/mpris.rs`, `zbus`) — do not spread it
* sharing: prefer owned data + `Msg` identities over new `Arc`/`Rc`
* anything web related: `ketch` not curl
* docs/concept discovery (ADRs, openspec, CONTEXT.md): `qmd query "..."` (collection `mbv`); `rg` only for exact strings
