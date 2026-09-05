## Why

The hero-on-left arrangement centralised only its *geometry*. `shared_hero_presentation`
returns bare `Rect`s, so the left pane's **fill**, **content inset**, **focus resolution**, and
**appearance condition** were hand-copied into seven destinations and drifted in every one of
them. Today the same nominal arrangement paints the left pane five different ways, with four
different appearance conditions and five different content insets. Three destinations are
visibly broken: Audiobookshelf Podcasts never fills the pane, Audiobookshelf Books passes a
`Color` where a `Style` is expected (setting foreground, not background), and Home clamps an
arrangement-owned pane's height to measured content. Feeds skips the fill entirely when nothing
is selected. All three defects show the right column's `SURFACE_BACKDROP` through the hole and
read as "the panel is missing".

None of this violates a written requirement, because canon pins the hero-on-left *rect* and
says nothing about the *paint*. The right rail already has a shared painter
(`hero_on_left_list_panel_border`) used by all seven destinations; the left half has seven
copies. That asymmetry is the entire defect class. The boundary gates missed it because all
three defects live in `render/components/`, their legitimate owner, and the conformance matrix
never samples a left-pane background cell.

The deeper cause of the Home defect is the provider split: `HeroData::Generic` is a separate UI
code path from `HeroData::Emby`, so a size-to-content hack could be added to one provider's
branch without touching the other. A single UI-level hero abstraction removes the branch the
hack lived in.

## What Changes

- **New shared arrangement primitive** `hero_on_left_pane(f, content_area, focus) -> Option<Rect>`:
  derives the left pane from `shared_hero_presentation` itself so no caller can hand it a
  re-derived extent, fills it, and returns the one shared content-inset rect. Fill is
  unconditional — independent of hero item, selection, or content height. `focus` is a closed
  two-state value (`ReadOnly` / `Workspace(bool)`), so a surface declares which *kind* of pane it
  has and the primitive resolves the treatment.
- **All seven hero-on-left destinations route through it**: Movies/home-videos/podcast-channels
  browser, TV, Music, Home, Feeds, Audiobookshelf Books, Audiobookshelf Podcasts. Their
  hand-painted fills are deleted, not adjusted.
- **BREAKING (visual)** Single left-pane content inset `(PANE_PAD_X, PANE_PAD_Y) = (2, 1)`
  everywhere. Movies insets twice today (effective `(4, 1)`), so its hero content moves two
  columns outward with no row shift; Music, Books and Podcasts move to match in both axes.
- **Home's height clamp is deleted.** Non-Emby hero content is top-anchored in a full-height
  pane. The content-height calculation feeding the hero content rect is retained.
- **Uniform focus rule.** A left pane whose workspace can hold focus (TV, Music, ABS Books, ABS
  Podcasts) renders `SURFACE_FOCUSED` when its workspace holds focus. Read-only heroes (Home,
  Movies, Feeds) never do. Podcasts gains the treatment; Feeds' current always-resting behaviour
  is made explicit rather than incidental.
- **The `SURFACE_BACKDROP` inset is reframed as the "main content box"** — a shared primitive
  for a Hero description. Structured workspaces additionally receive a separate recessed
  media-list box, whose parent-owned `WideMediaList` supplies the interactive rows. TV therefore
  presents title, metadata, a blank row, overview box, then season-pills/media-list box; Music
  and Audiobookshelf follow the same ownership model as they migrate. The primitive retains one
  internal padding value.
- **One UI-level `Hero` trait** implemented for Emby items, Audiobookshelf/generic entries, and
  feed entries. It carries title, ordered metadata, optional description, and semantic artwork;
  arrangements own slots while destination components own `WideMediaList` state and interaction.
  `HeroData::Generic` special-casing is removed at the UI layer; the hero renderer no longer
  branches on provider.
- **Placeholder artwork.** While images are on, every hero renders an image region; an item with
  no artwork gets a single shared placeholder owned centrally (theme role + component), not per
  provider. **Feeds gains an image region it has never had** — the most visible change here.
- **Images-off collapses the region entirely**, uniformly on all seven surfaces: no reserved and
  no placeheld artwork area, text and metadata take the full width. This is decided once at the
  hero layout, not per surface.
- **Redundant one-row `SURFACE_BACKDROP` strips** below the left pane in Movies, Music and Books
  are deleted — they are no-ops that read as re-deriving the status-row reserve the shared
  primitive already owns.
- **Drift-proofing:** the conformance matrix samples left-pane background cells for all seven
  surfaces (including the empty-selection and non-Emby-selection states), and an `ast-grep` rule
  rejects `Block::default().style(<Color>)`, the silent-foreground trap that caused the Books
  defect.
- **Canon:** `CONTEXT.md` commits the term "Hero pane" (and "main content box"), and the
  `mbv-frontend` skill lists the new primitive and the `Hero` trait in its reuse list.

## Capabilities

### New Capabilities

_None._ The behaviour belongs to two existing capabilities.

### Modified Capabilities

- `right-panel-arrangements`: adds a requirement that the hero-on-left left pane is a shared,
  unconditionally filled container with one owner, one extent, one inset and one focus rule;
  adds requirements for the Hero overview box and separately recessed parent-owned media-list
  box on structured workspaces, with one padding value; extends the
  two-focusable-panes requirement so the focus rule is stated once for every focusable left
  workspace rather than named per surface; corrects the wide-arrangement requirement, which
  currently lists Emby podcast libraries as having an interactive left detail workspace when they
  in fact browse read-only through the generic browser path.
- `ui-design-system`: adds a requirement that hero content is provider-neutral at the UI layer
  (one `Hero` abstraction, no provider branch in the renderer, placeholder artwork rather than
  an empty image region, and one uniform images-off collapse); extends the mechanical-bypass
  requirement to cover the `Block::style(<Color>)` silent-foreground trap.

## Impact

**Code**

- `src/app/render/arrangements/hero_left.rs` — new `hero_on_left_pane` primitive; rename
  `hero_on_left_recessed_box` to the main-content-box primitive.
- `src/app/components/browser/paint.rs`, `src/app/render/components/tv_wide.rs`,
  `music_wide.rs`, `home.rs`, `feeds.rs`, `audiobookshelf_book.rs`,
  `audiobookshelf_podcast.rs` — route through the primitive; delete local fills, insets and
  strips.
- `src/app/render/components/home_hero.rs`, `home_latest_row.rs`, `hero.rs` — the `Hero` trait
  and removal of the `HeroData::Generic` UI branch.
- `src/app/render/theme/` — placeholder artwork role.
- `src/app/render/tests_conformance_matrix.rs` — left-pane background sampling. This file is
  747 lines today and will need splitting in the same change.
- `rules/frontend-boundary/` — new `Block::style(<Color>)` rule plus fixtures.

**Docs / canon**

- `CONTEXT.md` (Presentation section), `.agents/skills/mbv-frontend/SKILL.md` and its
  `.opencode/` mirror, `docs/architecture/interactive-surface-ledger.md` (no row-state change
  expected; the surfaces are already `migrated`).

**Not in scope**

- Queue's chrome column (cited as deliberately outside hero-on-left in ADR 0021 and
  `right-panel-arrangements`).
- Inline/narrow presentation, breakpoints, and the minimum-height guard.
- Content stretching to fill the pane (D2 top-anchors; stretch is explicitly out).
- Mouse hit geometry, which `restore-mouse-support` (#638) owns.
