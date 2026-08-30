# mbv

Rust terminal media client for Emby, Audiobookshelf, and Feeds. It embeds mpv;
playback belongs to the terminal, Local daemon, or packaged `mbvd`.

## Read first

- `CONTEXT.md`: domain vocabulary; *Avoid* means incorrect terminology. When
  a change introduces a new domain term, add it in the same PR. When a term
  would collide with or rename an existing entry, flag it to the user instead
  of resolving it unilaterally.
- Current (not superseded) ADRs in `docs/adr/` before architecture changes.
- Design changes live in `openspec/changes/<name>/`; otherwise use GitHub
  Issues/discussions (`slatkin/mbv`) and gists for ad-hoc notes.
- Commit specs/plans/docs with code; merge applied deltas into `openspec/specs/`
  and archive completed changes.

## Boundaries

- `src/`: interactive binary and TUI; `src/local_daemon.rs` bootstraps the
  user-owned Local daemon without Remote Service authentication (ADR 0018).
- `crates/mbv-core/`: Service/runtime, provider APIs, config, ctrl/shared
  protocols, queue, source preparation, and mpv projection. No UI/feed fetch.
- `crates/mbvd/`: separately packaged daemon, persistent state, and sockets.
- Change source-of-truth types before callers.

## Planning and Documenting

- When planning changes, engage the user with design questions. Do not dump unnecessary
information about the plan into the session window. Detail should be captured in markdown
documents. No one is reading ephemeral screen dumps, ever and they are not accessible to
you are future agents unless you capture them in writing.

## Commands

Prefix every command in a chain with `rtk`; use `rtk proxy <cmd>` only to
bypass filtering. `rtk grep` format flags (`-c/-l/-L/-o/-Z`) run raw.

- Check: `rtk cargo check -p <package>`
- Test: `rtk cargo nextest run -p <package>` (prefer nextest over `cargo test`)
- Lint: `rtk cargo clippy --workspace --all-targets`
- Boundary scan: `rtk ast-grep scan` (rule fixtures: `rtk ast-grep test`)
- File-size check: `rtk make check-code-file-lines`
- Format: `rtk cargo fmt` (accept its output; see below)

## Formatting

`cargo fmt` uses stock rustfmt defaults (edition 2021, `max_width=100`); there is
no `rustfmt.toml`. Run it as part of every change and **accept its output**. It
reflows the whole import list and any long signature your edit touches, so lines
adjacent to your diff will change even when you did not edit them — those are
your unformatted additions being normalized, not unrelated churn. Never `git
checkout` a fmt diff; it is correct by definition. Use `cargo fmt --check` to
verify a change is fmt-clean without writing.

## TUI ownership

Two boundaries stack: the **render boundary** governs painting; the
**interactive boundary** (TuiRealm, ADR 0022) governs state and input.

### Render boundary

- `src/app/render/{screens,arrangements,components,theme}/`: screens own app
  state and content; arrangements own placement and breakpoints; components
  own painting and geometry; theme exposes semantic roles only, primitives
  are private. See `CONTEXT.md` Presentation for term definitions.
- Screens do not call Ratatui, construct `Rect`s, or compute hit targets.
  A structural or visual override lives in the owning arrangement, component,
  or theme — never as a screen-local painter branch.
- Workflow, the reuse/override decision table, and the completion checklist:
  `.opencode/skills/mbv-frontend/SKILL.md` (canonical: `openspec/specs/ui-design-system/spec.md`
  and the archived `enforce-mbv-ui-design-system` change
  `openspec/changes/archive/2026-08-23-enforce-mbv-ui-design-system/`); see
  `CONTEXT.md` Presentation for term definitions.

### Interactive boundary (ADR 0022, TuiRealm)

- TuiRealm's `Application<ComponentId, Msg, UserEvent>` owns mounted
  components, focus, subscriptions, and event delivery. Interactive Components
  live in `src/app/components/`, never as new `impl App` interaction handlers.
- The shell `Model` (`src/app/shell*.rs`) owns `App`, terminal/worker/Service
  lifecycle, Player and canonical queue authority, persistence, and external
  effects. A component never receives `App`, a Service client, `PlayerProxy`,
  `Config`, credentials, or an mpsc; it emits a typed `Msg`
  (`src/app/components/msg.rs`) for anything crossing its authority boundary.
  `rules/interactive-component-boundary/` enforces this — run `rtk ast-grep scan`.
- Data flows shell→component one way via `sync_<surface>()` and `push_*`
  helpers: the shell projects validated snapshots; the component owns its
  cursor, scroll, drafts, viewport, and render-derived hit geometry. A `sync_*`
  that mirrors component-local interaction state back into `App` is a
  regression, not a pattern.
- Every row of `docs/architecture/interactive-surface-ledger.md` reached
  `migrated` (2026-08-27); `legacy` and `component` are historical states. A new
  independently interactive surface gets a row and lands as a component directly.
- Mouse is **accepted-broken for the alpha** (D16 in
  `openspec/changes/migrate-tui-to-tuirealm/design.md`): the legacy mouse
  framework was deleted rather than migrated, and mouse behaviour is not part of
  any row's verification record. Do not repair mouse routing opportunistically.

### Keyboard routing (ADR 0023)

- There is exactly one keyboard resolution site: `src/app/router.rs` with its
  ordered policy in `src/app/key_policy.rs`, folded into the tick in
  `shell_run.rs`. It returns ADR 0002's `Command` / `Swallow` / `FallThrough`
  from a plain-data `RouterSnapshot`. Never add a second resolution site, and
  never express precedence as a TuiRealm `SubClause` — `SubClause` cannot encode
  first-match, which is the reason the router exists.
- A component interprets only its own local chords and emits a semantic intent.
  A raw `KeyEvent` crossing the component boundary is legacy, not an escape hatch.
- Legacy-endpoint removal is **in flight**
  (`openspec/changes/remove-legacy-keyboard-endpoint`, sections 6–9 open):
  `GlobalViewKey`, the raw `*Key` request variants, `CONTEXT_STACK`,
  `Model::handle_legacy_key`, and `components/typed_key.rs` still exist and are
  scheduled for deletion. Add no new callers of any of them.

## Constraints

- Source files cap at 800 lines; split over-limit files in the same PR.
- Ctrl changes add capability strings, never bump protocol version; see the
  rule above `CTRL_PROTOCOL_VERSION`.
- Replace flaky tests; do not troubleshoot them.
- Put symbol-specific rules above the symbol.
- Never speculatively patch a daemon/Local-daemon/Stay-Alive issue (mbvd,
  `src/local_daemon.rs`, shared-data, ctrl protocol, Emby/ABS startup racing
  the daemon). These bugs live across process boundaries and plausible-looking
  fixes routinely land nowhere near the real cause. Before touching any code:
  reproduce with real logs/instrumentation from the actual failing process,
  capture the real response/error (not a reconstruction), and state the
  confirmed root cause before writing a fix. A fix proposed without that
- Commit your work when completed. DO NOT leave a dirty worktree with mystery
  changes.

