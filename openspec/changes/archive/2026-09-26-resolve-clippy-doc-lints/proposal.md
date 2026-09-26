# Proposal

## Why

Issue #816 deferred four clippy `pedantic` doc-comment lints (`doc_markdown`, `missing_errors_doc`, `missing_panics_doc`, `too_long_first_doc_paragraph`) out of #804 so they could be decided on their merits. They're the last thing blocking `cargo clippy --workspace --all-targets -- -D warnings` from being clean, which #804 (closed) and the CI lint gate both depend on.

## What Changes

- **Scope added 2026-09-26 by user decision:** the original four-lint premise was stale — a full unmasked clippy run showed ~24 further pedantic findings blocking `-D warnings`. Section 5 of tasks.md was added to fix all of them (`needless_pass_by_value` ×17 signature changes in `src/app/dispatch/library/`, `allow_attributes_without_reason` ×2, `unused_self` ×1, `cast_precision_loss` ×1 line, `must_use_candidate` ×2, `missing_debug_implementations` ×1, `cargo_common_metadata` readme ×5 crates).
- `doc_markdown` (135 sites, current count): fix mechanically via `cargo clippy --fix`, keep the lint enabled.
- `too_long_first_doc_paragraph` (51 sites): drop from `[workspace.lints.clippy]` — no rustdoc is published for this repo, so the lint's summary-listing rationale doesn't apply.
- `missing_errors_doc` (131 sites): drop from `[workspace.lints.clippy]` — the flagged fns are only reachable by same-repo path-dependency siblings (`mbv`, `mbvd`), not external consumers, and hand-writing 131 `# Errors` sections risks manufacturing the filler-slop the lint enablement was meant to catch.
- `missing_panics_doc` (35 sites): keep the lint enabled, hand-write real `# Panics` sections naming the actual panic source (index/slice/unwrap/divide) at each site — this is the one lint with genuine safety signal in a codebase where a panic in the playback path takes down the TUI.

## Capabilities

No spec-level behavior changes — this is lint configuration and documentation only. `skip_specs: true` is set in `.openspec.yaml`.

## Impact

- `Cargo.toml`: `[workspace.lints.clippy]` table (remove two lint keys).
- `crates/mbv-core/src/{ctrl.rs, player/controller.rs, player/proxy.rs, player/submit.rs, remote_player.rs, remote_player/connect.rs}`, `crates/mbv-keybinds/src/{chord.rs, config.rs, registry.rs}`, `crates/mbv-net/src/mock_http.rs`: add `# Panics` doc sections (35 sites).
- Every file with a `doc_markdown` finding: mechanical backtick fixes via `clippy --fix`, no manual review needed per file.
- No runtime/behavior change from the doc-lint rows; the section-5 signature changes alter internal Rust signatures only (no observable behavior). `cargo clippy --workspace --all-targets -- -D warnings` becomes reachable as a CI gate.
