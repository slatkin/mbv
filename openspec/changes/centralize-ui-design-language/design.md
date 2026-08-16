## Context

See `proposal.md — Why` for motivation. This section records only the current state and the
constraints that shaped the approach.

**Where the responsive decision lives today.** Four screens plus the parent renderer each test the
width independently, all reading `TWO_COLUMN_THRESHOLD` (`src/app/mod.rs:186`):

| Site | What it decides |
|---|---|
| `render/home.rs:77` | Home's hero placement, then 20 dependent branches |
| `render/list.rs:169` | whether grouped Music takes the wide path |
| `render/audiobookshelf_books.rs:75` | whether books take the wide path |
| `library_column_width.rs:36` | library list column count |
| `render/mod.rs:378` | paints Home's narrow background from outside Home |
| `render/mod.rs:526` | carves Music's pills row from outside Music |

**Three different splitting styles are already in the tree.** Books and Music dispatch to separate
wide/narrow functions. Home interleaves one flag through a single 630-line function, re-deriving it
three times under different names (`green_panel_full.is_none()`, `is_narrow`,
`wide_home_panel_unfocused`) and threading two of them into `home_latest_row.rs`. Home is the
outlier and the source of the reported bug.

**The breakpoint is owned by the wrong thing.** `library_column_width.rs:29` asserts at compile time
that `TWO_COLUMN_THRESHOLD == 2 * LIBRARY_COLUMN_MIN_WIDTH + LIBRARY_COLUMN_GAP`. The width at which
Home rearranges its hero is therefore pinned to the minimum readable width of a library list cell.

**Five independent hero implementations exist**: `home_hero.rs` (`KeepWatchingHeroLayout`),
`detail.rs` (`render_hero_title_row` + compact banner), `audiobookshelf.rs`, `music_wide.rs`,
`audiobookshelf_books.rs`. Books and podcasts are near-identical copy-paste of each other.

**One shared piece already works.** `render_pill_bar` (`widgets.rs:352`) plus the
`pill-selector-presentation` spec is the only presentation concern that has never drifted and never
needs per-screen direction. It is the model for this change.

**Constraints:**

- Source files cap at 800 lines; `list.rs` (775), `render/mod.rs` (731) and `home.rs` (726) are
  already close, so splitting is mandatory regardless of design intent.
- Project testing rules forbid new committed UI snapshot assertions — the UI churns too fast for
  them to be worth their maintenance.
- Movies, TV shows and music are the mature screens and must move as little as possible.
- `CONTEXT.md` already binds **Panel** to Library-or-Queue and lists "view mode" and "layout mode"
  under `_Avoid_` for Panel mode, so neither word is available for this concept.

## Goals / Non-Goals

**Goals:**

- A visual change is made in one place and reaches every screen.
- Wide and narrow can be edited independently, structurally, not by convention.
- A new screen inherits a complete arrangement without being individually designed.
- Where a screen genuinely differs, that difference is findable in one place.

**Non-Goals:**

- Redesigning how anything looks. This change moves ownership; the screen-by-screen visual audit is
  the user's separate follow-up.
- Left panel layout. Only its focus colours join the central lever.
- Overlays, modals, the settings surface and the search sidebar. They are not hero-plus-list screens
  and do not use the arrangements.
- Making the hero focusable on Home. The two-pane contract allows it; supplying content does not
  happen here.

## Decisions

### 1. Two arrangements, not three

Narrow is hero-on-top with one column, not a separate design. This means five of the eight screens
(movies, shows, podcasts, feeds, home videos) never rearrange at all — the breakpoint only changes
their list column count — and the three that do rearrange (Home, music, audiobooks) share a single
fallback that already exists and is exercised daily.

*Alternative rejected:* treating narrow as its own arrangement. It would have meant building and
maintaining three presentations where the third is a strict subset of the first, and would have left
the hero-on-top screens with a mode change they do not actually have.

