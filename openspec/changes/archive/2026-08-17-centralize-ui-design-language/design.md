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
- Making Home's hero focusable. It is a non-focusable preview; whether it ever becomes focusable
  is deferred and out of scope for this change.

## Design language

The deliverable the title names is a *language*, and a language has a structure and a vocabulary.
Both are recorded here so the change reads as a design system rather than as a refactor.

### Three tiers

1. **Primitives** — raw hues. The 45 constants in `palette.rs` as they are today, made private.
   After this change nothing outside `palette.rs` names a primitive.
2. **Roles** — the public API. A role names what a colour *means* (a resting surface, a selected
   row, a rule), never what hue it is. Every call site uses a role; changing a role changes every
   use.
3. **Variants** — named deviations from a role (decision 7), defined once beside the roles and opted
   into by name. Nothing else may override a role.

A new screen writes against roles and variants only: it cannot invent a hue the language does not
already contain, and it cannot re-answer a question the language already answers.

### Role vocabulary (final)

Every constant in `palette.rs` is assigned to exactly one role below. Names are SCREAMING_SNAKE,
grouped by category prefix. The value column is today's constant, unchanged. Where several constants
share a hue they become one role; where one constant serves two jobs it becomes two roles with the
same value (`SURFACE_PLAYBACK` / `SURFACE_RESTING`). `PRUPLE` is renamed `PURPLE`; nothing else is
renamed or retired except as the table states. This table is the implementation; no role name is
invented or re-decided during migration.

**Surfaces**

| Role | Value | Collapses |
|---|---|---|
| `SURFACE_BACKDROP` | `#2d353b` | `LIBRARY_SIDE_BG`, `PLAYBACK_INDICATOR_BG` |
| `SURFACE_CHROME` | `#1e2326` | `DARK_BG`, `QUEUE_BUTTON_FOCUSED_BG` |
| `SURFACE_PANEL` | `#3c424a` | `PANEL_BG` |
| `SURFACE_FOCUSED` | `#3c4841` | `BG_GREEN`, `MEDIA_SELECTED_BG`, `QUEUE_COLUMN_FOCUSED_BG` |
| `SURFACE_RESTING` | `#333c43` | `PLAYBACK_PANEL_BG` — the resting-content / unfocused half |
| `SURFACE_PLAYBACK` | `#333c43` | `PLAYBACK_PANEL_BG` — the now-playing-strip half |
| `SURFACE_ACCENT_SOFT` | `#48584e` | `BG_GREEN_SOFT`, `SCROLLBAR`, `TRACK_BLOCK_BG` |
| `SURFACE_ITEM_FOCUSED` | `#535353` | `FOCUSED` |
| `SURFACE_STATUS_PILL` | `#282828` | `STATUS_PILL_BG` |

`QUEUE_LIST_BG` is retired: the focused queue list adopts `SURFACE_FOCUSED`, and its hue remains
available only through `SURFACE_ACCENT_SOFT`.

**Text**

| Role | Value | Collapses |
|---|---|---|
| `TEXT_PRIMARY` | `#e6e6e6` | `TEXT` |
| `TEXT_SECONDARY` | `#9e9e9e` | `SUBTLE` |
| `TEXT_MUTED` | `#6c6c6c` | `MUTED` |
| `TEXT_EMPHASIS` | `#fdf6e3` | `WHITE` |
| `TEXT_SOFT` | `#f4ead3` | `SOFT_WHITE` |
| `TEXT_ON_ACCENT` | `#1a1a1a` | `BASE` |
| `TEXT_ON_STATE` | `#1e2326` | `TOAST_FG` |
| `TEXT_DETAIL` | `#6c766c` | `MUTED_GREEN` |
| `TEXT_QUEUE_UNFOCUSED` | `#48584e` | `QUEUE_UNFOCUSED_FG` |
| `TEXT_PLAYBACK` | `#83c092` | `PLAYBACK_CONTENT_FG` |
| `TEXT_PLAYBACK_META` | `#859289` | `PLAYBACK_META_FG` |

**Accents**

| Role | Value | Collapses |
|---|---|---|
| `ACCENT` | `#35a77c` | `AQUA` — selection marker, watched, folders |
| `ACCENT_BLUE` | `#3a94c5` | `FOAM` |
| `ACCENT_GREEN` | `#93b259` | `GREEN` |
| `ACCENT_SAGE` | `#a7c080` | `IRIS` — active tab, focused pill text |
| `ACCENT_WARM` | `#dbbc7f` | `YELLOW` |
| `ACCENT_ORANGE` | `#e59875` | `ORANGE` |
| `ACCENT_PURPLE` | `#d699b6` | `PRUPLE` (renamed) |

