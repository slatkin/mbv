# mbv

Rust terminal media client for Emby, Audiobookshelf, and Feeds. It embeds mpv;
playback belongs to the terminal, Local daemon, or packaged `mbvd`.

## Read first

- `CONTEXT.md`: domain vocabulary; *Avoid* means incorrect terminology. When
  a change introduces a new domain term, add it in the same PR. When a term
  would collide with or rename an existing entry, flag it to the user instead
  of resolving it unilaterally.
- Current (not superseded) ADRs in `docs/adr/` before architecture changes.
- `openspec` contains current and archived specs for major implementations.

## Boundaries

- `src/`: interactive binary and TUI; `src/local_daemon.rs` bootstraps the
  user-owned Local daemon without Remote Service authentication (ADR 0018).
- `crates/mbv-core/`: Service/runtime, provider APIs, config, ctrl/shared
  protocols, queue, source preparation, and mpv projection. No UI/feed fetch.
- `crates/mbvd/`: separately packaged daemon, persistent state, and sockets.
- Change source-of-truth types before callers.

## Commands

Prefix every command in a chain with `rtk`; use `rtk proxy <cmd>` only to
bypass filtering. `rtk grep` format flags (`-c/-l/-L/-o/-Z`) run raw.

- Check: `rtk cargo check -p <package>`
- Test: `rtk cargo nextest run -p <package>` (prefer nextest over `cargo test`)
- Lint: `rtk cargo clippy --workspace --all-targets`
- File-size check: `rtk make check-code-file-lines`

## TUI ownership

- `src/app/render/{screens,arrangements,components,theme}/`: screens own app
  state and content; arrangements own placement and breakpoints; components
  own painting and geometry; theme exposes semantic roles only, primitives
  are private. See `CONTEXT.md` Presentation for term definitions.
- Screens do not call Ratatui, construct `Rect`s, or compute hit targets.
  A structural or visual override lives in the owning arrangement, component,
  or theme — never as a screen-local painter branch.
- This boundary is mandatory for new UI work from this PR forward. Existing
  surfaces migrate individually per
  `openspec/changes/enforce-mbv-ui-design-system/ledger.md`; an unmigrated
  surface is not licence to add another one.
- Workflow, the reuse/override decision table, and the completion checklist:
  `.opencode/skills/mbv-frontend/SKILL.md`.

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
  evidence is a guess, not a fix — say so and stop instead of shipping it.

## Project process

- Design changes live in `openspec/changes/<name>/`; otherwise use GitHub
  Issues/discussions (`slatkin/mbv`) and gists for ad-hoc notes.
- Commit specs/plans/docs with code; merge applied deltas into `openspec/specs/`
  and archive completed changes.
