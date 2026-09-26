# Design

## Context

See `proposal.md` — Why. Facts that shape the approach, all measured on `main`
at `ef10064a4`:

- `crates/mbv-core/src/lib.rs` is both a module list and a home for four loose
  helpers: `PATH_SEGMENT` + `encode_path_segment` (percent-encoding),
  `reconnect_backoff_sleep` (jittered retry), `native_tls_agent` (ureq agent
  construction). Only the last is `pub`; the rest are `pub(crate)`.
- `keybinds/` uses nothing outside `std` — no `serde`, no `crossterm`. The
  `[keys]` TOML is parsed in `mbv-core/src/config/`, which hands
  `RawKeybinds`/`RawSection` to `keybinds::load`. That is already the right
  direction (`config` → `keybinds`) and needs no inversion.
- `ws.rs` has one internal reference: `crate::reconnect_backoff_sleep`.
  `audiobookshelf/socket.rs` calls the same helper and stays in `mbv-core`, so
  the helper must be reachable from both.
- `mock_http.rs` is declared `#[cfg(any(test, feature = "test"))] pub mod`. The
  `mbv` crate consumes it through its dev-dependency
  `mbv-core = { features = ["test"] }`.
- `visualizer_worker.rs` is the only file in the repo that touches `pipewire`
  (the other four `pipewire` hits are test function *names*). `visualizer.rs`
  sits above it and does depend on config/player/queue, so it stays put.
- Three of six crate names in #814 are taken verbatim; `mbv-fuzzy` is not (see
  Decisions).

## Goals / Non-Goals

**Goals:**

- Each new crate compiles with no path dependency on `mbv-core` or `mbv` except
  `mbv-ws` → `mbv-net`.
- Old module paths are gone, not shimmed, so a stale `mbv_core::ws::…` is a
  compile error rather than silently working.
- `mock_http` stays gated exactly as today: available to `mbv-core`'s own tests,
  to `crates/mbvd`, and to the `mbv` crate's tests; absent from release builds.

**Non-Goals:**

- No dependency inversion, no trait extraction, no API redesign. Every moved
  item keeps its signature and its behaviour.
- No ureq-centralization cleanup. #814 notes `mbv-net` "absorbs" it and
  `project_ureq_centralization` tracks duplicate agent construction; that is a
  follow-up, because doing it here would mix a behaviour-capable change into a
  pure move.
- `visualizer.rs`, `feed_parse.rs`, `images/`, `palette/` stay in the TUI crate.
  Tiers 2–5 own those.
- No `cargo build` timing benchmark. The win is structural; measuring it is not
  a gate.

## Decisions

### D1: `reconnect_backoff_sleep` goes to `mbv-net`, not `mbv-ws`

#814 offers both. `mbv-net` wins because `audiobookshelf/socket.rs` calls it and
stays in `mbv-core` — if the helper lived in `mbv-ws`, `mbv-core` would depend on
`mbv-ws` purely for a retry sleep, which is backwards for a websocket transport
crate. In `mbv-net` it sits next to the other shared network incantations and
both callers reach it symmetrically.

Alternative rejected: duplicate the ~15 lines in both crates. It is the exact
duplication the helper's doc comment says it exists to prevent.

### D2: `mbv-text`, not `mbv-fuzzy` + a second crate, and not `mbv-tui-util`

`fuzzy_match::word_match_score` (~130 lines) and `text_safety::is_control_char`
(3 lines) are both pure predicates over user-facing text with no other
dependants. One crate, named for what is in it. `mbv-tui-util` was rejected as a
junk-drawer name: a crate that means "misc" accumulates, and the whole point of
#814 is boundaries that mean something.

### D3: `mbv-net` owns the `test` feature; `mbv-core` forwards

`mbv-net` gets `[features] test = []` and gates `pub mod mock_http` with
`#[cfg(any(test, feature = "test"))]`, mirroring today's `mbv-core`. `mbv-core`'s
existing `test = []` becomes `test = ["mbv-net/test"]` so
`mbv-core = { features = ["test"] }` keeps working unchanged for `crates/mbvd`.

The `mbv` crate's own tests name `mock_http` directly, so its `[dev-dependencies]`
gains `mbv-net = { path = "crates/mbv-net", features = ["test"] }` and those
call sites become `mbv_net::mock_http::…`. `mbv-net` also appears in `mbv`'s
regular `[dependencies]`, because `feed_parse.rs` calls `native_tls_agent`.

