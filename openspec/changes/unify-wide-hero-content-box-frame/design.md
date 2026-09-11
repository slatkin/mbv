## Context

See `proposal.md` — Why. The facts that shape the approach, all verified at `a7d6526c`:

- `wide_hero_hero_pane` (`src/app/render/arrangements/wide_hero.rs:296-317`) already derives the
  pane's content inset (`padded_rect(hero_panel, PANE_PAD_X, PANE_PAD_Y)` at `:316`) and returns it
  as a bare `Rect`. Destinations then compute the box from *that* rect, or from the pane rect, or
  from a rect they expand themselves.
- The box primitive (`wide_hero.rs:449-481`) insets inwards by `PANE_PAD_X` from whatever rect it
  receives (`:461-464` panel, `:474-480` payload), so it cannot tell a pane rect from a content
  rect. That ambiguity is the defect; the frame is decided at the call site.
- Frames today: TV expands back out by hand (`components/tv_wide.rs:459-465`, repeated
  `:495-503`); Music measures from the pane (`arrangements/music.rs:47-48`, `music_wide.rs:534`);
  Home/Movies/homevideos (`components/hero.rs:596-604`), Feeds (`feeds.rs:206`), ABS Podcasts
  (`audiobookshelf_podcast.rs:425`) and ABS Books (`audiobookshelf_book.rs:176`) hand the content
  rect; ABS Books additionally insets its own hero text by `SELECTED_BLOCK_SIDE_PADDING == 2`
  (`audiobookshelf_book.rs:422-425`).
- `Hero`/`HeroContent` is already the declared content contract (`hero_model.rs:12-27`), and
  `publish_geometry` (`components/music_wide.rs:107-140`) is the existing precedent for an
  arrangement publishing geometry to a destination — there is no need for a second publisher.

## Goals / Non-Goals

**Goals:**

- Make the box's horizontal frame arrangement-owned and unconstructible from a destination.
- One content frame across all seven Wide hero destinations, with no per-destination
  compensation of any kind.
- A guard that fails for a destination which regresses to its own frame.

**Non-Goals:**

- Converging the three wide-hero text painters (`paint_hero_content`, `paint_wide_hero_text`,
  `render_home_hero_meta_block`) onto the `Hero` contract. Their shapes differ for real reasons
  (wrapped multi-line title, subtitle row, watch-state suffix, N reserved meta rows, per-line
  "past the image" width flag); only the geometry arithmetic is being removed here.
- Feeds' second (watched-filter) pill row and its self-derived row budget
  (`feeds.rs:73-136`) — a separate chrome-row authority with its own change.
- Moving `home_hero_emby.rs`'s `impl App` presentation measurement.

## Decisions

**D1 — The pane primitive returns the frame, not the destination (chosen; advisor-recommended).**
`wide_hero_hero_pane` returns a `Copy` newtype `WideHeroHeroPane { pane, content }`, and
`wide_hero_hero_content_box` / `_with_surface` take that newtype plus the caller's `y`/`height`
rather than a bare `Rect`. The box's left edge and width come from `content.x` / `content.width`
once; the payload keeps the D9 `(PANE_PAD_X, PANE_PAD_Y)` inset. A destination has no bare rect to
pass, so a mis-framed box cannot be written — the fix is enforced by the compiler, not by
convention.

*Alternatives rejected:* (a) keep the bare `Rect` and add a convention/check — the record refutes
it, since the requirement already said this verbatim (`spec.md:262-267`) and D9
(`standardize-hero-on-left-pane/design.md:129-137`) already fixed the primitive's padding, and the
drift survived both plus a month of use; (b) keep per-destination frames and declare them — the
same spec forbids per-screen geometry declarations (`spec.md:300-301`).

**D2 — `y`/`height` stay caller-supplied.** Vertical placement is genuinely destination-specific
(TV sizes the overview to its wrapped text and then places its episode box below it; Home places
its box under artwork plus metadata). Only the horizontal frame is a shared invariant, so only the
horizontal frame moves into the arrangement.

**D3 — TV's compensation is deleted, not preserved.** Once the frame is arrangement-owned, TV's
`content_area.x.saturating_sub(PANE_PAD_X)` expansions are exactly the workaround the new type
makes unnecessary; leaving them would double-apply the frame.

**D4 — ABS Books' wide `SELECTED_BLOCK_SIDE_PADDING` is dropped in this change.** That inset is a
legacy of the inline presentation and, once the box frame is the content edge, keeping it would
leave the book hero text two columns right of its own box — i.e. it would trade one drift for
another. This is a visible delta (book hero text moves two columns left) and is called out for
live review.

