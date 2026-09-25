# Tasks

## 1. Resolve the flat-list selection by target

- [x] 1.1 In `src/app/components/tv_content/episode_rows.rs`, make `TvContent::selected_item()` return `self.selected_episode_item()` when `self.flat_episode_mode()` is true, keeping the existing alphabetical-ordinal resolution for the show list. This single change covers every narrow caller in `keyboard.rs` (`handle_key_narrow` Enter/Ctrl+P/Ctrl+A/Ctrl+S/Ctrl+W, `shared_library_effect`) and `panel_owner.rs`. Verify: `cargo check -p mbv`.
- [x] 1.2 Add a component test in `src/app/components/tv_content/tests/` (follow the `writing-tests` skill; hermetic, no sleeps): narrow geometry, `TvContentMode::Latest`, items pushed newest-first where the newest episode is NOT alphabetically first (e.g. "Zeta" then "Alpha"); select the first displayed row, send Enter, assert the emitted `ShellRequest::TvEpisodeActivate` carries the "Zeta" episode id; repeat for Ctrl+P expecting `EmbyLibraryPlay` with the same id. Verify: the test fails before 1.1 and passes after, via `cargo nextest run -p mbv tv_content`.

## 2. Gate

- [x] 2.1 `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv` all pass.
