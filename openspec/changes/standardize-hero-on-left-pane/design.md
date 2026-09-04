## Context

See `proposal.md` — Why. The state that shapes the approach:

- `arrangements/hero_left.rs::shared_hero_presentation` (`:44-53`) returns `(left_pane,
  right_pane)` and owns the one-row status reserve. It owns geometry only; nothing paints.
- The right rail already has its shared painter, `hero_on_left_list_panel_border` (`:238-262`),
  called by all seven destinations. The new left-pane primitive is its mirror image and belongs
  next to it.
- `PANE_PAD_X = 2` / `PANE_PAD_Y = 1` already exist in `hero_left.rs:31-32` with a doc comment
  claiming they are the one shared inset. Five destinations pass something else.
- `hero_on_left_recessed_box` (`:271-295`) is already called by TV (`tv_wide.rs:369,392`), Music
  (`music_wide_tracks.rs:28`) and the shared hero painter (`hero.rs:604`, reached by Movies and
  Home). The three destinations missing it are ABS Books, ABS Podcasts and Feeds — so D7 is an
  extension of an existing primitive, not a new one.
- `HeroData` (`home_hero.rs:89-98`) has two variants; `render_home_hero_content`
  (`home_hero.rs:483-521`) matches on them and routes `Generic` to
  `home_latest_row::render_home_latest_detail_content`. The Home clamp (`home.rs:237-242`) lives
  in the `Generic` arm's caller and cannot exist without the split.
- `tests_conformance_matrix.rs` already has per-surface render harnesses
  (`render_browse_component`, `render_book_component`, `render_podcast_component`,
  `render_music_component`, `render_home_shell_with`, `feed_component`) for all seven surfaces.
  Adding background sampling is cheap; the file is 747 lines and will cross the 800-line cap.
- Existing placeholder vocabulary: `show_placeholder` flags across `detail.rs`, `tv_wide.rs`,
  `audiobookshelf_book.rs`, and `album_art.rs::render_inline_art_cell` (`:191-195`) which paints
  `BORDER_UNFOCUSED`. That is a *loading* placeholder, semantically distinct from D5's
  *no-artwork-exists* placeholder.

## Goals / Non-Goals

**Goals**

- One owner for left-pane fill, extent, inset, appearance condition, and focus resolution.
- Delete the drift, do not normalise it: destinations lose their local paint code entirely.
- Remove the structural affordance (provider branch) that let the Home clamp exist.
- Leave a mechanical check behind for each defect class so this cannot silently return.

**Non-Goals**

- No change to `shared_hero_presentation`'s rect computation, the breakpoint, the minimum-height
  guard, or the status-row reserve. The primitive consumes what that function returns.
