## 0. Prerequisites

- [ ] 0.1 Archive the two completed-but-unarchived changes (`extend-home-latest-abs-feeds`,
      `redesign-audiobookshelf-book-browsing`) so this change's deltas apply to a current spec base
- [ ] 0.2 Resolve the pre-existing build errors in the working tree (`AudiobookshelfBook` match arms
      in `player_types.rs`, `player_run_commands.rs`, `player_reporting.rs`; private
      `home_section_pref` in `tests_home_latest.rs`) so "unchanged" comparisons are meaningful
- [ ] 0.3 Add a throwaway capture harness (uncommitted) that renders a named screen at one narrow and
      one wide width to text, for before/after diffing
- [ ] 0.4 Capture baselines for all eight screens; record where audiobooks differs from music at the
      same size, so the intended end state of phase 5 is known rather than discovered

## 1. Colour roles and the central focus lever

- [ ] 1.1 Enumerate every distinct colour role in use, mapping each of the 46 constants in
      `palette.rs` to a role, and identify the ~10 that are pure aliases
- [ ] 1.2 **Review gate**: agree the role names with the user before any call site is migrated
      (renaming later is a large mechanical diff — see design Risks)
- [ ] 1.3 Add the semantic role layer to `palette.rs` alongside the existing constants, taking values
      from the mature screens where two screens disagree
- [ ] 1.4 Add the named-variant mechanism, with no variants defined until a screen needs one
- [ ] 1.5 Add the single focused/unfocused resolution used by all panels and components
- [ ] 1.6 Split `PLAYBACK_PANEL_BG` into its two roles (now-playing strip vs resting content panel)
      and visually check every adjacency where the shared value was providing continuity
- [ ] 1.7 Migrate the 22 longhand focus ternaries in `src/app/render/` to the central lever
- [ ] 1.8 Migrate the 49 hand-rolled background fills to roles
- [ ] 1.9 Migrate the 13 hand-rolled `▔`/`▁` rule sites to the shared border helper
- [ ] 1.10 Bring the left panel onto the central focus lever: `render_queue_panel_frame` and the card
      stop naming `QUEUE_LIST_BG`/`LIBRARY_SIDE_BG`
- [ ] 1.11 Remove the now-unused raw constants from `palette.rs`
- [ ] 1.12 Diff against baselines: no screen may change

## 2. One breakpoint

- [ ] 2.1 Move the breakpoint to a single design constant, keeping today's value
- [ ] 2.2 Remove the compile-time assert in `library_column_width.rs:29`; derive cell width from the
      breakpoint instead of the breakpoint from cell width
- [ ] 2.3 Remove the dead branch in `right_panel_content_area` (`widgets.rs:38`), where
      `TAB_LEFT_PAD` and `TAB_LEFT_PAD_TWO_COL` are both 2 and the column count is consulted for no
      effect
- [ ] 2.4 Remove the two breakpoint tests in `render/mod.rs` (Home's narrow background at :378, the
      Music pills carve at :526) and the geometry they own, leaving the parent passing only a `Rect`
      and focus state
- [ ] 2.5 Diff against baselines: no screen may change

## 3. Extract hero-on-top

- [ ] 3.1 Define the hero-on-top arrangement by extracting from movies/TV, absorbing
      `top_hero_layout` and `hero_block_shell`; it owns hero, list, column count, pills, selection,
      scrollbar, borders and backgrounds
- [ ] 3.2 Define the per-screen declaration: image source, image shape, metadata lines and order,
      colour variant, element presence
- [ ] 3.3 Move movies/TV onto the arrangement, with their differences expressed only in their
      declaration
- [ ] 3.4 Move podcasts onto the arrangement; delete its duplicated hero implementation in
      `audiobookshelf.rs`
- [ ] 3.5 Diff against baselines: movies, TV and podcasts must be unchanged

## 4. Extract hero-on-left

- [ ] 4.1 Define the hero-on-left arrangement by extracting from grouped Music, including the
      two-pane focus model and the pill row at the top of the list pane
- [ ] 4.2 Move hero text wrapping into the arrangement; screens supply unwrapped title and overview
      strings (replaces the two different call shapes at `home.rs:181` and `home.rs:260`)
- [ ] 4.3 Implement the below-breakpoint fallback to hero-on-top with one column, shared by all
      hero-on-left screens
- [ ] 4.4 Move grouped Music onto the arrangement
- [ ] 4.5 Diff against baselines: music must be unchanged

## 5. Adopt: Home and audiobooks

- [ ] 5.1 Move Home onto hero-on-left; delete `two_column` and its four derived aliases
      (`wide_pill_section`, `green_panel_full`, `wide_home_panel_unfocused`, `is_narrow`)
- [ ] 5.2 Remove the mode parameters from `home_latest_row.rs` (`is_narrow`,
      `wide_home_panel_unfocused`); rows stop being mode-aware
- [ ] 5.3 Fold Home's bespoke narrow hero (image beside metadata) into the shared narrow fallback
- [ ] 5.4 Move audiobooks onto hero-on-left, correcting the gap recorded in task 0.4; delete its
      bespoke hero and browser painting
- [ ] 5.5 Confirm Home and audiobooks now render identically to music at the same size, apart from
      their declared differences
- [ ] 5.6 Split any file left over the 800-line cap

## 6. New wide arrangements: feeds and home videos

- [ ] 6.1 Move feeds onto hero-on-top; it gains a wide arrangement and a two-column list
- [ ] 6.2 Move home videos onto hero-on-top; same
- [ ] 6.3 Visually review both at wide widths — this is new behaviour with no prior design

## 7. Unified mouse hit targets

- [ ] 7.1 Define one hit-target representation produced by both arrangements
- [ ] 7.2 Replace the four per-screen row representations in `LayoutMain` (`left_item_rows`,
      `home.hitmap`, `wide_music_track_hitmap`, `audiobookshelf_episode_rows`) with it
- [ ] 7.3 Replace the two per-screen pane rects (`wide_music_right_area`,
      `audiobookshelf_book_right_area`) and delete `LayoutMain::is_wide_music_active()`
- [ ] 7.4 Collapse the per-screen branches in `input_mouse.rs`, `input_mouse_panels.rs` and
      `lib_cursor_actions.rs` onto the common form
- [ ] 7.5 Verify by hand: click an item row, a pill, and each pane on all eight screens, in both
      arrangements

## 8. Component files

- [ ] 8.1 Decide whether this phase lands here or as a follow-up change (design Open Questions),
      based on how much splitting phases 1-7 already forced
- [ ] 8.2 Split `render_main` into one file per component: card, queue list, playback strip, tab bar,
      status bar, visualizer, library content
- [ ] 8.3 Add **Component** to `CONTEXT.md` under Presentation, defined so it does not collide with
      Panel

## 9. Close-out

- [ ] 9.1 Delete the throwaway capture harness and all captures
- [ ] 9.2 Run `rtk cargo clippy --workspace --all-targets` and
      `rtk cargo nextest run -p mbv -p mbv-core`
- [ ] 9.3 Run `rtk make check-code-file-lines`
- [ ] 9.4 Confirm `CONTEXT.md` matches what was built (Wide/Narrow mode, Hero-on-top, Hero-on-left
      were added during design)
- [ ] 9.5 Merge the applied deltas into `openspec/specs/` and archive this change