**D5 — TV's focused episode box: absorbed by `unify-surface-colour-neutral`, do not re-implement.**
That change's table already introduces one identity for exactly these sites — "A pane's content
box: TV's episode listing (`tv_wide.rs:515`), Music's track listing (`wide_hero.rs:468`, chosen at
`music_wide.rs:528-532`) and the generic pane inset (`wide_hero.rs:467`) are the same identity"
(`src/app/render/theme/surface_table.rs:161-172`, `Surface::MainContentBox`). Its migration
therefore already removes TV's manual `SURFACE_ACCENT_SOFT` repaint as a separate way of saying
it. This change's task 2.5 is a post-landing *check* that it did, not a re-implementation; if the
surface change left the repaint in place, folding it into the primitive's surface parameter
remains a free rider on the retyping.

*Consequence for this change's scope:* only the horizontal frame is ours. Surface naming, the
resolver, and the guardrails belong to that change and must not be re-expressed here.

**D6 — The guard is a cross-destination buffer invariant.** A syntactic check can match presence
but not an omission: every current call site passes a *valid* rect, so a scan cannot distinguish
frames. The invariant ("at Wide geometry, the leftmost main-content-box column equals the pane's
content edge") lives in `tests_conformance_matrix.rs`, which already harnesses the destinations,
and is stated relationally so it cannot break on a deliberate layout change.

## Risks / Trade-offs

- **The intended target frame is inferred, not stated.** `wide_hero.rs:438-448`'s doc can be read
  either way → Evidence for `content.x` (box edge = pane + `PANE_PAD_X`) is TV's hand-expansion,
  Music's pane-frame arithmetic, the title alignment both produce, and the existing "one content
  inset" scenario (`spec.md:174-178`). Task 1.1 settles it with a buffer comparison before any
  refactor.
- **Four surfaces visibly move** (Feeds hero box, ABS Podcasts episode box, ABS Books hero text +
  chapters box, plus the reported Home/Movies/homevideos box) → Every moved surface is listed in
  the proposal with a buffer-capture task so the deltas are reviewed as intended, not chased as
  regressions — the same discipline `standardize-hero-on-left-pane/design.md:275-281` used.
- **Characterisation tests pin drifted baselines** (`tests_wide_hero_pane_characterization.rs`
  states this explicitly) → Their expected output must be re-read against the new invariant rather
  than accepted or blindly updated; a test that only pinned the drift is deleted, not retuned.
- **`overview_pad` conflates a layout mode with a content parameter** (`hero.rs:499`, `:585-604`)
  → Out of scope beyond ensuring the box path no longer uses it to choose a frame; noted here so
  the next reader does not mistake it for the authority.

## Migration Plan

**Sequencing — this change MUST land after `unify-surface-colour-neutral`.** That change's
acceptance bar is that nothing rendered changes: main's buffer tests pass byte-identical and no
test file is edited. This change deliberately moves pixels on four surfaces and re-baselines the
characterisation tests that pinned the drift, so landing it first would invalidate exactly the
proof that change depends on. The two also work on the same files: that change's Unit A migrates
`arrangements/wide_hero.rs` (this change's core primitive) and Unit C migrates `components/hero.rs`
and `components/music_wide.rs`, both of which this change retypes. Its Unit C also collapses the
hero-pane focus match, which is adjacent to the `LeftPaneFocus` path this change keeps intact.

Before starting, re-verify every `file:line` citation in this change against the tree as it stands
then (task 0.1); the surface work will have moved them. No requirement-level conflict exists —
that change modifies `ui-design-language`, this one `right-panel-arrangements`. Two of this
change's decisions were written before that change's table existed and are now settled *by* it:
D5 is absorbed (its `Surface::MainContentBox` row already folds TV's manual repaint, the named
variant and the generic inset into one identity), and the hero-pane match's `LeftPaneFocus`
behaviour is described there as collapsing to the one bool at the call site
(`unify-surface-colour-neutral/design.md` D3(d)) — so the landed parameter list of
`wide_hero_hero_pane` must be read, not assumed, before retyping its return type. Nothing in that
change touches geometry, which is the whole of this change's subject.

Single internal-API change, no compatibility shim: retype the primitive and update all call sites
in one commit (the compiler enumerates them). Rollback is `git revert` of one commit; nothing is
persisted, so there is no data migration.