- No change to the right rail, its border primitive, or the canonical media-list controls.
- No mouse or hit-geometry work. `restore-mouse-support` (#638) owns `HitRegions<Target>`; this
  change must not add hit maps and must not move painted geometry without noting it there.
- No content stretch-to-fill (D2 top-anchors), no new metadata fields, no Queue changes.

## Decisions

### D-A. The primitive takes the content area, not a pane rect

```
pub(in crate::app) fn hero_on_left_pane(
    f: &mut Frame,
    content_area: Rect,
    focus: LeftPaneFocus,
) -> Option<Rect>
```

It calls `shared_hero_presentation(content_area)?` itself, takes the `left_panel` from that,
fills it with the surface resolved from `focus`, and returns
`padded_rect(left_panel, PANE_PAD_X, PANE_PAD_Y)`.

*Why it takes `content_area` rather than the caller's `left_panel`:* `browser/paint.rs:57`
currently does `left_panel.height = left_content_area.height;` — a caller re-deriving an
arrangement-owned extent. It is a no-op today (both values are `area.height - 1`) and it is
precisely the class the new requirement forbids. Accepting a pane rect leaves that door open in
all seven destinations; accepting `content_area` closes it structurally, because a caller has
nothing to hand in but the rect the arrangement already consumes. `shared_hero_presentation` is
pure and cheap, so recomputing it inside the primitive costs nothing.

The `Option` mirrors `shared_hero_presentation`. Every call site already sits behind a
`wide_library_panes(...)?` or `shared_hero_presentation(...).is_some()` gate, so this is at worst
one extra `?`.

*Why one call rather than a fill function plus a separate inset function:* every observed defect
is a destination that did one of the two and not the other, or did them against different rects.
A single call site makes the pairing unskippable and makes "did this destination conform?" a
grep for one symbol.

*Rejected:* extending `shared_hero_presentation` to paint. It is a pure geometry function called
from geometry-publishing paths (`music_wide.rs::publish_geometry`, and `browser/mod.rs`'s
`is_some()` gate) that must stay callable without a `Frame`.

### D-B. Focus is a closed enum, not a `bool`

```
pub(in crate::app) enum LeftPaneFocus {
    ReadOnly,
    Workspace(bool),   // the workspace's current focus state
}
```

The primitive resolves `ReadOnly => SURFACE_RESTING` and
`Workspace(held) => resolve_surface_focus(held)`.

*Why not a `bool`:* the per-screen mistake this change exists to prevent is exactly the
read-only-versus-workspace confusion. ABS Podcasts is the surface *gaining* focus-green under D8,
and its paint site (`audiobookshelf_podcast.rs:209-260`) has only `focused: bool` and
`interaction: PodcastInteraction` in scope — the correct value is
`focused && interaction.episode_selection.is_some()`, derivable but not sitting in a variable.
With a `bool` parameter, passing the in-scope `focused` compiles, looks right, and silently
fails D8 by turning the pane green while the show list has focus. With the enum, `ReadOnly` and
`Workspace(..)` are two visibly different call shapes and the reviewer can check the variant, not
the expression.

This reverses the earlier draft's decision to keep a `bool`. The earlier reasoning ("a new type
for seven call sites the conformance matrix already pins") was wrong on both counts: the matrix
as originally drafted asserted the *same* expectation the caller supplies (see D-I), and seven
call sites across five files is exactly where a two-state distinction stops being obvious.

Per-surface variants are fixed by this change and named in the tasks:

| Destination | Variant |
|---|---|
| Movies / home videos / Emby podcasts / feed-group browser | `ReadOnly` |
| Home | `ReadOnly` |
| Feeds | `ReadOnly` |
| TV | `Workspace(focused && episode_cursor.is_some())` |
| Music | `Workspace(focused && track_cursor.is_some())` |
| ABS Books | `Workspace(focused && chapter_selection.is_some())` |
| ABS Podcasts | `Workspace(focused && episode_selection.is_some())` |

### D-C. `hero_on_left_recessed_box` is renamed and loses its padding parameters (D9)

It becomes the main content box primitive with the committed name from `CONTEXT.md`. Renaming an
existing four-caller primitive is a smaller and more honest diff than adding a differently-named
wrapper and leaving the old name to be found by the next reader.

Per locked decision **D9**, the `pad_x` / `pad_y` parameters are **removed**; the primitive owns
one internal padding value, `(2, 1)`, matching the pane inset from D6. All four existing call
sites drop their arguments:

- `tv_wide.rs:369` and `:392` pass `PANE_PAD_X, PANE_PAD_Y` today — no visual change.
- `music_wide_tracks.rs:28` passes `PANE_PAD_X, PANE_PAD_Y` today — no visual change.
- `hero.rs:604` passes `overview_pad, 1`, where `overview_pad` is `WIDE_OVERVIEW_PAD` — this one
  shifts. Movies' and Home's overview text moves horizontally by the difference. Accepted under
  D9; it must be reviewed as an intended delta, not chased as a regression.

The three destinations missing the box (ABS Books, ABS Podcasts, Feeds) gain it in the
per-surface phase, carrying their existing payload into it.

### D-D. The `Hero` trait is a UI-layer trait over content, not over providers

```
trait Hero {
    fn title(&self) -> &str;
    fn subtitle(&self) -> Option<&str>;      // series/author/show
    fn meta_rows(&self, width: u16) -> Vec<...>;
    fn body(&self) -> HeroBody;              // Listing(..) | Description(&str)
    fn artwork(&self) -> HeroArtwork;        // Image(..) | Placeholder
}
```

`artwork()` answers *what this item has*, with exactly two states: real artwork, or none
(`Placeholder`). It is deliberately **not** where the images-off mode lives — see D-E.

Implemented for `EmbyItem`, for the Audiobookshelf/generic `QueueItem` hero, and — per locked
decision **D11** — for feed entries, with no Feeds exception: a feed entry renders the same
image region (real thumbnail when it has one, placeholder when it does not) and the same main
content box carrying its entry text. The renderer
takes `&dyn Hero` (or a generic) plus a `Rect` and never inspects the concrete type.
`HeroData::Generic` disappears; `HeroData` collapses to the layout-carrying struct the Emby arm
already is.

*Why a trait rather than a single struct built by both providers:* the metadata layout needs
width-dependent text measurement (`keep_watching_hero_layout`, `home_latest_detail_text`) that
today runs at different points for each provider. A trait lets each provider keep its own
measurement while the renderer keeps one path. If the two measurement paths converge during
implementation, collapse to a struct — that is strictly simpler and preferred if it works.

*What this fixes structurally:* `home.rs:237-242` reads `if let Some(HeroData::Generic(_, area))`.
With no `Generic` variant, the clamp has nowhere to be written. Deleting the clamp without
removing the variant leaves the affordance in place.

### D-E. Placeholder artwork, and where images-off lives (D10)

Two different questions, deliberately answered in two different places:

| Question | Where it is answered | Result |
|---|---|---|
| Does *this item* have artwork? | `Hero::artwork()` → `Image` \| `Placeholder` | placeholder fills the region |
| Are images rendered *at all*? | the hero **layout**, one level up | the image region does not exist |

Per locked decision **D10**, images-off is not a third `HeroArtwork` variant. It is a layout
input: the hero layout takes the global images setting and returns `Option<Rect>` for the
artwork region — `None` when images are off, in which case text and metadata occupy the full
content width. `artwork()` is then never consulted.

*Why not a third variant:* a `HeroArtwork::Hidden` would make every renderer branch on a global
setting at the point where it is asking about one item, which is how per-surface divergence gets
reintroduced. Keeping the collapse at the layout level means it is decided once, and the seven
surfaces inherit it because they all take their artwork rect from that layout — which is exactly
what D10's "the collapse must be uniform across all seven surfaces" requires.

The placeholder itself is a `SURFACE_ARTWORK_PLACEHOLDER` theme role plus a
`render_artwork_placeholder(f, area)` in `render/components/`.

*Distinct from the existing loading placeholder* (`album_art.rs:191-195` `BORDER_UNFOCUSED`
block): that means "artwork is coming", this means "this item has none". They may end up looking
identical; they are still different call sites with different meanings, and collapsing them would
make "images never finished loading" indistinguishable from "this feed entry has no cover".

### D-F. Movies' effective inset is `(4, 1)`, and D6 moves it horizontally — not vertically

`browser/paint.rs` insets **twice**: `:35` calls `wide_library_panes(body_area, PANE_PAD_X,
PANE_PAD_Y)`, which itself does `left_area = padded_rect(left_panel, 2, 1)`
(`arrangements/library.rs:18`); then `:82` insets that result again with
`padded_rect(left_area, PANE_PAD_X, 0)`. Effective inset is therefore **`(4, 1)`**, not `(2, 0)`
as the seeding analysis recorded.

D6's `(2, 1)` therefore produces on Movies:

- a **two-column horizontal shift** — hero content moves two columns toward the pane edge;
- **no row shift** — the vertical inset is already `1`.

Task 3.1's verification is stated in those terms. The earlier draft's "exactly the intended
one-row hero shift" was unmeetable and would have sent an implementer looking for a bug.

The fix is to stop double-insetting: take the content rect from `hero_on_left_pane`'s return
value and delete the `:82` `padded_rect` entirely. The `wide_library_panes` call at `:35` stays,
because its `pad_y` also produces `right_area` (`library.rs:19-23`).

### D-G. Sequence: primitive → per-surface conformance → Hero trait (with placeholder) → gates

Each phase leaves the tree in a state where the next phase's defects are *visible*: adding the
primitive first means each destination's conversion is a deletion, and the conformance test
added after the conversions can be written against one expected value rather than seven.

Placeholder artwork is folded **into** the `Hero` trait phase, not landed before it. Routing
seven per-screen no-artwork conditionals first and then tearing them down with `artwork()` would
rebuild the exact provider-divergence class this change exists to delete, in the interim tree.
The placeholder is one variant of `HeroArtwork`; it arrives when the trait does.

The `Hero` trait remains **after** the per-surface pane work. The clamp deletion (D2) and the
pane fill are independently observable fixes; landing them first means a trait-refactor
regression cannot be confused with an unfixed pane defect.

### D-I. The conformance test asserts the rule, not the caller's answer

A matrix that reads each surface's focus expectation from the same expression the caller passes
proves only that the fill happened — it re-derives the bug it is meant to catch. So the
assertions are stated per surface kind, from the D-B table, and both focus states are exercised:

- the four workspace surfaces (TV, Music, ABS Books, ABS Podcasts) are rendered **twice** — with
  the left workspace holding focus and with the right rail holding focus — asserting
  `SURFACE_FOCUSED` and `SURFACE_RESTING` respectively;
- the three read-only surfaces (Movies/home-videos/Emby-podcasts/feed-group browser, Home,
  Feeds) are rendered in **every** focus state the surface can reach, asserting
  `SURFACE_RESTING` in all of them.

That is what makes ABS Podcasts' D8 gain and Feeds' read-only rule testable rather than
tautological.

### D-H. File splits

Two files are at risk of the 800-line cap:

- `tests_conformance_matrix.rs` (747) — will cross it. Split by concern in the same change
  (`tests_conformance_matrix.rs` keeping the shared harnesses, plus a
  `tests_conformance_hero_pane.rs` for the new left-pane assertions).
- `home_hero.rs` (684) — the `Hero` trait work adds to it while removing the `Generic` arm; net
  direction is unclear. Measure after the trait lands; split only if it crosses.

`audiobookshelf_podcast.rs` (689), `hero.rs` (691) and `music_wide.rs` (680) all *lose* lines in
this change (hand-painted fills and strips are deleted), so no split is planned for them.

## Risks / Trade-offs

- **The Hero trait is the largest and least-bounded item, and it now carries the placeholder and
  images-off work too.** → It is phased after the visible pane fixes and is independently
  revertible: phases 1–3 deliver every pane fix without it. If its scope grows past the estimate
  during implementation, stop and split it into its own change rather than growing this one.
- **D6's uniform inset is a visible change on Movies** (two-column horizontal shift, no row
  shift — see D-F) and on Music/Books/Podcasts (insets change in both axes); **D9 additionally
  shifts Movies' and Home's overview text** inside the main content box. → Buffer
  characterization tests for each affected surface land *before* the inset changes, and the diffs
  are reviewed as intended visual deltas, not as passes.
