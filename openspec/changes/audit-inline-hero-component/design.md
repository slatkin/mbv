## Context

The inline hero is the component that replaces a selected list row with a variable-height detail block in the single-column browser. #593 (merged) split the render tree into screens/arrangements/components/theme and made palette primitives private — an access-control boundary. It did not touch what geometry is shared. The handoff on #563 identified the inline hero as the first component to audit. The audit (#596) classified five surfaces as drift. This change implements the drift fixes.

The shared shell already works: `hero::inline_detail_flow` (scroll math), `hero::selected_detail_shell` (borders), and `hero::HeroContent` + `paint_hero_content` (Model A) are used by 4–6 surfaces. `home_hero::beside_image_hero_dims` + `render_beside_image_hero` (Model B) is used by 3 surfaces. `render_pill_bar` is used by every tab. The drift is in the content painted inside that shell.

The norm: Model A (right-aligned, wrap-around) for tall images; Model B (right-half, meta-column) for wide 16:9; no-image uses Model A's degenerate form. Pills in the panel, one row. No variants, no exceptions. Structured lists (seasons, tracks, episodes) are hidden in the inline hero and accessed via a modal on Enter.

## Goals / Non-Goals

**Goals:**
- Eliminate all five inline-hero drift surfaces so every tab renders the same content shape.
- Add a constituent-list modal reusing the existing modal-frame vocabulary.
- Route every inline hero through `paint_hero_content` (Model A) or `render_beside_image_hero` (Model B) — no bespoke content paths remain.
- Ship as one PR per drift fix, each independently mergeable.

**Non-Goals:**
- The hero-on-left (wide) presentation — separate audit, separate change. Wide mode continues showing track listings beside the static hero.
- Redesigning any surface's visual presentation beyond removing drift.
- The arithmetic collapse work from the #563 handoff (proceeds in parallel, independent of this change).
- Hit-target ownership migration (deferred per #593's design gate decision).

## Decisions

### 1. Model selection by image aspect ratio, not by surface

The inline hero selects Model A or Model B based on the image's aspect ratio, not the surface's identity. Tall images (~2:3, posters, book covers, podcast covers) use Model A (`paint_hero_content`). Wide 16:9 thumbnails (Home Keep Watching, Home ABS episodes) use Model B (`render_beside_image_hero`). No-image surfaces use Model A with `image: None` (the Feeds pattern).

**Why not one model for everything:** the text-wrapping algorithms are fundamentally different. Model A narrows text row-by-row as each row passes a narrow, tall image. Model B uses a fixed meta-column width beside a wide image that occupies half the area. Forcing one algorithm onto both image shapes produces bad layouts — text either wraps too narrowly beside a wide thumbnail or too widely beside a tall poster.

**Why not a third model for no-image:** Model A already handles it correctly (Feeds proves this). Adding a third path for no-image would be a new surface-specific split, the exact pattern this change eliminates.

### 2. Podcast author/description as HeroLines, not a third spacer mode

The podcast hero currently hand-paints author and description below the Model A upper block because its spacer pattern (spacer before description only if description present, then unconditional trailing spacer) doesn't match `HeroContent`'s two built-in modes. Rather than adding a third spacer mode to `HeroContent`, the author and description become `HeroLine::Plain` entries in `HeroContent::lines`. The spacer behavior is handled by the caller's line construction: an empty `HeroLine::Plain("")` produces a blank row (Model A already skips rendering empty lines but still advances the row counter — confirmed in `hero.rs:427-440`).

**Alternative considered:** adding a `Spacer` variant to `HeroLine` or a third `unconditional_spacer` mode. Rejected because it grows the API for one surface's needs, and the existing `HeroLine::Plain("")` + row-advance already produces the correct spacing.

**Caveat:** if the empty-line-as-spacer pattern proves fragile across edge cases (zero-height area, image overlap), a `HeroLine::Spacer` variant is the upgrade path. The initial implementation tries the simpler approach first.

### 3. Selection modal reuses modal-frame, not a new overlay system

The constituent-list modal uses `modal_frame.rs` (the same frame, backdrop, and centered placement used by confirm, multiselect, and context-menu). The modal's content is a scrollable list of items with names and metadata, navigable by existing movement keys. Enter selects; Esc/Backspace cancels.

**Why not grow the inline hero:** the user's decision was explicit — the inline hero never grows to accommodate a list. The modal is a separate interaction surface. This keeps the hero component's API fixed (title + meta + overview + image) and moves list interaction entirely out of the hero.

**Why not an inline expansion (the original model):** the original inline-hero grew the replacement block to fit a list. The user called that "messy and awkward." The modal system is more developed now and provides a cleaner interaction.

### 4. Podcast played/unplayed filter moves to the selection modal

The current podcast hero bakes All/Played/Unplayed filter pills inside the hero content. With pills removed from the hero, the filter needs a new home. The selection modal (which lists episodes) is the natural place: the modal can show filter pills at its top, then the filtered episode list below. This keeps the filter contextual to episode browsing without polluting the hero or the panel.

**Alternative considered:** moving the filter to the panel alongside alphabetical pills. Rejected because the panel would then have two pill rows (alphabetical + episode filter), which is the same two-row problem the audit identified as drift. The filter is per-show, not per-tab, so it belongs with the episode list.

### 5. One PR per drift fix, in dependency order

The five drift fixes are sequenced by dependency:

1. **Selection modal** (new component) — must exist before any surface can route Enter to it.
2. **Series** — simplest structural removal (delete extension, route Enter to modal).
3. **Music** — remove `album_detail` workspace, route Enter to modal.
4. **Podcasts** — remove hand-painted block + in-hero pills, add alphabetical panel pills, route Enter to modal with filter.
5. **ABS Books** — model switch (B→A), simplest geometry change.
6. **Home Feed** — align image placement, last because it's the least clear which model fits.

Each PR includes a characterization buffer test (if coverage is missing) then the migration, with the test updated to reflect the intended buffer change.

## Risks / Trade-offs

- [Risk] The selection modal changes the keyboard interaction model for series, music, and podcasts — users who expect inline episode/track lists will see a modal instead. → This is the intended product change; the modal is explicitly the chosen interaction. The characterization test for each surface documents the before/after.
- [Risk] Removing the inline episode/track list means users can no longer see episodes while browsing in narrow mode without pressing Enter. → Accepted. The hero-on-left (wide) presentation always shows the track listing — this is the reward for screen space. Narrow mode shows the hero summary; Enter reveals the list.
- [Risk] The podcast played/unplayed filter in the modal may feel buried compared to the current always-visible in-hero pills. → The filter is contextual to a selected show; the modal is where show-specific interactions live. The alphabetical panel pills handle tab-level browsing.
- [Risk] ABS Books switching from Model B to Model A changes the book hero's visual layout (image moves from right-half to right-aligned, text wrapping changes). → This is an explicit buffer change judged correct: the tall cover fits Model A, not Model B. The characterization test documents the change.
- [Risk] `HeroLine::Plain("")` as a spacer may not produce the exact same spacing as the hand-painted podcast block. → The characterization test for the podcast surface will catch any spacing discrepancy. If it fails, a `HeroLine::Spacer` variant is the documented upgrade path.
- [Risk] The Home Feed drift fix is the least clear — it may need a product decision on whether the feed item should show an image at all in the inline hero. → Resolved: Feeds have never shown images; Home Feed uses Model A no-image (text-only), matching the dedicated Feeds tab.

## Open Questions

None. The Home Feed image question was resolved: Feeds have never shown images; Home Feed uses Model A no-image (text-only), matching the dedicated Feeds tab.
