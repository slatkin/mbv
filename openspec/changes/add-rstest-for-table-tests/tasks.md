## 1. Dependency and baseline

- [x] 1.1 Add `rstest` to `[workspace.dependencies]` in the root `Cargo.toml` starting from `default-features = false` (design D3), and reference it as `rstest.workspace = true` under `[dev-dependencies]` in the root package and in `crates/mbv-core/Cargo.toml`. Verify with `cargo check -p mbv-core --all-targets` and `cargo check -p mbv --all-targets` succeeding, and `cargo tree -p rstest --edges normal` showing no async runtime or timeout path.
- [x] 1.2 Commit the dependency change alone (design D5) and verify `git show --stat` for that commit lists only the two manifests plus `Cargo.lock`.
- [x] 1.3 Record the conversion baseline per target family before touching any test: the test names and counts for the socket `*_returns_none` family, the two `api_tests_parsing` families, and the backdrop `dim_*` set. Verify by `cargo nextest list -p mbv-core | grep -c` and `cargo nextest list -p mbv | grep -c` producing 8, 5, 4 and 7 respectively — the numbers the post-conversion checks compare against.

## 2. Socket decode family (`mbv-core`)

- [x] 2.1 Convert the eight `*_returns_none` tests in `crates/mbv-core/src/audiobookshelf_socket.rs` (lines ~446-518) to one `#[rstest]` `#[case]` table with named cases (`#[case::malformed_json("not json")]`, `#[case::truncated_open_packet("0{{{")]`, …), each name derived from the test it replaces (design D4). Verify `cargo nextest run -p mbv-core audiobookshelf_socket` passes and `cargo nextest list -p mbv-core` shows eight cases named `<fn>::case_N_<name>`.
- [x] 2.2 Verify no assertion was lost in 2.1: the eight named cases each map to one removed test name, with no leftover `*_returns_none` test function and no `#[case]` row lacking an expectation. Verify with a `git diff` read of that file showing eight cases against eight deleted `#[test]` functions, and `cargo nextest list -p mbv-core | grep -c returns_none` returning 0.
- [x] 2.3 Run the per-file gates for 2.1-2.2 and commit that file alone: `cargo fmt --all`, `cargo clippy -p mbv-core --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`. Verify all three pass and the commit touches only `audiobookshelf_socket.rs`.

## 3. Parsing families (`mbv-core`)

- [x] 3.1 Convert the five `parse_item_*_is_folder` tests in `crates/mbv-core/src/api_tests_parsing.rs` (lines 68, 74, 236, 242, 248) to a single `#[case]` table over the `Type` string, with one case name per former test (e.g. `#[case::collection_folder("CollectionFolder")]`). Verify `cargo nextest run -p mbv-core api_tests_parsing` passes and `cargo nextest list -p mbv-core` shows five named cases.
- [x] 3.2 Convert the four `parse_video_info_*` tests in the same file (lines ~352-370) to a `#[case]` table over `(width, height, codec, expected)`. Verify the four cases pass and are individually named, and that `parse_video_info` still receives the same `json!`-built stream input as before.
- [x] 3.3 Verify no assertion was lost in 3.1-3.2: five plus four named cases against nine removed `#[test]` functions, with no remaining `parse_item_*_is_folder` or `parse_video_info_*` single-case test. Verify by reading the `git diff` and confirming `cargo nextest list -p mbv-core | grep -c 'is_folder\|parse_video_info'` returns 0.
- [x] 3.4 Run the per-file gates and commit `api_tests_parsing.rs` alone: `cargo fmt --all`, `cargo clippy -p mbv-core --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`. Verify all three pass and the commit touches only that file.

## 4. Backdrop dim family (root package)

- [x] 4.1 Convert the `dim_*` shape in `src/app/render/components/backdrop.rs` (the measured `dim_rgb_*` trio plus its four identical-shape siblings, per design D8) to one `#[case]` table over `(Color, Color)` with named cases. Verify `cargo nextest run -p mbv dim` passes seven named cases and that `Color::Indexed` passthrough plus the `White`/`Black`/`Reset` cases keep their existing expectations.
- [x] 4.2 Verify no assertion was lost in 4.1: seven cases against seven removed `#[test]` functions, and no remaining single-case `dim_*` test. Verify by reading the `git diff` and `cargo nextest list -p mbv | grep -c 'backdrop.*dim'`, which must equal seven.
- [x] 4.3 Run the per-file gates and commit `backdrop.rs` alone: `cargo fmt --all`, `cargo clippy -p mbv --all-targets -- -D warnings`, `cargo nextest run -p mbv`. Verify all three pass and the commit touches only that file.

## 5. Convention

- [x] 5.1 Add the convention to the testing policy section of `AGENTS.md` (design D6): fixture-varying test families use named `#[case]` tables; `#[case]` is not a mechanism for generating many thin tests; conversions are opportunistic and file-by-file. Verify the line sits inside the existing testing rules rather than as a new standalone section, and that it names both the encouragement and the guard.
- [x] 5.2 Commit the `AGENTS.md` change alone and verify the commit touches only that file.

## 6. Workspace verification

- [x] 6.1 Verify the full gates pass on the whole workspace with the dependency in place: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --release --test-threads=4`. Verify all three succeed and the total test count equals the pre-change count (the four families' 24 tests are now 24 cases, so the total must be unchanged).
- [x] 6.2 Verify no production code changed: `git diff --stat` across the change's commits lists only `Cargo.toml`, `Cargo.lock`, the two manifests, the three test-bearing files and `AGENTS.md`. Fail the check if any other file appears.
- [x] 6.3 Verify the dependency earned its place by comparing test-build wall time before and after (`cargo nextest run --release --test-threads=4` cold and warm). Record the delta in the change notes so a future reviewer can judge the cost.
- [x] 6.4 Comment on #710 with the conversion outcome and the families left opportunistic (the other 13 measured families), so the follow-up is tracked rather than implied by the Open Question.
