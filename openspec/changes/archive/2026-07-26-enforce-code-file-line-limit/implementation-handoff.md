# Implementation Handoff

## Inventory

The final classifier inventory contains 224 governed tracked files. The largest
file is `src/app/input_power_music_track_navigation_tests.rs` at 756 lines.
The repository-wide checker reports zero violations. The classifier is the
documented set of `.rs`, `.lua`, `.sh`, `.py`, `.js`, `.ts`, `.tsx`, `.c`, `.h`,
`.cpp`, `.hpp`, `Makefile`, `PKGBUILD`, `PKGBUILD-git`, and `.githooks/*` paths;
documentation, configuration, generated output, and binary paths remain
excluded.

## Structural Decisions

- The checker is the single shell implementation used by `make check-code-file-lines` and CI.
- App test inventories were moved into subsystem-named siblings using the existing parent-module privacy model.
- Rendering, input, and browse production code was split at existing method and responsibility boundaries.
- `mbv-core` keeps its public module APIs and lexical privacy through ordered component includes; inline tests were extracted into physical test files without changing test identities.
- Lua components are concatenated by `scripts/mbv.lua` into one chunk, preserving shared lexical state and mpv registration order. The loader derives its directory from the script source when mpv does not provide one.
- Release tarballs, Cargo deb assets, and both PKGBUILD paths ship the loader and all `mbv_*.lua` components.

## Verification

- `make check-code-file-lines` with a temporary index containing all worktree files: passed, zero violations.
- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --all-targets`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo test --workspace`: passed, 635 app tests and 255 `mbv-core` tests.
- `cargo test --release && cargo build --release`: passed.
- Lua component and concatenated-loader syntax checks: passed.
- Five-second mpv smoke test with `scripts/mbv.lua`: loaded without Lua errors; the timeout stopped the intentionally idle mpv process.
- Release bundle check: passed; all 10 Lua files were present.
- Normalized Rust test identity comparisons: passed for app action/input/render modules and all six `mbv-core` modules.
- Temporary-index `git diff --cached --check`: passed.
