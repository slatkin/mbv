## 0. Prerequisites

The unrelated pre-existing build errors (the `AudiobookshelfBook` match arms and the private
`home_section_pref`) are fixed in their own change before this one starts; this change assumes a
clean, runnable baseline.

- [ ] 0.1 Archive the two completed-but-unarchived changes (`extend-home-latest-abs-feeds`,
      `redesign-audiobookshelf-book-browsing`) so this change's deltas apply to a current spec base
- [ ] 0.2 Add a throwaway capture harness (uncommitted) that renders a named screen at one narrow and
      one wide width to text, for before/after diffing; "unchanged" means byte-identical text
- [ ] 0.3 Capture baselines for all eight screens; record where audiobooks differs from music at the
      same size, so the intended end state of phase 5 is known rather than discovered

## 1. Colour roles and the central focus lever

- [ ] 1.1 Implement the role layer in `palette.rs` exactly as the "Role vocabulary" table in
      `design.md` specifies, alongside the existing constants (which stay until callers migrate);
      rename `PRUPLE` to `PURPLE`
- [ ] 1.2 Verify the implemented role names match the table character-for-character; no alias or
      synonym is introduced during migration
- [ ] 1.3 Add the semantic role layer taking values from the table, which already resolves every
      disagreement by the mature-screen-wins rule
- [ ] 1.4 Add the named-variant mechanism with zero variants defined (design decision 7)
- [ ] 1.5 Add the single focused/unfocused resolution (`SURFACE_FOCUSED` vs `SURFACE_RESTING`) used
      by all panels and components
- [ ] 1.6 Split `PLAYBACK_PANEL_BG` into `SURFACE_PLAYBACK` and `SURFACE_RESTING` (both `#333c43`);
      read each of the 25 references and assign it to the role of what it draws
- [ ] 1.7 Migrate the 22 longhand focus ternaries in `src/app/render/` to the central lever
- [ ] 1.8 Migrate the 49 hand-rolled background fills to roles
- [ ] 1.9 Migrate the 13 hand-rolled `▔`/`▁` rule sites to the shared border helper
- [ ] 1.10 Bring the left panel onto the central focus lever: `render_queue_panel_frame` and the card
      adopt `SURFACE_FOCUSED`/`SURFACE_RESTING`, a deliberate visible change (design decision 8)
- [ ] 1.11 Remove the now-unused raw constants from `palette.rs`
- [ ] 1.12 Diff against baselines: only the card and queue list surfaces may change (decision 8);
      every content screen must be byte-identical

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

## 3. Extract the components

- [ ] 3.1 Define the `HeroShell` component by merging `hero_block_shell` (`list.rs:97`) and
      `render_selected_block_borders` (`widgets.rs:212`); it takes a focus state and resolves its
      background via the focus lever