### 2. Shared code paints; it does not return geometry

The shared arrangement code owns hero text layout and wrapping, row rendering, pills, selection
highlight, scrollbar, borders and backgrounds. Screens supply data.

*Alternative rejected:* geometry-only helpers that return `Rect`s and let each screen paint. This is
exactly what exists — `top_hero_layout` and `hero_block_shell` have been shared by three screens for
some time — and the tree still accumulated 49 hand-rolled background fills and 13 hand-rolled border
rules. Geometry-only leaves every paint decision at the call site, which is the behaviour being
removed.

**Consequence to accept:** because the hero's text must wrap differently depending on whether the
image sits above or beside it (`home.rs:181` vs `home.rs:260` pass different widths and padding to
the same function), text wrapping moves into the shared code and screens hand over unwrapped
strings. The shared code is therefore substantially larger than a frame-drawing helper.

**Narrow hero shell is uniform.** Home's narrow hero is not a bespoke composition: its
image-beside-metadata with overview wrapping below the image is already the shared hero-on-top
shape (`detail.rs` `text_dims` narrows text beside the poster, then re-wraps full-width past it).
What distinguishes Home narrow today is only the absent `hero_block_shell` — it draws no `▁`/`▔`
borders, where movies/TV/books/podcasts all do. The shared narrow fallback SHALL draw the bordered
`hero_block_shell`, and Home narrow gains borders to match. Image aspect (16:9 half-width vs fixed
portrait) remains a declared `image shape` difference, not a shell concern.

**Selection marker is one glyph.** The marker variants in use today (music's inline `▍`, the
multi-column edge `▎`/`▏`, Home narrow's gutter `▎`, and books' blank) collapse to a single
central marker: the thinnest block glyph (`▏` U+258F). Selection marks SHALL NOT vary between
lists; the arrangement owns the glyph and its placement, and screens cannot override it. The tab
bar is the exception and keeps its own selected-tab marker.

### 3. The width is tested once, outside every screen

No screen evaluates the breakpoint. `home.rs` loses `two_column` and its four derived aliases;
`list.rs`, `audiobookshelf_books.rs` and both `render/mod.rs` sites lose their checks. The parent
passes a `Rect` and a focus state and nothing else.

*Alternative rejected:* keeping the check per-screen but tidying it into a computed layout struct
before rendering. The render path would still be shared between the two arrangements, so an edit
intended for one would still reach the other. That is the current bug with better formatting.

### 4. Extract from the mature screens rather than design fresh

Grouped Music becomes the hero-on-left definition; movies/TV becomes the hero-on-top definition
(already half-extracted as `top_hero_layout`). Home, audiobooks, feeds and home videos then adopt.

*Alternative rejected:* designing a new shared presentation and migrating everything onto it. It
would put the mature screens at the greatest risk, inverting the stated priority, and would stand up
shared code with no proven consumer.

This also makes "mature screens change least" structurally true rather than a promise checked at
review: they are the reference, so by construction they move least. Where two screens disagree
today, the mature screen's value wins and Home bends.

### 5. One breakpoint, with ownership inverted

Its value does not change. It becomes a design constant that the library's cell width derives from,
rather than a number derived from the library's minimum cell width. The compile-time assert pinning
the two together is removed.

*Alternative rejected:* one breakpoint per decision (four constants). More precise, but nothing today
wants them to differ, and four knobs is four things to keep consistent. Reconsider if a screen is
later found to cross over at the wrong width.

The width breakpoint is not the only responsive threshold — it is only the one screens currently
test themselves. The arrangement also owns a height floor (`MIN_WIDE_AREA_HEIGHT`), a minimum pane
width (`MIN_PANE_WIDTH`), the below-image metadata side-width floor, and the hero-suppression rule
when height cannot fit a hero and a usable list. These are arrangement-owned thresholds, not
per-screen decisions, and screens own none of them.

### 6. Per-screen differences are declared together, not expressed at the point of use