**Rules**

| Role | Value | Collapses |
|---|---|---|
| `RULE` | `#46545f` | `SEEK_TRACK` — the hero shell border and the seek track are one role, exactly as they share it today |
| `BORDER_UNFOCUSED` | `#3f3f3f` | `OVERLAY` |

**State**

| Role | Value | Collapses |
|---|---|---|
| `STATE_ERROR` | `#e57e80` | `RED`, `TOAST_BG` |
| `STATE_SUCCESS` | `#648c5a` | `TOAST_BG_SUCCESS` |
| `STATE_WARNING` | `#b49650` | `TOAST_BG_WARNING` |

**Pill selector** — the existing `PILL_SELECTOR_*` group is already a role group; it is renamed
without value changes:

| Role | Value | Collapses |
|---|---|---|
| `PILL_ROW_BG` | `#1e2326` | `PILL_SELECTOR_ROW_BG` |
| `PILL_BG` | `#1e2326` | `PILL_SELECTOR_BG` |
| `PILL_FG` | `#495156` | `PILL_SELECTOR_FG` |
| `PILL_SELECTED_BG` | `#3a94c5` | `PILL_SELECTOR_SELECTED_BG` |
| `PILL_SELECTED_FG` | `#1e2326` | `PILL_SELECTOR_SELECTED_FG` |
| `PILL_OVERFLOW_FG` | `#3c4841` | `PILL_SELECTOR_OVERFLOW_FG` |

The focus lever (decision 8) selects `SURFACE_FOCUSED` vs `SURFACE_RESTING` from the two-input focus
model, so a screen supplies focus and never names either. This change defines **zero variants**:
every colour disagreement is resolved by the mature-screen-wins rule (decision 4), so no screen needs
one yet. The variant mechanism exists so a future screen can declare one without reaching for a
literal; a variant added later is appended to this table, never defined at a call site.

## Component catalogue

A reusable component is the unit a screen or arrangement composes. Its contract: given a `Rect`, its
data, and a focus state, it paints itself and returns its mouse hit targets. It never reads the
breakpoint, never tests which screen it is on, and never names a colour — it uses roles. The
arrangement decides *position and focus* and *which components are present*; it does not paint their
contents.

`render_pill_bar` is the one presentation concern that has never drifted, and it is the model for
every row below. The catalogue records what each component is and what this change does to it.

| Component | Today | This change |
|---|---|---|
| `PillBar` | shared, never drifted (`widgets.rs:339`); already returns hit targets | keep as-is |
| `Scrollbar` | shared, but `render_scrollbar` takes a `color` arg while `render_right_scrollbar` hardcodes `SCROLLBAR` | role, not arg; one entry point |
| `HeroShell` | two near-identical ▁/▔ painters: `hero_block_shell` (`list.rs:97`) and `render_selected_block_borders` (`widgets.rs:212`) | merge into one; takes focus state, resolves bg via the lever |
| `Hero` | five bespoke implementations (`home_hero.rs`, `detail.rs`, `audiobookshelf.rs`, `music_wide.rs`, `audiobookshelf_books.rs`) | extract; owns image, metadata lines and order, overview wrapping; takes unwrapped strings + image shape |
| `List` | per-screen row rendering + markers | extract; owns rows, the selection marker, and its scrollbar; returns row hit targets |
| `SelectionMarker` | a no-op `selection_marker` (`widgets.rs:316`); four competing markers in the tree | make real: the unified edge block (decision 2) |
| `QueuePanelFrame` | `render_queue_panel_frame` (`widgets.rs:258`) | joins the focus lever (task 1.10) |
| `Card`, `TabBar`, `PlaybackStrip`, `StatusBar`, `Visualizer` | already separate files (`card.rs`, `chrome_tabs.rs`, `chrome_status.rs`, `visualizer.rs`) | formalize as components; stop being painted inline by `render_main` |

The two arrangements are thin compositions of `Hero`, `List`, `PillBar`, `Scrollbar`, and
`SelectionMarker`. This is stronger than the earlier "shared code paints" wording: the components
being the *only* painters is what removes the 49 hand-rolled backgrounds and 13 hand-rolled rules,
because a screen cannot reach past a component to paint a different background.

