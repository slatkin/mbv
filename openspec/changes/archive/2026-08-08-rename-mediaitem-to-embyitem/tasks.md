# Tasks

## 1. Rename

- [x] 1.1 Rename the type `MediaItem` → `EmbyItem` at its definition in
      `crates/mbv-core/src/api_types.rs`, and update every reference across the
      workspace (~92 files, ~332 sites). Prefer a single cargo-aware / IDE
      rename so the tree never passes through a half-renamed state — do not
      hand-edit files one at a time.
- [x] 1.2 Confirm no incidental changes rode along: `git diff` shows **only**
      the `MediaItem` → `EmbyItem` identifier rename — no signature, logic,
      formatting, or comment changes.

## 2. Verify (all must pass)

- [x] 2.1 `cargo check --workspace` green
- [x] 2.2 `cargo test -p mbv-core` green
- [x] 2.3 `cargo clippy --workspace --all-targets` green
- [x] 2.4 `make check-code-file-lines` passes
- [x] 2.5 serde field names unchanged — the rename is wire-invisible; existing
      queue-state / queue-document round-trip tests pass untouched

## 3. Close out

- [x] 3.1 Update CONTEXT.md vocabulary if it names `MediaItem`: the Emby wire
      item type is now `EmbyItem`.