- **Feeds is the most visible change in the proposal** (D11): feed entries gain an image region
  — placeholder or real thumbnail — and a main content box, neither of which they have today. →
  Called out explicitly for live review in tasks 3.5 and 6.4; not to be treated as an incidental
  side effect of the trait.
- **Deleting the Home clamp changes non-Emby Home hero appearance in a way the clamp was
  deliberately added to avoid** (`home.rs:214-220` comment: a stranded cover far below short
  text). → The cover's placement comes from `render_home_latest_detail` anchoring at the *bottom*
  of the rect it is given. With a full-height pane, that anchor must be changed to top-anchor with
  the text; if it is not, D2 produces exactly the stranded cover the clamp was hiding. This is a
  required part of task 2.4, not an afterthought.
- **The `Block::style(<Color>)` ast-grep rule may have existing hits outside this change's
  scope.** → Run it before writing the fix tasks; any pre-existing hits are either fixed here (if
  in hero-on-left paths) or filed separately. The rule may not land with a dirty tree —
  `ui-design-system` forbids an accepted baseline.
- **Renaming `hero_on_left_recessed_box` and dropping its padding parameters touches four call
  sites and any test referring to it.** → Mechanical; the compiler finds all of them. Only the
  `hero.rs:604` site changes output (D-C).
- **`hero_on_left_pane` recomputes `shared_hero_presentation` that its callers already
  computed.** → Pure function on a `Rect`; the cost is nil and the structural guarantee (a caller
  cannot hand in a mutated pane) is the entire point.
- **`restore-mouse-support` (#638) is landing on the same branch and touches the same
  destinations.** → This change moves painted geometry (the D6 inset). Any hit geometry #638
  derives from the left pane must be re-derived after this lands. Sequence this change and #638
  deliberately; do not run them concurrently on the same files.

## Migration Plan

No data, protocol, or persistence migration. Purely visual and structural, landing as one branch
with per-phase commits. Rollback is per-phase revert; phases 1–3 are independent of phase 4.

## Open Questions

None. The three questions raised in the first draft — the main content box's internal padding,
images-off behaviour, and Feeds' hero shape — are resolved as locked decisions D9, D10 and D11
respectively, and are encoded in D-C, D-E and D-D above.