- [ ] 3.2 Define the `SelectionMarker` component — the unified edge block (a thin AQUA block at the
      list's outer edge, directional in two-column mode, no inline glyph, no `##` prefix) —
      replacing the no-op `selection_marker` (`widgets.rs:316`)
- [ ] 3.3 Define the `Scrollbar` component with one entry point and a role (not a `color` arg),
      retiring the duplicated `render_scrollbar` / `render_right_scrollbar` split
- [ ] 3.4 Define the `Hero` component (image, metadata lines and order, overview wrapping; it owns
      how many lines fit the rows it is given) and the `List` component (rows, selection marker,
      scrollbar; returns row hit targets), extracted from movies/TV (`list.rs` `top_hero_layout`
      path) and grouped Music
- [ ] 3.5 Apply the unified `SelectionMarker` to every list (replaces the single-column `▌`+`##`,
      inline `▍`, Home narrow gutter `▎`, books' blank) — deliberate visible change, so the
      "unchanged" claims of the mature-screen phases do not apply to the marker
- [ ] 3.6 Diff against baselines: apart from the selection marker, movies, TV and music must be
      unchanged

## 4. Assemble hero-on-top

- [ ] 4.1 Define the hero-on-top arrangement as a composition of `Hero`, `List`, `PillBar`,
      `Scrollbar`, `SelectionMarker`
- [ ] 4.2 Define the per-screen declaration: image source, image shape, metadata lines and order,
      colour variant, element presence
- [ ] 4.3 Move movies/TV onto the arrangement, with their differences expressed only in their
      declaration
- [ ] 4.4 Move podcasts onto the arrangement; delete its duplicated hero implementation in
      `audiobookshelf.rs`
- [ ] 4.5 Diff against baselines: movies, TV and podcasts must be unchanged

## 5. Assemble hero-on-left

- [ ] 5.1 Define the hero-on-left arrangement as a composition of the same components, including the
      two-pane focus model and the pill row at the top of the list pane
- [ ] 5.2 Move hero text wrapping into the `Hero` component; screens supply unwrapped title and
      overview strings (replaces the two different call shapes at `home.rs:181` and `home.rs:260`)
- [ ] 5.3 Implement the below-breakpoint fallback to hero-on-top with one column, shared by all
      hero-on-left screens
- [ ] 5.4 Move grouped Music onto the arrangement
- [ ] 5.5 Diff against baselines: music must be unchanged

## 6. Adopt: Home and audiobooks

- [ ] 6.1 Move Home onto hero-on-left; delete `two_column` and its four derived aliases
      (`wide_pill_section`, `green_panel_full`, `wide_home_panel_unfocused`, `is_narrow`)
- [ ] 6.2 Remove the mode parameters from `home_latest_row.rs` (`is_narrow`,
      `wide_home_panel_unfocused`); rows stop being mode-aware
- [ ] 6.3 Move Home's narrow hero onto the shared hero-on-top fallback: it keeps its
      image-beside-metadata wrap (already the shared shape) and gains the `HeroShell`
      (`▁`/`▔`) borders it lacks today (design decision 2)
- [ ] 6.4 Move audiobooks onto hero-on-left, correcting the gap recorded in task 0.3; delete its
      bespoke hero and browser painting
- [ ] 6.5 Confirm Home and audiobooks now render identically to music at the same size, apart from
      their declared differences
- [ ] 6.6 Split any file left over the 800-line cap

## 7. New wide arrangements: feeds and home videos

- [ ] 7.1 Move feeds onto hero-on-top; it gains a wide arrangement and a two-column list
- [ ] 7.2 Move home videos onto hero-on-top; same
- [ ] 7.3 Visually review both at wide widths — this is new behaviour with no prior design

## 8. Unified mouse hit targets

- [ ] 8.1 Define one hit-target representation produced by the components (row targets from `List`,
      pill targets from `PillBar`, pane targets from the arrangement)
- [ ] 8.2 Replace the four per-screen row representations in `LayoutMain` (`left_item_rows`,
      `home.hitmap`, `wide_music_track_hitmap`, `audiobookshelf_episode_rows`) with it
- [ ] 8.3 Replace the two per-screen pane rects (`wide_music_right_area`,
      `audiobookshelf_book_right_area`) and delete `LayoutMain::is_wide_music_active()`
- [ ] 8.4 Collapse the per-screen branches in `input_mouse.rs`, `input_mouse_panels.rs` and
      `lib_cursor_actions.rs` onto the common form
- [ ] 8.5 Verify by hand: click an item row, a pill, and each pane on all eight screens, in both
      arrangements

## 9. Chrome component files

- [ ] 9.1 Finish `render_main`'s decomposition so it only composes and no longer paints inline: move
      the inline backgrounds (`render/mod.rs:330,344`) and the reach-ins (`:378`, `:526`) behind
      their components
- [ ] 9.2 Add **Component** to `CONTEXT.md` under Presentation, defined so it does not collide with
      Panel

## 10. Close-out

- [ ] 10.1 Delete the throwaway capture harness and all captures
- [ ] 10.2 Run `rtk cargo clippy --workspace --all-targets` and
      `rtk cargo nextest run -p mbv -p mbv-core`
- [ ] 10.3 Run `rtk make check-code-file-lines`
- [ ] 10.4 Confirm `CONTEXT.md` matches what was built (Wide/Narrow mode, Hero-on-top, Hero-on-left
      were added during design)
- [ ] 10.5 Merge the applied deltas into `openspec/specs/` and archive this change
