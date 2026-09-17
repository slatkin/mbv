# Tasks: home-rows-playback-palette

## 1. Canonical painter palette

- [x] 1.1 In `src/app/render/components/media_list/row.rs`, change the two-tone branch's colours: secondary (item title) `TEXT_FOCUS_ACCENT` → `PLAYBACK_TITLE_FG`, primary (container/context) stays/`title_color` → `PLAYBACK_CONTEXT_FG` in both the marquee `parts` vec and the truncation branch; single-part rows keep `title_color` unchanged. Verify: `cargo nextest run -p mbv`.
- [x] 1.2 Update the pinning two-tone tests in `src/app/render/components/media_list.rs` (`episode_row_paints_secondary_title_in_the_focus_accent_role` → playback-roles, plus the marquee/truncation pins) and add: single-part row keeps the ordinary title role; played split row mutes the item title while context keeps gold. Verify: `cargo nextest run -p mbv media_list`.
- [x] 1.3 Update the two-tone role comments in `row.rs` and the `secondary` field doc in `src/app/components/media_list/mod.rs` to the split-row palette contract. Verify: `cargo clippy --workspace --all-targets -- -D warnings` clean.

## 2. Shell-side feed-name resolution

- [ ] 2.1 Add the feed-id → display-name lookup field to `HomeContent` (`src/app/types_playback.rs`), populated in `assign_home_content` (`src/app/shell_home_content.rs`) from the existing config subscription match, and cleared on `HomeContentCleared`. Verify: `cargo nextest run -p mbv shell_home_content` / existing Home tests pass.
- [ ] 2.2 Test: assignment resolves a matching subscription's display name, and a non-matching/`None` `feed_id` yields no entry. Verify: `cargo nextest run -p mbv shell_home_content`.

## 3. Home row projection

- [ ] 3.1 In `project_active_section` (`src/app/components/home_content.rs`), swap `display_name_parts()` for the playback title-parts mapping with the shell-resolved subscription name; primary = context part, secondary = title part, single-part rows unchanged. Verify: `cargo nextest run -p mbv home_content`.
- [ ] 3.2 Tests: Home projects episode→series, audio track→artist, feed entry→subscription, ABS podcast→show; movie/book/feed-without-subscription project single-part; truncation priority still protects the item title. Verify: `cargo nextest run -p mbv home_content`.

## 4. Gates

- [ ] 4.1 Full gates: `cargo nextest run -p mbv`, `cargo nextest run -p mbv-core` (expect unchanged), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt`, `openspec validate --all`. Verify: all clean.
- [ ] 4.2 Tick-integration check that a Home section render through the shell sync pass paints the new palette on split rows and the ordinary role on single-part rows (extend `tests_tick_integration_home.rs` style). Verify: `cargo nextest run -p mbv tests_tick_integration_home`.
