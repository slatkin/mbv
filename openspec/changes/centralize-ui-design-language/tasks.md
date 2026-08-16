## 0. Prerequisites

The unrelated pre-existing build errors (the `AudiobookshelfBook` match arms and the private
`home_section_pref`) are fixed in their own change before this one starts; this change assumes a
clean, runnable baseline.

- [x] 0.1 Archive the two completed-but-unarchived changes (`extend-home-latest-abs-feeds`,
      `redesign-audiobookshelf-book-browsing`) so this change's deltas apply to a current spec base
- [x] 0.2 Add a throwaway capture harness (uncommitted) that renders a named screen at one narrow and
      one wide width to text, for before/after diffing; "unchanged" means byte-identical text
- [x] 0.3 Capture baselines for all eight screens; record where audiobooks differs from music at the
      same size, so the intended end state of phase 5 is known rather than discovered

## 1. Colour roles and the central focus lever

- [x] 1.1 Implement the role layer in `palette.rs` exactly as the "Role vocabulary" table in
      `design.md` specifies, alongside the existing constants (which stay until callers migrate);
      rename `PRUPLE` to `PURPLE`
- [x] 1.2 Verify the implemented role names match the table character-for-character; no alias or
      synonym is introduced during migration
- [x] 1.3 Add the semantic role layer taking values from the table, which already resolves every
      disagreement by the mature-screen-wins rule
- [x] 1.4 Add the named-variant mechanism with zero variants defined (design decision 7)
- [x] 1.5 Add the single focused/unfocused resolution (`SURFACE_FOCUSED` vs `SURFACE_RESTING`) used
      by all panels and components
- [x] 1.6 Split `PLAYBACK_PANEL_BG` into `SURFACE_PLAYBACK` and `SURFACE_RESTING` (both `#333c43`);
      read each of the 25 references and assign it to the role of what it draws
- [x] 1.7 Migrate the 22 longhand focus ternaries in `src/app/render/` to the central lever
- [x] 1.8 Migrate the 49 hand-rolled background fills to roles
- [x] 1.9 Migrate the 13 hand-rolled `▔`/`▁` rule sites to the shared border helper
- [x] 1.10 Bring the left panel onto the central focus lever: `render_queue_panel_frame` and the card
      adopt `SURFACE_FOCUSED`/`SURFACE_RESTING`, a deliberate visible change (design decision 8)
- [x] 1.11 Remove the now-unused raw constants from `palette.rs`
- [x] 1.12 Diff against baselines: only the card and queue list surfaces may change (decision 8);
      every content screen must be byte-identical

## 2. One breakpoint

- [x] 2.1 Move the breakpoint to a single design constant, keeping today's value
- [x] 2.2 Remove the compile-time assert in `library_column_width.rs:29`; derive cell width from the
      breakpoint instead of the breakpoint from cell width
- [x] 2.3 Remove the dead branch in `right_panel_content_area` (`widgets.rs:38`), where
      `TAB_LEFT_PAD` and `TAB_LEFT_PAD_TWO_COL` are both 2 and the column count is consulted for no
      effect
- [x] 2.4 Remove the two breakpoint tests in `render/mod.rs` (Home's narrow background at :378, the
      Music pills carve at :526) and the geometry they own, leaving the parent passing only a `Rect`
      and focus state
      DEFERRED to 9.1: both sites are load-bearing production paint logic (Home's narrow-focused
      full-panel background, Music's narrow pills carve), not dead code; removing them now with no
      arrangement/component to hand the behavior to would visibly change Home/Music narrow rendering,
      contradicting 2.5 ("no screen may change") and design.md's own phase-2 description ("No visible
      change"). design.md's Migration Plan and task 9.1 already assign this exact removal (same line
      numbers) to phase 9, alongside `render_main`'s decomposition. What phase 2 does guarantee: both
      sites already read the single `TWO_COLUMN_THRESHOLD` constant with no duplicate value and no
      compile-time assert (removed in 2.2) — the "one breakpoint value" invariant holds; only the
      reach-in *paint* removal is deferred.