Alternative rejected: a separate `mbv-mock-http` crate. It is 242 lines whose
only purpose is to mock the `ureq` transport `mbv-net` configures; splitting
them puts a crate boundary through one concern.

### D4: One change, six commits — not six changes

#814 says "each should be one PR". Kept as one OpenSpec change because the six
moves share one mechanical shape, one `Cargo.toml` edit site, and one
verification gate; six proposals would be six copies of this document. The
task groups below are commit-sized and ordered so that each group leaves the
workspace green, so it can still land as six PRs if wanted.

### D5: Visibility widens to `pub`, and that is accepted

`bounded::run_with_hard_bound*`, `stream::SocketStream`,
`fuzzy_match::word_match_score`, `text_safety::is_control_char`,
`visualizer_worker::{StereoSampleBuffer, join_worker}` are `pub(crate)` or
`pub(in crate::app)` today; crossing a crate boundary forces `pub`. This is the
one place the split loosens rather than tightens a boundary. Net effect is still
a tightening: `pub` on a 500-line leaf crate with three named consumers is a
narrower surface than `pub(crate)` inside a 46k- or 147k-line crate.

### D6: Module layout inside each new crate

Each crate is `crates/mbv-<name>/src/lib.rs` plus the moved files verbatim.
`mbv-keybinds` keeps its `chord.rs` / `config.rs` / `registry.rs` / `tests.rs`
split, with today's `keybinds.rs` becoming its `lib.rs` (same `mod` + `pub use`
block, doc comment retained). `mbv-net` is a `lib.rs` holding the four loose
helpers plus `mod bounded; mod stream; mod mock_http;`. Per `AGENTS.md`: no
`mod.rs`, no `include!`.

Each `Cargo.toml` inherits `version.workspace`, `edition.workspace`,
`rust-version.workspace`, `license.workspace`, `repository.workspace`,
`keywords.workspace`, `categories.workspace`, `[lints] workspace = true`, and
names its externals from `[workspace.dependencies]` — `fuzzy-matcher` and
`pipewire` move from `mbv`'s `[dependencies]` into `[workspace.dependencies]`
where they are not already there (`pipewire` already is).

## Risks / Trade-offs

- **`clippy --workspace` surfaces new lints.** The workspace lint table is
  inherited, but `pedantic`/`cargo` lints that were satisfied crate-internally
  can fire on newly-`pub` items — notably `missing_panics_doc`,
  `must_use_candidate`, and `cargo::` metadata lints on a new manifest (missing
  `description`). → Give every new crate a `description` and fix lints at the
  source; `AGENTS.md` forbids `allow`/`expect` without per-instance approval, so
  any that cannot be fixed stops the task and goes to the user.
- **`mbv-core` drops `percent-encoding`; `mbv` drops `pipewire`.** If a
  reference is missed, the build fails loudly. → Remove each dependency line in
  the same commit as the move, so `cargo check -p` proves the drop.
- **Six new crates is six more manifests to keep in version lockstep.** →
  Everything inherits from `[workspace.package]`; nothing is pinned per crate.
- **In-flight `decompose-app-god-type` and `prune-tui-test-suite` both edit
  `src/app/`.** Groups 5 and 6 (`mbv-visualizer`, `mbv-text`) collide with them;
  `visualizer_worker` is named by `state/app_struct.rs` and `state/construct.rs`,
  which `decompose-app-god-type` rewrites. → Those two groups run last, and only
  after re-checking `openspec list` for overlap. Groups 1–4 touch only
  `crates/mbv-core/` and are safe alongside both.
- **Total build time can go up slightly, not down.** Splitting adds crate-graph
  and codegen-unit overhead; the win is in *incremental* rebuilds and in
  parallelism across the six leaves. → Accepted; the compiler-enforced
  boundaries and the Tier 2–5 unblocking are the primary payoff.

## Migration Plan

Nothing is deployed and nothing persists, so there is no runtime migration. The
sequencing constraint is `mbv-net` (group 3) before `mbv-ws` (group 4); the other
groups are independent. Rollback for any group is `git revert` of that group's
commit — no shim means no half-migrated state to unwind.
