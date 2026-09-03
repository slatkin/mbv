# mbv

Rust terminal media client for Emby, Audiobookshelf, and Feeds. Embeds mpv; playback runs in-terminal, via the Local daemon, or packaged `mbvd`.

## Project rules

* Read `CONTEXT.md` for domain vocabulary. *Avoid* terms are incorrect. Add new terms with the change; ask before renaming/colliding with existing terms.
* Read current `docs/adr/` entries before architecture changes.
* Design work lives in `openspec/changes/<name>/`; commit plans/specs/docs with code, merge applied deltas into `openspec/specs/`, then archive completed changes.
* Durable planning belongs in markdown artifacts, not long chat output. Ask the user only about material design/product choices.
* Source ownership:

  * `src/`: interactive binary/TUI; `src/local_daemon.rs` owns Local-daemon bootstrap.
  * `crates/mbv-core/`: runtime/services/providers/config/protocols/queue/source prep/mpv projection; no UI/feed fetch.
  * `crates/mbvd/`: packaged daemon, persistence, sockets.
* Change source-of-truth types before callers.

## Tooling

Call `pi_lens_activate_tools` before code exploration/work.

Prefix command chains with `rtk`; use `rtk proxy <cmd>` only to bypass filtering. `rtk grep -c/-l/-L/-o/-Z` runs raw.

* check: `rtk cargo check -p <package>`
* test: `rtk cargo nextest run -p <package>`; prefer nextest
* lint: `rtk cargo clippy --workspace --all-targets`
* size: `rtk make check-code-file-lines`
* format: `rtk cargo fmt`

Rustfmt is stock edition-2021/max-width-100. Run it for every change and accept all resulting reflow; never revert fmt output. Use `cargo fmt --check` for read-only verification.

Prefer these available CLI tools over any alternative:

* gh: github client
* ketch: web search/scraping
* ast-grep: code structural search, lint, and rewriting

## TUI boundary

`screens` own app state/content; `arrangements` placement/breakpoints; `components` painting/geometry; `theme` semantic roles.

Screens must not call Ratatui, construct `Rect`s, compute hit targets, or contain painter overrides. New UI work must respect this boundary even where legacy code does not.

See `.opencode/skills/mbv-frontend/SKILL.md`; canonical spec: `openspec/specs/ui-design-system/spec.md`. Interactive migration: `docs/architecture/interactive-surface-ledger.md`.

## Constraints

* Source files max 800 lines; split over-limit files in the same change.
* Ctrl changes add capabilities; never bump `CTRL_PROTOCOL_VERSION`.
* Replace flaky tests; do not troubleshoot them.
* Put symbol-specific rules above the symbol.
* Never guess at daemon/process-boundary fixes (`mbvd`, Local daemon, shared data, ctrl, Stay-Alive, Emby/ABS startup races). Before editing, reproduce in the actual failing process, capture real logs/error/response, and establish root cause. Without that evidence, stop and report the fix as speculative.
