# Fix Podcast Media-List Ownership Design

## Context

See `proposal.md` for motivation. `AudiobookshelfPodcastComponent` already keeps a persistent `InlineMediaBrowser<String>` for Normal/Narrow show browsing and a persistent `WideMediaList<String>` for the episode workspace, but the Wide show rail is a fresh `WideMediaList<String>` built in `render_audiobookshelf_podcast_content` on every frame. Live show position instead survives in `AudiobookshelfBrowseState.selected_id`, a component `scroll` field, and an App-side copy. `AudiobookshelfPodcastGeometry` republishes `show_rows`, `columns`, list rects, and selected-row offsets; `view()` writes paint results back, clicks search the row map, and wheel input checks a parent rect and emits movement without claiming the event.

The existing event-scoped shell push is sound. `AudiobookshelfPodcastShowMove` already receives a resolved index and performs shell-owned detail fetch and library-position persistence; `AudiobookshelfPodcastEpisodeTransition` is an intentional no-op because its state mutates locally. Queue is the concrete ownership precedent (umbrella D4), while Feeds supplies the active-control retained-hit, wheel-claim, shell-tick, pill, and presentation-test precedent. The delta in `specs/audiobookshelf-podcast-library-ui/spec.md` resolves the inline-detail and alphabetical-bucket conflicts authorized by umbrella D8.

## Goals / Non-Goals

**Goals:**

- Give persistent Wide and Inline show controls exclusive live cursor, scroll, viewport, and current-frame row-geometry authority.
- Keep provider content and shell effects separate from presentation position, with movement resolved once by the component.
- Converge podcast row and hero presentation on the existing shared Emby/TV vocabulary without forking helpers.
- Prove the boundary through focused and real shell-tick tests at Wide and Normal/Narrow.

**Non-Goals:**

- Change episode filter or selection authority, episode workspace behavior beyond row projection, the episode modal, playback/enqueue intents, download/progress state, images/covers, or persistence mechanisms.
- Change alphabetical bucket data generation, `BrowserComponent`, other destinations, or shared media-list, pill-bar, hero, detail-shell, formatting, anchor, access, or test-harness helpers.
- Add another router, mounted identity, subscription, or focus boundary.
- Change the established browser-left/hero-right Wide arrangement; stale naming is touched only if a directly edited hunk makes a trivial correction safe.

## Decisions

### D1. Persistent show controls own live position

Add `wide_show_list: WideMediaList<String>` beside the existing persistent `narrow_list`. The active show control owns selection, scroll, viewport, and row geometry. Show-row content refresh uses stable-target-preserving control APIs and clamps locally when a target disappears; it does not adopt the content struct's cursor during ordinary projection. Delete the component `scroll` field and the per-frame Wide control construction.

`AudiobookshelfBrowseState.selected_id`, `cursor()`, and `select()` remain only on the effect path: after the component resolves a movement target/index, the shell may select that index to fetch detail and persist the library position. The App copy is not presentation authority.

Alternative: retain `selected_id` as a synchronized cursor shared by both controls. Rejected because it preserves the parent/content mirror and turns every refresh into an implicit render seed.

### D2. Movement and pointer interaction target only the active control

Keyboard movement calls the active show control's selection operation and emits `AudiobookshelfPodcastShowMove` with the resulting resolved index. Click resolution uses the active control's `claims_current_point`/`resolve_current_point`; headings and blank space remain unclaimed. Wheel input is eligible only when the active control claims the current point, moves that control directly, and returns `MouseClaimed`, following Feeds and Queue. No losing message is discarded after mutation.

Alternative: keep `show_rows`, `columns`, and `list_area` arithmetic in the parent. Rejected because those are duplicate interaction and geometry authorities that can drift from the latest painted control.

### D3. Responsive handoff is control-to-control through one anchor

On a Wide/Normal transition, capture `viewport_anchor()` from the outgoing control using that control's selected-row offset for its viewport, then apply it to the incoming persistent control. Delete `painted_row_offset`, `selected_row_offset` writeback, `state.select(idx)` reseeding, and the throwaway-control anchor endpoint. Ordinary content refresh remains separate from responsive re-anchor.

Alternative: reconstruct the destination cursor and scroll from `AudiobookshelfBrowseState` plus a painted offset. Rejected because that recreates a destination-specific viewport authority outside the controls.

### D4. The bespoke show geometry bundle is removed, while legitimate workspace geometry remains

