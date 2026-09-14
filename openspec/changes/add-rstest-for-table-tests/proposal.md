## Why

The refreshed coverage audit on #697 measured **17 shape-duplicate test families** — 46 test bodies that are identical once literals are blanked and differ only in fixture. Four of them are large enough to matter today:

| family | where |
|---|---|
| 8× `*_returns_none` | `crates/mbv-core/src/audiobookshelf_socket.rs:446-518` |
| 5× `parse_item_*_is_folder` | `crates/mbv-core/src/api_tests_parsing.rs:68, 74, 236, 242, 248` |
| 4× `parse_video_info_*` | `crates/mbv-core/src/api_tests_parsing.rs:352-370` |
| 3× `dim_rgb_*` | `src/app/render/components/backdrop.rs:52-70` |

Each case asserts a different value, so this is maintenance debt rather than coverage noise. It matters because the debt refills itself: today the cheapest way to add a case is to copy the previous test, and cost is what actually drives test shape in this repo. Table-driven tests make the non-duplicated form the shorter one.

## What Changes

- Add `rstest` (0.27) as a **test-only** dependency: declared once in `[workspace.dependencies]`, then as a `[dev-dependencies]` entry in the two packages that own the target families (root `mbv` for `src/app/render`, `mbv-core` for the rest).
- Convert the four families above to `#[case]` tables, using **named cases** (`#[case::malformed_json("not json")]`) so every case keeps its own name in failure output. A table that reports "case 4 of 8 failed" is not an acceptable outcome.
- Keep every existing assertion. This change re-packages tests; it does not add, weaken, or delete coverage.
- Record the convention in `AGENTS.md` next to the existing testing rules: fixture-varying families use `#[case]` tables; `#[case]` must never be used to mass-generate thin tests.
- Leave the other 13 measured families alone. Conversion is opportunistic and file-by-file as those files are touched — no repo-wide sweep.

Explicitly **not** in scope: rewriting one-off tests that read clearly as they are, converting async/daemon fixtures, or adopting any other test framework. `proptest` is tracked separately in #711.

## Capabilities

### New Capabilities

None. This is test tooling: no behaviour of the system changes, no requirement changes. The change opts out of specs via `skip_specs: true` in `.openspec.yaml`, per the schema's rule for pure tooling changes.

### Modified Capabilities

None. No spec under `openspec/specs/` describes test conventions — the repo's testing policy lives in `AGENTS.md`, which is where the new convention is recorded.

## Impact

- **Dependencies**: +1 dev-dependency (`rstest 0.27`). No runtime, release-artifact, or packaging impact. No deny/licence gate exists in the repo, so no allowlist to update.
- **Files**: workspace `Cargo.toml`, root `Cargo.toml` and `crates/mbv-core/Cargo.toml` (dev-deps), `crates/mbv-core/src/audiobookshelf_socket.rs`, `crates/mbv-core/src/api_tests_parsing.rs`, `src/app/render/components/backdrop.rs`, `AGENTS.md`.
- **CI**: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --release --test-threads=4` all apply to the converted tests — rstest expands to ordinary `#[test]` functions, so nextest naming and filtering are unaffected, but the `-D warnings` clippy gate covers the generated code.
- **Risk**: low. Test-only; a failure mode to watch is loss of per-case diagnosability, which named cases prevent and which the tasks verify explicitly.
- Relates to #710 (this change) and #697 (the audit that measured it).
