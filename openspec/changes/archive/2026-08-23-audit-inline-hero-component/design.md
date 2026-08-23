## Context

The inline hero is the component that replaces a selected list row with a variable-height detail block in the single-column browser. #593 (merged) split the render tree into screens/arrangements/components/theme and made palette primitives private — an access-control boundary. It did not touch what geometry is shared. The handoff on #563 identified the inline hero as the first component to audit. The audit (#596) classified four surfaces as presentation drift. Home Feed was already text-only/no-image, but its metadata still used a bespoke Home painter; this change also routes that path through the shared Model A component.

The shared shell already works: `hero::inline_detail_flow` (scroll math), `hero::selected_detail_shell` (borders), and `hero::HeroContent` + `paint_hero_content` (Model A) are used by 4–6 surfaces. `home_hero::beside_image_hero_dims` + `render_beside_image_hero` (Model B) is used by 3 surfaces. `render_pill_bar` is used by every tab. The drift is in the content painted inside that shell.

The norm: Model A (right-aligned, wrap-around) for tall images; Model B (right-half, meta-column) for wide 16:9; no-image uses Model A's degenerate form. Pills in the panel, one row. No variants, no exceptions. Structured lists (seasons, tracks, episodes) are hidden in the inline hero and accessed via a modal on Enter.

## Goals / Non-Goals

**Goals:**
- Eliminate all four inline-hero drift surfaces so every tab renders the same content shape.
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

The four presentation drift fixes are sequenced by dependency, followed by Home Feed shape verification and shared-component routing:

1. **Selection modal** (new component) — must exist before any surface can route Enter to it.
2. **Series** — simplest structural removal (delete extension, route Enter to modal).
3. **Music** — remove `album_detail` workspace, route Enter to modal.
4. **Podcasts** — remove hand-painted block + in-hero pills, add alphabetical panel pills, route Enter to modal with filter.
5. **ABS Books** — model switch (B→A), simplest geometry change.
6. **Home Feed** — retain the already-conforming text-only/no-image shape, route literal Feed items through shared `paint_hero_content` Model A, and preserve the Audiobookshelf-only artwork branch.

Each surface includes a characterization buffer test (if coverage is missing) before its migration or conformance verification. A surface may have a conforming visual shape while still requiring a production routing migration to reach the shared component.

### 6. Narrow/wide input gating reuses the existing `is_wide_*_active()` pattern

Series' `series_selection`, Music's `album_track_focus`, and the ABS podcast/book focus fields each drive both the narrow inline hero (migrated by this change) and the wide hero-on-left arrangement (non-goal, must stay untouched) through one shared key handler per surface. There is no existing narrow/wide branch in that handler.