## Decisions

### 1. Two arrangements, not three

Narrow is hero-on-top with one column, not a separate design. This means five of the eight screens
(movies, shows, podcasts, feeds, home videos) never rearrange at all — the breakpoint only changes
their list column count — and the three that do rearrange (Home, music, audiobooks) share a single
fallback that already exists and is exercised daily.

*Alternative rejected:* treating narrow as its own arrangement. It would have meant building and
maintaining three presentations where the third is a strict subset of the first, and would have left
the hero-on-top screens with a mode change they do not actually have.

### 2. Components paint; arrangements compose them

A reusable component owns its painting; the arrangement positions components and decides focus, and
does not paint their contents (see "Component catalogue"). The arrangement reads the breakpoint once
and hands each component a `Rect` and a focus state; each component returns its hit targets. Screens
supply data.

*Alternative rejected:* geometry-only helpers that return `Rect`s and let each screen paint. This is
exactly what exists — `top_hero_layout` and `hero_block_shell` have been shared by three screens for
some time — and the tree still accumulated 49 hand-rolled background fills and 13 hand-rolled border
rules. Geometry-only leaves every paint decision at the call site, which is the behaviour being
removed.

*Alternative rejected:* one arrangement object that paints everything itself — hero text, rows,
pills, selection, scrollbar, borders and backgrounds. This removes drift but replaces eight drifting
renderers with one god-object, and it strands the components that are already shared (`PillBar`,
`Scrollbar`, `HeroShell`) inside an arrangement they cannot be reused from. Composition gives the
same single point of control in smaller, reusable units.

**Consequence:** the hero's text must wrap differently depending on whether the image sits above or
beside it (`home.rs:181` vs `home.rs:260` pass different widths and padding to the same function), so
text wrapping moves into the `Hero` component and screens hand over unwrapped strings. The `Hero`
component is therefore substantially larger than a frame-drawing helper, and it owns the full
metadata layout — order, content, and how many lines fit the rows it is given — since line count is
a function of available height, not a per-screen decision.

**Consequence:** the shared components become the single point of control. A bug in one affects every
screen that uses it, where today a bug affects one. This is the same bet already made — successfully
— with `render_pill_bar`; control is spread across small single-purpose components, so a defect is
isolated to one concern (marker, shell, list) rather than one screen.

**Narrow hero shell is uniform.** Home's narrow hero shares the image-beside-metadata shape (with
overview wrapping below the image) with the mature hero-on-top (`detail.rs` `text_dims`), but it is
the only screen that draws no `hero_block_shell` — no `▁`/`▔` borders, where movies/TV/books/
podcasts all do. Folding Home narrow into the shared fallback therefore ADDS the borders: a visible
change to Home narrow, accepted by the "Home bends to match" rule, not a no-op. Image aspect (16:9
half-width vs fixed portrait) remains a declared `image shape` difference, not a shell concern.

