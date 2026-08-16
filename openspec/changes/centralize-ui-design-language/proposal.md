## Why

Every new screen and every visual tweak currently has to be directed page by page. There is no shared
definition of what a screen looks like, so the same decision is re-made in each renderer and then
drifts. Measured across `src/app/render/` today: 49 hand-rolled background fills, 22 longhand
focused/unfocused colour ternaries that already disagree with each other, 13 hand-rolled border
rules despite a shared helper existing, and 99 references to per-screen `two_column`/`is_wide`/
`is_narrow` flags.

The wide/narrow arrangement has the same problem in a sharper form. Each screen tests the width
itself and then branches on that flag throughout its renderer, so the two arrangements are the same
statements with different booleans. `home.rs` derives four aliases from one flag
(`wide_pill_section`, `green_panel_full`, `wide_home_panel_unfocused`, `is_narrow`) and branches on
them at 20 points. There is no edit to narrow Home that is guaranteed not to reach wide Home. That
is the reported bug: changing one mode changes the other.

The existing specs already tried to solve this by naming a screen to copy —
"Podcast libraries use the TV Shows tab composition", "Book libraries use the Music tab composition",
"Wide Music focus uses the Home visual language". The intent was recorded three times, but there was
nothing shared to point at, and copying a style from one screen to another by hand is the task that
most reliably fails. Audiobooks is specified as hero-on-left and is not actually applied as one.
Feeds and home videos have no wide arrangement at all.

## What Changes

### Colour and focus

- Add a semantic token layer over `palette.rs`. Tokens name roles, not hues. The existing raw
  constants stay until callers migrate, then are removed.
- **Focus colour becomes a single central lever.** Shared code owns the focused/unfocused switch;
  screens pass `focused: bool` and never name a colour. This covers the left panel (card and queue)
  as well, whose current pair (`QUEUE_LIST_BG`/`LIBRARY_SIDE_BG`) disagrees with every content panel
  (`BG_GREEN`/`PLAYBACK_PANEL_BG`).
- Per-screen colour exceptions are permitted, but only as **named variants** defined once in the
  token layer and opted into by name. No call site passes a raw `Color`.
- Where two screens disagree today, **the mature screen wins**: movies, TV shows and music define
  the values. Home bends to match. A screen-by-screen audit follows this change separately.

### Arrangements

- There are **two arrangements, not three**. Narrow is hero-on-top with one column.
  - **hero-on-top** — movies, shows, podcasts, feeds, home videos. Never rearranges; the breakpoint
    only changes the list from one column to two.
  - **hero-on-left** — Home, music, audiobooks. Wide only; below the breakpoint these fall back to
    hero-on-top with one column.
- **Screens stop testing the width.** The breakpoint is read once, in shared code. `home.rs`,
  `list.rs`, `audiobookshelf_books.rs` and both sites in `render/mod.rs` lose their width checks.
- **One breakpoint**, centrally managed, keeping today's value. It stops being derived from library
  cell arithmetic (`library_column_width.rs:29` currently hard-asserts `82 == 2*40 + 2`); the library
  column count derives from the breakpoint instead of defining it.
- Shared code **paints**, it does not merely return geometry. It owns hero text layout and wrapping,
  rows, pills, selection highlight, scrollbar, borders and backgrounds. Screens supply data.
- Shared code owns the whole arrangement, including regions the parent currently reaches in to
  paint: `render/mod.rs:378` (Home's narrow background) and `render/mod.rs:526-547` (Music's pills
  row). The parent passes a `Rect` and `focused`, nothing else.
- **Two focusable panes** in hero-on-left, uniformly. Home has none to focus today; that is
  unimplemented content, not a different contract.
- **NEW BEHAVIOUR**: feeds and home videos gain a wide arrangement (hero-on-top). They currently
  render identically at 200 columns and at 60.
- **BREAKING (visual)**: audiobooks is re-applied as a correct hero-on-left. Home changes to match
  the shared definition.

### Per-screen overrides

- Each screen carries **one settings block, in one place**, listing everything it does differently.
  A screen with no block gets the default, so the central lever stays intact.