`LayoutMain` already exposes `is_wide_tv_active()` and `is_wide_music_active()` — booleans derived from whether the wide renderer populated its own area field this frame — used today by render code, not input code, but the same technique applies: input handlers read `self.layout.main.is_wide_X_active()` (the last completed frame's layout) to decide whether Enter opens the new selection modal (narrow) or keeps the existing in-hero focus mode (wide). ABS podcasts and books have no equivalent field yet; add `is_wide_podcast_active()` / `is_wide_book_active()` following the same `<area>.width > 0 && <area>.height > 0` shape.

**Why not a new mechanism:** the codebase already reads `self.layout.main.*` from input handlers for width-dependent decisions (e.g. `input_feed_tab_keys.rs`'s column-count check). Reusing the pattern keeps the narrow/wide distinction in one place instead of duplicating breakpoint math in the input layer.

### 7. Series selection modal uses its defined season pills

The inline hero's removal (decision/task 2.2) is total — no season pills, no episode table, nothing structured remains inline. The selection modal projects the Series screen's already-defined season pills through the shared pill presentation and lists the selected season's episodes below them. Selecting an uncached season initiates its fetch and shows the shared loading state. This supersedes the attempted flat season-header list, which was not an approved presentation variant.

### 8. Home Feed shape conformance still requires shared routing

Literal Home Feed entries are `QueueItem::Feed` values. Their existing visual
shape was already the generic no-image form: title and metadata were painted as
text, with no artwork reservation or image-cache lookup. That shape matches the
dedicated Feeds tab, but the metadata was painted by Home's bespoke
`render_home_latest_detail` path rather than shared `paint_hero_content`.
The required migration is therefore structural routing, not image placement:
non-Audiobookshelf Home items now use shared Model A. The Audiobookshelf episode
path remains separate: its declared wide-thumbnail artwork behavior, including
the image branch in `render_home_latest_detail`, is preserved. Group 6's
characterization established the visual truth; Group 7a completes the shared
component routing.

### 9. Failed conformance audits invalidate surface-level completion

Passing component tests and finding calls to shared painters is not completion.
The post-implementation audits found that Music, Movies/TV, and ABS Books still
owned parallel replacement, scrolling, target, pill-placement, or modal-data
behavior. Several tests preloaded caches or explicitly asserted obsolete inline
content, so they could pass while user-visible behavior remained divergent.

Completion now requires an end-to-end contract matrix at narrow, bottom-selected,
cannot-fit, Mini, and wide presentations. A shared painter call is necessary but
not sufficient: admission, replacement, scrolling, fallback, targets, markers,
controls, async refresh, and input parity must also use the shared contract.

### 10. Every screen supplies pills; presentation has one owner

Every screen already has a defined pill model. Screens supply labels, stable IDs,
and selection only. The shared arrangement owns exactly one painted pill row and
one spacer row; the spacer inherits the parent panel background. The shared
`render_pill_bar` component owns all pill styling and one-row hitboxes. A surface
must invoke this presentation once and may not own pill geometry or paint a
second copy. This applies to panel pills and to defined pills projected into a
selection modal; no new pill semantics are invented.

### 11. Grouping remains surface content; inline replacement is shared behavior

Music may own artist headers and album ordering, Books may own author buckets,
and letter-grouped libraries may own letter headers. None may own a separate
selected-row replacement algorithm. One shared replacement plan owns fit
admission, source-row swallowing, continuation rows, `inline_detail_flow`,
persisted scroll, ordinary-row fallback, one parent target, and marker
suppression. A grouping header may remain visible only when doing so does not
push any part of the selected hero outside the viewport.

### 12. Selection modals retain source identity and derive live list state

The modal is not a copied row cache. It retains a typed source identity and a
typed `Loading`, `Empty`, or `Ready` list state. Provider completion events
refresh the matching open modal and preserve the cursor by stable item
identity. All surfaces use one bounded frame, viewport, row format, pill
presentation, movement model, cancellation model, and activation dispatch.
Surface adapters provide domain data and existing pill models, never geometry or
styles. Narrow Books render no chapters inline; wide workspaces retain their
declared persistent child lists.

Series season completion has a producer-boundary ordering invariant: a season
fetch may start and emit `SeriesSeasonEpisodesFetched` only after
`SeriesDetailFetched` has populated the complete ordered Series detail cache.
The completion handler preserves valid cache-present refreshes; a cache-missing
completion is stale/impossible and is ignored rather than synthesizing season
metadata.

Series tracks overall detail loading separately from `(series_id, season_id)`
episode requests, so fan-out suppresses duplicate work. No timer or backoff
policy is introduced.

### 13. Remediation groups are the canonical completion owners

After the Group 12 checkpoint, Groups 8-12 are reviewed and complete. Their
work fulfills the older presentation tasks for Music, Books, shared scrolling,
and pills. Groups 13-15 exclusively own the remaining modal state, adapters,
and interaction behavior; Group 16 exclusively owns the final structural and
tooling audits. Earlier incomplete checkboxes that describe the same behavior
are legacy mappings, not separate implementation work or additional acceptance
gates.

## Risks / Trade-offs

- [Risk] The selection modal changes the keyboard interaction model for series, music, and podcasts — users who expect inline episode/track lists will see a modal instead. → This is the intended product change; the modal is explicitly the chosen interaction. The characterization test for each surface documents the before/after.
- [Risk] Removing the inline episode/track list means users can no longer see episodes while browsing in narrow mode without pressing Enter. → Accepted. The hero-on-left (wide) presentation always shows the track listing — this is the reward for screen space. Narrow mode shows the hero summary; Enter reveals the list.
- [Risk] The podcast played/unplayed filter in the modal may feel buried compared to the current always-visible in-hero pills. → The filter is contextual to a selected show; the modal is where show-specific interactions live. The alphabetical panel pills handle tab-level browsing.
- [Risk] ABS Books switching from Model B to Model A changes the book hero's visual layout (image moves from right-half to right-aligned, text wrapping changes). → This is an explicit buffer change judged correct: the tall cover fits Model A, not Model B. The characterization test documents the change.
- [Risk] `HeroLine::Plain("")` as a spacer may not produce the exact same spacing as the hand-painted podcast block. → The characterization test for the podcast surface will catch any spacing discrepancy. If it fails, a `HeroLine::Spacer` variant is the documented upgrade path.
- [Risk] Home Feed could be mistaken for the Audiobookshelf latest-item path because both appear in Home's generic sections. → The characterization uses a literal `QueueItem::Feed`; its text-only shape and shared Model A routing are verified separately, while Audiobookshelf episode artwork remains unchanged.

## Open Questions

None. Home Feed's existing no-image shape, required shared Model A routing, and the separate Audiobookshelf episode artwork behavior are explicit.