Each screen carries one block listing what it does differently. A screen declaring nothing gets the
defaults, so the central lever survives.

*Alternative rejected:* passing deviations at the call site. It fails the stated requirement that
overrides be easy to locate — answering "what does this screen do differently?" would again mean
reading its whole renderer.

**What a declaration may cover**, derived by reading all five existing hero implementations rather
than speculated:

| Item | Evidence it genuinely varies |
|---|---|
| hero image source | Emby item id + type list (`Backdrop/Primary/Logo` for movies, `Primary/Backdrop` otherwise) vs Audiobookshelf server URL + `library_item_id`, with a different fetch per kind |
| hero image shape | Home computes 16:9 backdrops (`w*9/32`); books and podcasts use portrait covers via `SERIES_IMAGE_COLS` |
| metadata lines and order | Home: title / show name / duration+progress / overview. Books: title / author / narrator. Movies: title / year / runtime / overview |
| colour variant | see decision 7 |
| element presence | not every screen has pills, an image, or a count row |

Nothing else was found to vary. Declarations may not cover geometry, the breakpoint, or focus
behaviour.

**On image shape specifically:** it sits close to geometry, which declarations are otherwise
forbidden from touching, so it was raised for review and resolved in favour of keeping it. It is a
property of the source data, not a layout preference — a portrait cover is a different shape from a
16:9 backdrop regardless of arrangement — and it is the clearest example of the class of exception
this mechanism exists for. The arrangement adapts to the shape it is given; it does not dictate it.

The boundary that keeps this from swallowing geometry: a declaration may state *what the content
is*, never *where it goes*. Shape is a fact about the artwork; position is a fact about the
arrangement.

### 7. Colour exceptions are named variants

A screen opts into a variant defined once with the roles; no call site passes a literal colour.