- Derived from reading the five existing hero implementations, screens differ on exactly:
  image source, image shape (16:9 backdrop vs portrait cover), which metadata lines are shown and in
  what order, colour variant, and which elements are present at all.
- Overrides never cover geometry, the breakpoint, or focus behaviour.

### Mouse hit targets

- Shared code writes **one common set of hit targets**, replacing the four per-screen ways
  `LayoutMain` currently says "rows you can click" (`left_item_rows`, `home.hitmap`,
  `wide_music_track_hitmap`, `audiobookshelf_episode_rows`) and the two ways it says "the right
  pane" (`wide_music_right_area`, `audiobookshelf_book_right_area`).

### Component file boundaries

- One file per UI component, so it is unambiguous where one begins and another ends: card, queue
  list, playback strip, tab bar, status bar, visualizer, library content. `render_main` currently
  draws all of them in a single 445-line function.
- "Component" is the chosen word because **Panel** is already taken by the glossary, where it means
  Library-or-Queue (what Panel mode and Panel focus switch between).
- This may land as a later phase if that is simpler.

## Capabilities

### New Capabilities

- `ui-design-language`: Semantic colour tokens with one central focus lever, named colour variants
  as the only per-screen colour exception, and mature screens as the source of token values.
- `right-panel-arrangements`: Wide and narrow as a right-panel concept distinct from Panel mode; the
  hero-on-top and hero-on-left arrangements and which screens use each; a single centrally-managed
  breakpoint that screens never test themselves; per-screen settings blocks; and one common set of
  mouse hit targets.

### Modified Capabilities

- `library-list-columns`: The column count now derives from the central breakpoint rather than the
  breakpoint being derived from minimum cell width.
- `library-list-hero`: Hero placement and the hero/list split become properties of the arrangement
  rather than of the list renderer.
- `music-library-hero`: "Wide Music focus uses the Home visual language" is replaced by naming the
  hero-on-left arrangement.
- `audiobookshelf-book-browsing`: "Book libraries use the Music tab composition" is replaced by
  naming the hero-on-left arrangement, and the arrangement is actually applied.
- `audiobookshelf-podcast-library-ui`: "Podcast libraries use the TV Shows tab composition" is
  replaced by naming the hero-on-top arrangement.

## Impact

**Glossary** — `CONTEXT.md` gained Wide mode / Narrow mode, Hero-on-top and Hero-on-left under
Presentation during design. Component may need adding.

**Code, by area:**

- `src/app/palette.rs` — semantic token layer added; raw hue constants removed as callers migrate.
- `src/app/mod.rs:186`, `src/app/library_column_width.rs` — breakpoint ownership inverts; the
  compile-time assert pinning it to cell arithmetic is removed.
- `src/app/render/mod.rs` (731 lines) — `render_main` stops reaching into Home and Music; splits
  toward one file per component.
- `src/app/render/home.rs` (726), `list.rs` (775), `music_wide.rs`, `audiobookshelf.rs`,
  `audiobookshelf_books.rs`, `audiobookshelf_book_browser.rs`, `feeds.rs`, `home_video.rs`,
  `home_hero.rs`, `home_latest_row.rs` — adopt shared arrangements; lose their width checks.
- `src/app/layout.rs` — per-screen hit-target fields collapse to one set;
  `LayoutMain::is_wide_music_active()` exists only because those fields are per-screen and goes away.
- `src/app/input_mouse.rs`, `input_mouse_panels.rs`, `lib_cursor_actions.rs` — read the unified hit
  targets instead of per-screen fields.

**File-size cap**: `home.rs` (726), `list.rs` (775) and `render/mod.rs` (731) are already near the
800-line cap, so splitting is required by repo policy regardless of design intent.

**Verification**: per the project's testing rules, no new committed UI snapshot assertions. Evidence
that movies, TV shows and music are unchanged comes from throwaway snapshots captured before and
after each phase and then deleted.

**Not in scope**: left panel layout (colours only); the screen-by-screen visual audit, which the
user will run after this lands.