Remove `show_rows`, `columns`, parent show `scroll`, and selected/painted row offsets. Narrow `AudiobookshelfPodcastGeometry` and the shell's `app.layout.main.*` copies to only rects or hit regions still read by shell-owned chrome, image, overlay, or provider-workspace paths; show hits never use those copies. Retain episode-row and selector geometry where the podcast-owned episode workspace still needs it.

Alternative: leave compatibility fields populated but unused. Rejected because continued publication preserves a tempting second source of truth and prevents the row from meeting the campaign contract.

### D5. Shell projection and effect ownership remain event-scoped

Keep `push_audiobookshelf_podcast_content()` event-scoped. `select_audiobookshelf_show` and `save_audiobookshelf_position` remain shell-owned and consume component-resolved values; `activate_audiobookshelf_position` remains the explicit restore/re-anchor path. Preserve the exhaustive documented no-op for `AudiobookshelfPodcastEpisodeTransition`.

Alternative: move detail fetch or persistence into the component. Rejected because components cannot own Services or persistence, and the existing typed boundary already resolves the target correctly.

### D6. Episode rows use canonical semantic projection

Project episode `duration_seconds` through the existing canonical short-duration formatter (`M:SS` or `H:MM:SS`) and map progress/current playback to the shared played/active row semantic states. Show rows remain duration-free Collections. Render the same episode rows and `All`/`Played`/`Unplayed` pills in Normal/Narrow selected-show detail as in the TV reference, as required by the delta; do not change filter state or episode intent behavior.

Alternative: preserve the sparse podcast skeleton or invent podcast-specific badges. Rejected by umbrella D8 and because the shared row painter already expresses the required duration and state slots.

### D7. Hero and pill chrome reuse accepted reference policy

Pass the existing show-title presentation into Wide hero so the title occupies the TV reference text area. Route bucket and episode-filter pills through the existing shared pill-bar contract, including its accepted 18-character, character-safe label truncation and overflow behavior. Bucket data remains the provider-owned set of non-empty surname ranges; no `All` or `#` buckets are synthesized.

Alternative: add podcast-specific pill rendering or alter bucket construction. Rejected because presentation convergence belongs in shared chrome use, while bucket data is explicitly outside this repair.

### D8. Verification uses discriminating shell-tick and buffer evidence

Register `tests_tick_integration_podcast.rs` and exercise both Wide and Normal/Narrow through `Application::tick()`, `draw_frame`, and the shell sync pass. Assertions distinguish consumed from unconsumed input using `raw_messages`, verify the selected target is the row painted as selected, cover both on-control and off-control wheel legs, and prove blank/header no-ops. Paint counters prove exactly one active list painter and no base-frame underpaint. Resize round-trips preserve target plus row offset. Focused media-list/component tests cover retained-geometry invalidation and anchor behavior; render buffers cover duration, played/active slots, Wide title, inline episodes/pills, and truncation.

Alternative: rely on direct `Component::on` and geometry unit tests. Rejected because they do not verify TuiRealm delivery, shell synchronization, or one-painter composition.

## Risks / Trade-offs

- [Removing the content cursor from presentation exposes stale shell assumptions] → Keep it as explicit effect-path state and cover movement through the shell tick, asserting the painted control target rather than an App recomputation.
- [Deleting geometry fields breaks legitimate image or overlay readers] → Inventory every shell reader and retain only the smallest provider-workspace/chrome rect subset, while tests prove show hits resolve solely through control geometry.
- [Inline episode content exceeds available replacement height] → Reuse `InlineMediaBrowser` fit admission and the existing selected-detail/TV row-budget policy; preserve ordinary-row fallback when detail cannot fit.
- [Played and active projection disagree] → Characterize current progress/playhead inputs and assert the canonical precedence in render buffers without changing progress ownership.
- [Pill text changes hide provider bucket behavior] → Test both shared truncation/overflow chrome and the unchanged non-empty surname-range data set.

## Migration Plan

1. Characterize focused list behavior, Wide and Normal/Narrow shell-tick composition, and the spec-delta presentation before transferring authority.
2. Add the persistent Wide show control; move movement, retained hits, wheel claims, and responsive handoff to the active controls; remove the parent/content presentation mirror and bespoke show geometry.
3. Narrow shell/render geometry publication and converge episode, hero-title, and pill projection through existing shared helpers.
4. Run breakpoint-focused tests and the full bounded gate list. If the repair must be reverted, revert the source and delta together; no persisted-data migration is involved.
5. Close umbrella row 7.1 only after the bounded PR merges and receives reviewer sign-off, per umbrella D5.