*Alternative rejected:* free-form colour overrides. Same expressive power, but the second screen with
the same need invents a third shade instead of reusing the first. That is how `LIBRARY_SIDE_BG` came
to serve three unrelated jobs (right-column base, unfocused queue, and Home's selected row).

### 8. Focus colouring is one lever, covering the left panel

Screens pass focus and never name a colour. Focus is two inputs, not one bool: the existing
`PanelFocus` (which panel is focused) plus, for hero-on-left screens only, a pane bit naming which
pane (`left`/`right`). Hero-on-top screens have one focusable region and pass no pane bit. Grouped
Music already derives exactly this (`left_focused = library_focused && track_active`,
`right_focused = library_focused && !track_active`); the arrangement consumes the two inputs rather
than a single `focused: bool`, which cannot express both the hero's panel-level brightness and the
track list's pane-level brightness at once.

The queue and card join this even though their layout does not, because their current pair
(`QUEUE_LIST_BG`/`LIBRARY_SIDE_BG`) already disagrees with every content panel's
(`BG_GREEN`/`PLAYBACK_PANEL_BG`), and leaving them out would preserve the exact complaint: having to
direct a change panel by panel.

### 9. One set of mouse hit targets

`LayoutMain` currently has four representations of "rows you can click" (`left_item_rows`,
`home.hitmap`, `wide_music_track_hitmap`, `audiobookshelf_episode_rows`) and two of "the right pane"
(`wide_music_right_area`, `audiobookshelf_book_right_area`). `LayoutMain::is_wide_music_active()`
exists only because those fields are per-screen. The arrangement produces one common form.

Without this, drawing would be unified but clicking would not, and each new screen would still add a
field plus a branch in `input_mouse.rs`.

### 10. Components get their own files; "Panel" keeps its meaning

`render_main` draws roughly a dozen distinct things in 445 lines, which is why it is unclear where
one element ends and the next begins. Each gets a file: card, queue list, playback strip, tab bar,
status bar, visualizer, library content. They are called **components**, because `CONTEXT.md` binds
**Panel** to Library-or-Queue.

This is sequenced last because nothing else depends on it, and the 800-line cap will already have
forced much of it.

### 11. Verification is by throwaway comparison, not committed tests

Project rules forbid new committed UI snapshot assertions. To evidence "the mature screens did not
change", each phase captures rendered output for the affected screens at one narrow and one wide
width before the change, diffs after, and deletes the captures. Mechanical evidence, no churn cost.

Existing structural tests that are not brittle (for example the column-count boundary test in
`library_column_width.rs`) stay.

## Risks / Trade-offs

**Home has no render tests at all.** `home_tests.rs` tests only a scroll helper that lives in
another file, and `tests_home_latest.rs` (18 tests) never renders. Home is also the screen changing
most. → Home's phase relies entirely on throwaway capture plus direct visual check; treat it as the
phase most likely to need a second pass.

**Token naming is a one-way door in practice.** Once several hundred call sites use a role name,
renaming is a large mechanical diff. → Settle the role names in phase 1 review, before migration
begins, not while it is under way.

**`PLAYBACK_PANEL_BG` does two unrelated jobs** — the actual now-playing strip, and "resting content
panel" on five screens. Splitting them will reveal places where the shared value was silently
providing visual continuity between the playback strip and an adjacent panel. → Expect one or two
seams to look wrong on first render and need a deliberate call; do not assume the split is neutral.

**Feeds and home videos gain a wide arrangement they have never had.** This is new behaviour, not a
refactor, and no design for it exists beyond "hero-on-top like the others". → Land those two last,
after the arrangement is proven by screens that already had a wide form.

**Audiobooks is specified as hero-on-left but is not applied as one.** The delta makes the spec
enforceable, but the current visual gap is unmeasured. → Diff books against music at the same size
before starting, so the intended end state is known rather than discovered.

**Big-bang risk across eight screens plus the palette.** → Phased commits, each independently
reviewable and revertable, in the order below.

**Two completed changes are unarchived** (`extend-home-latest-abs-feeds`,
`redesign-audiobookshelf-book-browsing`, both at 100%). Their deltas are not yet merged into
`openspec/specs/`, so this change's deltas may be written against a stale base. → Archive both before
implementation starts.

**Trade-off accepted:** the shared code becomes large and central. A bug in it affects every screen
at once, where today a bug affects one. This is the deliberate exchange for a single point of
control, and is the same bet already made — successfully — with `render_pill_bar`.

## Migration Plan

Phases are independently reviewable and land in order. Each ends with the throwaway comparison of
decision 11.

1. **Colour roles and the focus lever.** Additive; raw constants stay. No visible change anywhere,
   including the left panel. This is the phase where role names are agreed.
2. **One breakpoint.** Ownership inverts, value unchanged; the compile-time assert is removed;
   `render/mod.rs` stops testing it. No visible change.
3. **Hero-on-top extracted** from movies/TV; podcasts moves onto it. Mature screens, so any visible
   change here is a defect.
4. **Hero-on-left extracted** from grouped Music. Same standard.
5. **Adoption:** Home, then audiobooks. Visible change expected and intended; Home bends to the
   shared definition.
6. **New wide arrangements:** feeds, home videos.
7. **Unified mouse hit targets**, with the per-screen `LayoutMain` fields removed.
8. **Component files.** May be deferred to a follow-up change if phases 1-7 have already satisfied
   the 800-line cap.

**Rollback:** each phase is a separate commit against unchanged behaviour for phases 1-4, so any of
them can be reverted independently. Phases 5-6 change appearance deliberately and would be reverted
as a unit with their spec deltas.

## Open Questions

- Should the shared arrangement own the *count* of hero metadata lines, or only their order and
  content, when a screen is height-constrained? Deferrable: it affects the shared code's internals,
  not the specs or the phase order.
- Whether phase 8 lands here or as a follow-up change. Deferrable: it depends on how much splitting
  phases 1-7 already force, which is not knowable until phase 7.