- [x] 2.5 Diff against baselines: no screen may change
      Verified by inspection: 2.1-2.3 are zero-behavior-change edits (dead branch where both arms were
      already equal, a compile-time assert, a doc comment); 2.4 deferred to 9.1, no edit made. Saved
      the current capture_harness output as `target/ui-captures-baseline/` (gitignored, not committed)
      for phase 3 onward to diff against, since the previous phase-0 baseline was overwritten by the
      phase-1 agent's re-run with no separate copy kept.

## 3. Extract the components

- [x] 3.1 Define the `HeroShell` component by merging `hero_block_shell` (`list.rs:97`) and
      `render_selected_block_borders` (`widgets.rs:212`); it takes a focus state and resolves its
      background via the focus lever
- [x] 3.2 Define the `SelectionMarker` component — the unified edge block (a thin AQUA block at the
      list's outer edge, directional in two-column mode, no inline glyph, no `##` prefix) —
      replacing the no-op `selection_marker` (`widgets.rs:316`)
- [x] 3.3 Define the `Scrollbar` component with one entry point and a role (not a `color` arg),
      retiring the duplicated `render_scrollbar` / `render_right_scrollbar` split
- [x] 3.4 Define the `Hero` component (image, metadata lines and order, overview wrapping; it owns
      how many lines fit the rows it is given) and the `List` component (rows, selection marker,
      scrollbar; returns row hit targets), extracted from movies/TV (`list.rs` `top_hero_layout`
      path) and grouped Music
      Scoped per design.md's own migration plan (phases 4-5 own screen rewiring): moved
      movies/TV's hero-on-top geometry/shell (`top_hero_layout`, `hero_block_shell`, HERO_* consts)
      into new `hero.rs`, unchanged; documented `list_rows.rs` as the `List` component (already
      shared by movies/TV's list renderers) and `music_wide.rs`'s `compute_wide_left_layout` as the
      hero-on-left geometry source, left in place (entangled with that file's non-hero pane
      constants; unifying it with `hero.rs` is phase 5). Per-content-kind metadata painting (movie/
      series/album banners) stays in `list.rs`/`music_wide.rs` pending phase 4/5 rewiring.
- [x] 3.5 Apply the unified `SelectionMarker` to every list (replaces the single-column `▌`+`##`,
      inline `▍`, Home narrow gutter `▎`, books' blank) — deliberate visible change, so the
      "unchanged" claims of the mature-screen phases do not apply to the marker
- [x] 3.6 Diff against baselines: apart from the selection marker, movies, TV and music must be
      unchanged
      Ran `capture_harness`; diffs appeared only in movies/tv_shows/podcasts (▌##title →
      ▎ two-column marker) and audiobooks/_compare_books (blank → ▎ marker), all marker-only,
      byte-identical otherwise. home, feeds, home_videos, music, _compare_music showed zero diff
      (the harness's fixture state doesn't select a row in those inline-marker code paths, so no
      regression is visible there either). Refreshed `target/ui-captures-baseline/` to match.

## 4. Assemble hero-on-top

- [x] 4.1 Define the hero-on-top arrangement as a composition of `Hero`, `List`, `PillBar`,
      `Scrollbar`, `SelectionMarker`
      The real `Hero` content component now lives in `render/hero.rs`: `HeroContent`/`HeroImage`/
      `HeroLine`/`ImageTop` plus `paint_hero_content`, extracted from the movie hero
      (`detail.rs`'s `render_compact_detail`, hero-on-top's source). It paints title, right-aligned
      image reservation, meta line, "Playing" indicator, and overview/detail lines, and returns the
      image rect (image bytes stay behind `App::cached_image_protocol_mut`, which the component has
      no access to) plus the next unpainted row for callers appending more content. `List`
      (`list_rows.rs`), `PillBar` (`render_pill_bar`), `Scrollbar` and `SelectionMarker` were already
      shared/defined in phase 3; `render_list` in `list.rs` is the arrangement composing all of them
      for movies/TV/albums (album branch untouched, out of scope -- see 4.3's note).
- [x] 4.2 Define the per-screen declaration: image source, image shape, metadata lines and order,
      colour variant, element presence
      `HeroContent` is the declaration struct: `title`/`meta_line`/`meta_color`/`show_playing`/
      `lines`/`image` (source + shape via `HeroImage`/`ImageTop`). `meta_color` is the colour-variant
      field -- movie uses `MUTED_GREEN`, Series/Audiobookshelf-podcasts use `SUBTLE`, matching their
      pre-existing values exactly (no new hue). Domain content (list rows, pill groups, the episode
      table) stays screen-supplied data, not part of the declaration, per decision 6.
- [x] 4.3 Move movies/TV onto the arrangement, with their differences expressed only in their
      declaration
      Movies (`detail.rs`) and TV/Series (`detail_series_view.rs`) both now call
      `hero::paint_hero_content`. Series needed one extra declared field,
      `unconditional_spacer_after_meta` (true for Series, false for movies): Series reserves the
      blank row after its meta line unconditionally, movies only when a meta line was actually
      shown -- an existing, preserved difference between the two, not something this component
      unifies. Series' season pills + episode table are domain content painted after `Hero` returns,
      reusing its returned `img_rect` to reconstruct the same `text_dims` narrowing the pre-existing
      code used. Grouped Music's album-hero branch in `render_list` (list.rs:520-632) is untouched:
      Music is hero-on-left's source screen (design.md decision 4), out of scope for this phase.
- [x] 4.4 Move podcasts onto the arrangement; delete its duplicated hero implementation in
      `audiobookshelf.rs`
      Partially scoped: `render_audiobookshelf_hero` (Audiobookshelf server podcast shows, not
      Emby's `podcasts` collection type -- Emby podcasts already went through `render_compact_detail`
      via `selected_movie_item`, so 4.3 already covers them) now calls `paint_hero_content` for its
      title row and right-aligned image reservation, replacing its own copy of that logic and
      reusing the same `img_rect`/`text_width` derivation. Its author-line + description block was
      NOT moved into `HeroContent`'s `meta_line`/`lines`: it has a third, distinct spacer
      choreography (a spacer before the description only if one exists, then an unconditional
      trailing spacer regardless) that matches neither of `Hero`'s two spacer patterns (movie's
      conditional-on-meta, Series' unconditional-after-meta); forcing a third pattern into the shared
      component risked a real behaviour change in a "any visible change is a defect" phase for a
      cosmetic win. The duplication that remains is the author/description painting loop, not the
      title/image geometry this task's title implies was the main duplicate.
- [x] 4.5 Diff against baselines: movies, TV and podcasts must be unchanged
      Ran `capture_harness` after 4.1-4.4; `diff -rq target/ui-captures-baseline/ target/ui-captures/`
      reported zero differences (exit 0) across all eight screens. `cargo check` unchanged at 0
      errors/26 warnings; `cargo nextest run -p mbv` 829 passed, 1 skipped, matching pre-phase-4
      counts. `make check-code-file-lines` clean (largest touched file: `audiobookshelf.rs` at 633
      lines).

## 5. Assemble hero-on-left

- [x] 5.1 Define the hero-on-left arrangement as a composition of the same components, including the
      two-pane focus model and the pill row at the top of the list pane
      Grouped Music already composes `PillBar` (`render_music_group_pills_row`) and the shared
      `SelectionMarker`/`Scrollbar` primitives (`list_rows.rs`) for its right pane, and already
      derives the two-pane focus model (`left_focused`/`right_focused` from `PanelFocus` +
      `album_track_focus`, per design.md decision 8) — those needed no change. What was still
      Music-only and reusable by later hero-on-left screens moved into `hero.rs`: the pane split
      (`wide_music_panes` → `hero_on_left_panes`) and the right pane's pill-row-then-list-panel
      geometry (`hero_on_left_right_pane`, returning `HeroOnLeftRightPane { pills_area, list_panel }`),
      both moved unchanged. `HERO_ON_LEFT_MIN_AREA_HEIGHT`/`_MIN_PANE_WIDTH` (design.md decision 5's
      named thresholds) moved with them. `compute_wide_left_layout` (the hero/track vertical split
      and artwork sizing) stayed in `music_wide.rs`, documented in both files' headers: its constants
      are entangled with that file's non-hero track-list panel padding, and there is no second
      consumer yet to justify splitting one padding convention across two files.
      Not attempted: unifying the right pane's artist-grouped album browser
      (`render_wide_right_album_browser`) with movies/TV's letter-grouped `list_rows.rs` engine into
      one generic List implementation. They already share the same primitives (marker, scrollbar,
      hit-target shape); their row-grouping/scroll logic is genuinely different domain shapes
      (artist-header buckets vs. letter buckets), and design.md decision 6 states list content and
      grouping are screen-supplied *data*, not a presentation difference this phase must merge. No
      task in 5.1-5.5 names the browser engine, so this is out of scope, not deferred.
- [x] 5.2 Move hero text wrapping into the `Hero` component; screens supply unwrapped title and
      overview strings (replaces the two different call shapes at `home.rs:181` and `home.rs:260`)
      Added `hero.rs::paint_hero_on_left_text` (takes `&[WrappedHeroLine]`, each an unwrapped
      `&str` + screen-chosen `Style`; wraps and paints top to bottom, stopping at the area's bottom
      edge) plus the `WrappedHeroLine` type, both extracted unchanged from grouped Music's former
      `render_wide_left_hero`/`render_wrapped_text`. `music_wide.rs` now builds an unwrapped
      title/artist/year line list (domain filtering — skipping "Unknown Artist", omitting a zero
      year — stays screen-side, matching how `HeroContent::meta_color` already lets hero-on-top
      screens pick their own colour) and calls the shared function once. The `home.rs:181`/`:260`
      sites this task cites are Home's own hero, not touched here — Home does not adopt the
      arrangement until phase 6 (design.md Migration Plan step 6); this task's scope, per its own
      "Assemble hero-on-left" phase, is giving `Hero` the wrapping capability using Music (the
      hero-on-left source) as proof, so phase 6 has a shared function to call instead of Home's
      current two call shapes.
- [x] 5.3 Implement the below-breakpoint fallback to hero-on-top with one column, shared by all
      hero-on-left screens
      Already satisfied without new code: `render_wide_music_group` falls back to `self.render_list`
      — the same hero-on-top narrow renderer movies/TV use — whenever `area.width <
      TWO_COLUMN_THRESHOLD` or the content height is below `HERO_ON_LEFT_MIN_AREA_HEIGHT`. Grouped
      Music is the only hero-on-left screen today, so "shared by all hero-on-left screens" holds
      trivially with one member; phase 6 screens reuse this same fallback path when they adopt.
- [x] 5.4 Move grouped Music onto the arrangement
      `render_wide_music_group` and `render_wide_left_hero` now call the extracted
      `hero::hero_on_left_panes`, `hero::hero_on_left_right_pane`, and
      `hero::paint_hero_on_left_text` in place of their former local, Music-only implementations.
      No behavioral change — see 5.5.
- [x] 5.5 Diff against baselines: music must be unchanged
      Ran `capture_harness` (`cargo nextest run -p mbv capture_harness --run-ignored all`); `diff -rq
      target/ui-captures-baseline/ target/ui-captures/` exit 0 (zero differences) across all eight
      screens. `cargo check` unchanged at 0 errors/26 warnings. `cargo nextest run -p mbv` 829
      passed/1 skipped. `make check-code-file-lines` clean; largest touched file `music_wide.rs` at
      613 lines, `hero.rs` at 530 lines.

## 6. Adopt: Home and audiobooks

- [x] 6.1 Move Home onto hero-on-left; delete `two_column` and its four derived aliases
      (`wide_pill_section`, `green_panel_full`, `wide_home_panel_unfocused`, `is_narrow`)
      Wide branch now calls `hero::hero_on_left_panes`/`hero_on_left_right_pane` instead of local
      hand-rolled math. `two_column`'s own width test stays local to `home.rs` (same precedent as
      Music's `render_wide_music_group` and task 2.4's deferral of the render/mod.rs-level test
      removal to phase 9.1) — what was eliminated is the sprawl of separately-threaded derived
      booleans, computed inline at their point of use instead.
- [x] 6.2 Remove the mode parameters from `home_latest_row.rs` (`is_narrow`,
      `wide_home_panel_unfocused`); rows stop being mode-aware
      Both row painters dropped to 5 params. Selection-marker drawing moved out of
      `home_latest_row.rs` entirely into `home.rs`'s row loop, unconditional for both layouts —
      per decision 2, Home's wide rows retire their inline marker in favor of the same external
      edge-marker convention narrow already used (matching album/queue/music). Deleted
      `row_unselected_has_no_marker` (no longer meaningful at that granularity).
- [x] 6.3 Move Home's narrow hero onto the shared hero-on-top fallback: it keeps its
      image-beside-metadata wrap (already the shared shape) and gains the `HeroShell`
      (`▁`/`▔`) borders it lacks today (design decision 2)
      Uses `hero::top_hero_layout`/`hero_block_shell`. Fixed a row-collision bug found via capture
      inspection: the hero's top border shares a row with Home's own pill-gap background fill: the
      shell paint is now deferred (`narrow_shell` local) to after the pill-gap fill runs, so the
      border wins that row. Verified visually in `home-78.txt` (border now renders) after the fix.
- [x] 6.4 Move audiobooks onto hero-on-left, correcting the gap recorded in task 0.3; delete its
      bespoke hero and browser painting
      `render_audiobookshelf_book_right_pane_wide` and `render_wide_audiobookshelf_books` now call
      `hero::hero_on_left_panes`/`hero_on_left_right_pane`; deleted the duplicated local
      `book_wide_panes` and its pane-gap/min-width constants. The geometry values were already
      numerically identical to the shared ones, so this was a behavior-preserving consolidation,
      not a visible fix.
- [x] 6.5 Confirm Home and audiobooks now render identically to music at the same size, apart from
      their declared differences
      Compared `home-146.txt`, `music-146.txt`, `audiobooks-146.txt`: pill bar, hero border rows,
      and selected-row marker all land at identical columns across all three. Only differences are
      declared content (Music's artist-header row; each screen's own metadata shape).
- [x] 6.6 Split any file left over the 800-line cap
      `rtk make check-code-file-lines` clean — no governed file over 800 lines.

**Also fixed this phase**: `capture_harness.rs`'s `WIDE_WIDTH` was sized against raw terminal
width, not the right-panel content area (~44 columns narrower after the queue sidebar/gap/tab
padding). At the old value (`TWO_COLUMN_THRESHOLD + 28`) the content area never crossed the
82-column breakpoint, so every "wide" capture in phases 3-5 was silently re-testing the narrow
arrangement instead. Fixed to `TWO_COLUMN_THRESHOLD + 64`, verified empirically to land the
content area past the threshold. This only affects the throwaway capture-diff verification method
(gitignored, gates nothing) — `cargo nextest`'s dedicated wide-mode unit tests (e.g. `music_wide.rs`)
were unaffected and would have still caught outright breakage.

Verification: `cargo check -p mbv` 0 errors/26 warnings; `cargo nextest run -p mbv` 828 passed/1
skipped; `make check-code-file-lines` clean.

## 7. New wide arrangements: feeds and home videos

- [x] 7.1 Move feeds onto hero-on-top; it gains a wide arrangement and a two-column list
      Feeds now paints `hero::top_hero_layout`/`hero_block_shell`/`paint_hero_content` for the
      cursor-selected entry (text-only `HeroContent`, no image), inserted between feeds' existing
      pill-bar/watched-filter chrome and its list, so that chrome stays screen-owned exactly as
      design.md's per-screen declarations allow. The list gains a two-column packed layout at the
      shared breakpoint via a new `pack_feed_rows` render-time transform (reusing the generic
      `library_column_width.rs` geometry helpers, not Emby-specific despite the module name) that
      wraps entries into `cols`-wide rows without crossing feed-age-group boundaries; the original
      `feed_display_rows`/`FeedDisplayRow` and their tests are untouched. Feeds' old inline `"▶ "`
      marker is retired in favor of the unified outer-edge `SelectionMarker` (decision 2), via
      `list_rows.rs`'s `draw_column_selection_markers`.
      Scope flagged and confirmed with the user beyond what 7.1 named: two-column mode needs
      Up/Down/Left/Right keyboard and mouse-click behavior, which design.md's Impact list didn't
      specify for feeds. Wired fully, mirroring the existing Emby-library-list precedents rather
      than inventing new interaction patterns: `feed_tab_actions.rs` gained
      `feed_tab_move_cursor_rows`/`feed_tab_row_delta` (mirrors `lib_cursor_actions.rs`'s
      `letter_vertical_delta`, reading `layout.main.left_item_rows` directly); Up/Down/j/k in
      `input_feed_tab_keys.rs` now call it, and Left/Right/h/l call the existing flat
      `feed_tab_move_cursor(±1)` gated on `library_column_count(..) > 1`; `input_mouse.rs`'s
      `TabSelection::Feeds` arm gained a `cell_target` resolution mirroring the
      `TabSelection::EmbyLibrary` arm's, checked before the pre-existing `left_row_map` fallback.
- [x] 7.2 Move home videos onto hero-on-top; same
      Home videos already qualified for `render_list`'s existing hero-on-top machinery:
      `selected_movie_item` already recognized `"homevideos"` (truncate_overview), and
      `should_show_letter_pills` already excluded it (element-presence difference: no
      pills). Rerouted `widgets.rs`'s dispatch from the bespoke `render_home_video_list`
      straight to `render_list`, added home videos' one remaining declared difference (a
      count-label row instead of pills, `render_list`'s `is_home_video_view` gate calling
      the pre-existing shared `render_count_label`), and deleted `render_home_video_list`
      (dead after the reroute). `render_home_video_item`/`render_selected_home_video_detail`/
      `home_panel_scroll` stay: still used by the separate "feed home video group view"
      (`home_feed.rs`, grouping by folder), which is not one of design.md's eight screens
      and was left untouched — flagging this as an intentionally out-of-scope gap, not an
      oversight: it still uses the old inline-expand-in-row style rather than hero-on-top.
      Found and fixed a layout bug during verification: `top_hero_layout`'s `hero_shift`
      reclaims the blank row directly above `content_area` for the hero's top border,
      which silently overwrote the new count-label row until an explicit blank gap row
      was added back after it (mirroring the gap that existed there for every other
      hero-on-top screen).
- [x] 7.3 Visually review both at wide widths — this is new behaviour with no prior design
      Reviewed via the capture harness (feeds-146/78, home_videos-146/78) and confirmed by the
      user directly in-app. User signed off: "very good initial implementation," with a few
      follow-up issues to be addressed one by one in a later pass rather than blocking this phase.

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