**Selection marker is unified on the two-column list's convention.** Every list SHALL render its
selection marker the way the two-column library list does today: a thin AQUA block at the list's
outer edge, directional in two-column mode (`▎` at the left column's left edge, `▏` at the right
column's right edge), with no inline glyph and no `##` title prefix. This retires the
single-column `▌`+`##` indicator (`list_rows.rs:97-98`), the inline `▍` used by music, the album
list, the queue and Home's wide rows, and books' blank marker. It is a deliberate visible change
to every list, so it is an explicit exception to "mature screens change least" and to the
"unchanged" claims of the component-extraction phases. The tab bar is the exception and keeps its
own selected-tab marker.

### 3. The width is tested once, outside every screen

No screen evaluates the breakpoint. `home.rs` loses `two_column` and its four derived aliases;
`list.rs`, `audiobookshelf_books.rs` and both `render/mod.rs` sites lose their checks. The parent
reads the breakpoint once, resolves which arrangement each screen gets, and passes a `Rect`, a focus
state, and that resolved arrangement — nothing else. No component reads the breakpoint.

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
today, the mature screen's value wins: Home adopts movies/TV/music's colour values (task 1.3), the
bordered hero shell, and the shared narrow fallback. Home is never a value source — it is always an
adopter.

### 5. One breakpoint, with ownership inverted

Its value does not change. It becomes a design constant that the library's cell width derives from,
rather than a number derived from the library's minimum cell width. The compile-time assert pinning
the two together is removed.

*Alternative rejected:* one breakpoint per decision (four constants). More precise, but nothing today
wants them to differ, and four knobs is four things to keep consistent. Reconsider if a screen is
later found to cross over at the wrong width.

The width breakpoint is not the only responsive threshold — it is only the one screens currently
test themselves. The arrangement also owns a height floor (`MIN_WIDE_AREA_HEIGHT`, today 6), a
minimum pane width (`MIN_PANE_WIDTH`, today 40), the below-image metadata side-width floor (today
15), and the hero-suppression rule when height cannot fit a hero and a usable list — each keeping
today's value as it moves into shared code. These are arrangement-owned thresholds, not per-screen
decisions, and screens own none of them.

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
| element presence | every screen shows a hero image (source and shape above). A pill row is present on Home (sections), music (groups), books (surname buckets), feeds (groups), movies and TV (letter ranges, large libraries), and podcasts (episode filter, only while active). Home videos are the exception: a count label row and no pills |

Nothing else was found to vary. Declarations may not cover geometry, the breakpoint, or focus
behaviour.

**Domain content is data, not a declaration.** The arrangement renders whatever hero content, list
rows, and pills a screen hands it. What the list shows (tracks vs chapters) and how its pills group
(artist vs author-surname bucket) are DATA the screen supplies, not presentation differences, so
they are not declaration fields and SHALL NOT be claimed as such. A screen's substitution table
documents its domain data; its declaration covers only the presentation fields above.

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
`PanelFocus` (which panel is focused) plus, for hero-on-left screens only, a `left_focused: bool`
naming which pane is focused (`false` = right pane). Hero-on-top screens have one focusable region
and pass no pane bit. Grouped
Music already derives exactly this (`left_focused = library_focused && track_active`,
`right_focused = library_focused && !track_active`); the arrangement consumes the two inputs rather
than a single `focused: bool`, which cannot express both the hero's panel-level brightness and the
track list's pane-level brightness at once.

The queue and card join this even though their layout does not, because their current pair
(`QUEUE_LIST_BG`/`LIBRARY_SIDE_BG`) already disagrees with every content panel's
(`BG_GREEN`/`PLAYBACK_PANEL_BG`), and leaving them out would preserve the exact complaint: having to
direct a change panel by panel.

**The card and queue list adopt the content-panel focus scale.** Their focused surface becomes
`SURFACE_FOCUSED` (`#3c4841`, today's `BG_GREEN`) and their unfocused surface becomes
`SURFACE_RESTING` (`#333c43`, today's `PLAYBACK_PANEL_BG`). This is a deliberate, small visible
change to the left panel — focused `#48584e` → `#3c4841`, unfocused `#2d353b` → `#333c43` — and is
part of "mature screens win", not a no-op. The left column's full backdrop already uses
`QUEUE_COLUMN_FOCUSED_BG`/`PLAYBACK_PANEL_BG` (the content scale), so only the queue list and card
surfaces move. `LIBRARY_SIDE_BG` keeps its remaining job — the right panel's backdrop — as
`SURFACE_BACKDROP`.

### 9. One set of mouse hit targets

`LayoutMain` currently has four representations of "rows you can click" (`left_item_rows`,
`home.hitmap`, `wide_music_track_hitmap`, `audiobookshelf_episode_rows`) and two of "the right pane"
(`wide_music_right_area`, `audiobookshelf_book_right_area`). `LayoutMain::is_wide_music_active()`
exists only because those fields are per-screen. The arrangement produces one common form.

Without this, drawing would be unified but clicking would not, and each new screen would still add a
field plus a branch in `input_mouse.rs`.

### 10. Components are the primitive; "Panel" keeps its meaning

The reusable units are **components** — `Hero`, `List`, `PillBar`, `Scrollbar`, `SelectionMarker`,
`HeroShell`, plus the chrome (`Card`, `TabBar`, `PlaybackStrip`, `StatusBar`, `Visualizer`) — because
`CONTEXT.md` binds **Panel** to Library-or-Queue and that meaning must not drift. Each lives in its
own file, and an arrangement is a composition of them (see "Component catalogue").

Most of the chrome already has its file (`card.rs`, `chrome_tabs.rs`, `chrome_status.rs`,
`visualizer.rs`); what remains is finishing `render_main`'s decomposition so it only composes and no
longer paints inline (the backgrounds at `render/mod.rs:330,344` and the reach-ins at `:378`,
`:526`). This is therefore a small, early phase rather than a deferred one: components are what
phases 3-5 extract and compose, not an afterthought.

### 11. Verification is by throwaway comparison, not committed tests

Project rules forbid new committed UI snapshot assertions. To evidence "the mature screens did not
change", each phase captures rendered output for the affected screens at one narrow and one wide
width before the change, diffs after, and deletes the captures. Mechanical evidence, no churn cost.

Existing structural tests that are not brittle (for example the column-count boundary test in
`library_column_width.rs`) stay.

## Risks

Each risk below names its mitigation, which is a task; nothing here is left for the implementer to
judge later.

**Home has no render tests at all.** `home_tests.rs` tests only a scroll helper that lives in
another file, and `tests_home_latest.rs` (18 tests) never renders. Home is also the screen changing
most. → Home's adoption phase (phase 6) is verified by throwaway capture plus a direct visual check;
the capture diff is the acceptance criterion, and no committed test is added.

**Role renames are a one-way door.** Renaming a role after call sites migrate is a large mechanical
diff. → Implementers use exactly the role names in "Role vocabulary"; no alias or synonym is
introduced during migration.

**`PLAYBACK_PANEL_BG` does two unrelated jobs** — the actual now-playing strip, and "resting content
panel" on essentially every content screen (25 references across 13 render files). → Split into
`SURFACE_PLAYBACK` (now-playing strip) and `SURFACE_RESTING` (resting content panel), both `#333c43`.
Each of the 25 references is read and assigned to the role of what it draws; the values are identical
so there is no visual change, but the assignment is by reading, not by convention.

**Feeds and home videos gain a wide arrangement they have never had.** This is new behaviour, not a
refactor. → Phase 7, last among the arrangement phases, after the arrangements are proven by screens
that already had a wide form.

**Audiobooks is specified as hero-on-left but is not applied as one.** The delta makes the spec
enforceable, but the current visual gap is unmeasured. → Task 0.3 diffs books against music at the
same size before starting, so the intended end state is known before phase 6 begins.

**Big-bang risk across eight screens plus the palette.** → Phased commits, each independently
reviewable and revertable, in the order below.

**Two completed changes are unarchived** (`extend-home-latest-abs-feeds`,
`redesign-audiobookshelf-book-browsing`, both at 100%). Their deltas are not yet merged into
`openspec/specs/`, so this change's deltas may be written against a stale base. → Task 0.1 archives
both before implementation starts.

## Migration Plan

Phases are independently reviewable and land in order. Each ends with the throwaway comparison of
decision 11.

1. **Colour roles and the focus lever.** Additive; raw constants stay. The only visible change is
   the card and queue list adopting the content-panel focus scale (decision 8); every other surface
   is byte-identical. The role names are the "Role vocabulary" table; no name is decided during this
   phase.
2. **One breakpoint.** Ownership inverts, value unchanged; the compile-time assert is removed;
   `render/mod.rs` stops testing it. No visible change.
3. **Extract the components.** `HeroShell` (merge the two ▁/▔ painters), `Scrollbar` (role, not
   colour arg), `SelectionMarker` (make real — the unified edge marker; a deliberate visible change
   to every list, so it is an explicit exception to "mature screens change least"), then `Hero` and
   `List`, extracted from movies/TV (hero-on-top's source) and grouped Music (hero-on-left's source).
4. **Assemble hero-on-top** from the components; movies/TV move onto it; podcasts adopt and delete
   their duplicate hero. Mature screens, so any visible change here is a defect.
5. **Assemble hero-on-left** from the components; grouped Music moves onto it, including the
   two-pane focus model and the hero-on-left pill row.
6. **Adoption:** Home, then audiobooks. Visible change expected and intended; Home bends to the
   shared definition.
7. **New wide arrangements:** feeds, home videos.
8. **Unified mouse hit targets.** Components already return targets, so this phase removes the four
   per-screen row representations and the two pane rects from `LayoutMain` and collapses the
   `input_mouse*` branches.
9. **Chrome component files.** Finish `render_main`'s decomposition so it only composes; most chrome
   already has its file.

**Rollback:** each phase is a separate commit. Phases 2, 4-5 and 8-9 are no-visible-change and
revert independently. Phases 1 (card/queue focus scale), 3 (selection marker) and 6-7 change
appearance deliberately and would be reverted as a unit with their spec deltas.
