# Fix TV Series Media-List Ownership Design

## Context

See `proposal.md` for motivation. The accepted Browser-family repair established the Normal/Narrow side of TV: `BrowserComponent` owns that presentation and its seeding/anchor behavior is not reopened here. Wide TV already holds persistent `WideMediaList<String>` controls for series and episodes, and the base frame already reserves rather than underpaints the mounted surface. The remaining Wide violations are narrower: `TvWorkspaceComponent` retains a parent `cursor` mirror; series clicks write only that mirror; series and episode hits are reconstructed with `resolve_ordinal_at_y`, including `y.max(...)` and `unwrap_or(self.cursor)` fallbacks that claim non-row space; shell click arms write the ordinal into `BrowseLevel`; and wheel eligibility checks a `left_area` that the painter deliberately leaves empty.

The existing `ViewportAnchor<String>` handoff path connects Wide TV to Normal TV, but commit `880c9b17` removed its row-offset assertions, so only target preservation is currently proven. Queue remains the concrete ownership precedent (umbrella D4), and TV remains an Emby reference-design surface (umbrella D8). Completion follows umbrella D5: merged PR plus reviewer sign-off, recorded in both campaign records.

## Goals / Non-Goals

**Goals:**

- Give the persistent Wide series control exclusive series cursor/scroll authority.
- Resolve series and episode pointer interactions from retained current-frame control geometry and stable targets.
- Leave blank, heading, spacer, above-list, and otherwise unpainted row space unclaimed.
- Keep responsive transfer as one bidirectional `ViewportAnchor` preserving selected target and row offset.
- Prove the mounted Wide and Normal/Narrow TV paths through real `Application::tick()` integration tests.

**Non-Goals:**

- Change seasons, season pills, episode workspace content, the series detail box, `SeriesDetailFetched`, images, effects, persistence, or `TvActivate` / `TvEpisodeMove` / `TvEpisodeActivate` / `TvSeasonMove` / `TvCycleLetterPill` semantics.
- Change Normal/Narrow `BrowserComponent` internals, base-frame reservation, `render_wide_media_list`, shared retained APIs, breakpoint plumbing, pill chrome, routing, mounting, focus, identity, or subscriptions.
- Introduce another geometry map, router, painter, or destination-specific fork of the canonical controls.

## Decisions

### D1. The Wide series control is the only live series-position authority

Delete `TvWorkspaceComponent`'s parent `cursor` field and all movement, refresh, and paint write-backs to it. Keyboard movement continues to update the persistent series control directly. A series pointer hit selects the control through `select_target` or `select_index` before emitting its semantic request, so the row painted as selected and the row reported to the shell cannot diverge.

Alternative: keep the parent cursor synchronized after every control move. Rejected because the copy remains a second authority and the current click path proves synchronization can be missed.

### D2. Hit identity is stable and geometry comes only from the retained painted frame

Change the source-of-truth `TvHit::SeriesRow` and `TvHit::EpisodeRow` payloads from ordinals to stable `String` targets before adapting callers. Series and episode resolution gate on each control's `claims_current_point` and resolve through `resolve_current_point`; use `current_detail_rect` only where the painted detail pane requires it. Remove ordinal reconstruction, the `y.max` clamp, and selection fallback. Blank, heading, spacer, above-list, and stale/unpainted regions return no hit. If repository search confirms TV is the final caller of `resolve_ordinal_at_y`, delete it; otherwise leave the shared helper unchanged and record the remaining caller.

Alternative: preserve ordinal payloads and translate them after the event. Rejected because list mutation can make position unstable and because re-derived parent geometry can disagree with what the control painted.

### D3. Shell mouse arms consume component-resolved targets without restoring App cursor state

Click, double-click, and context-menu dispatch receive stable targets from `TvHit` and resolve the corresponding TV item/effect without calling `BrowseLevel::set_resting_cursor`. Focus and existing semantic side effects remain. The component mutation happens before the message crosses the boundary, respecting TuiRealm's delivery model; the shell does not recompute a losing or fallback row.

Alternative: write the stable target back as a resting cursor before performing the effect. Rejected because a live click-driven shell copy is still a mirror and violates the resolved-value ownership boundary.

### D4. Wheel eligibility follows retained series-control claims

Replace the dead `left_area` gate with `list.claims_current_point(at)`. Wheel input remains local: it moves the series control and returns the existing claim result, with no new typed wheel request or shell cursor update.

Alternative: repair `left_area` publication to cover the rail. Rejected because it would revive parent-published compatibility geometry beside the control's retained current frame.

### D5. Responsive handoff keeps the accepted Browser boundary and re-proves the full anchor

Keep `hand_off_tv_breakpoint` and the one-shot `ViewportAnchor<String>` plumbing. Tests must demonstrate Wide→Normal and Normal→Wide each preserve the selected stable target and selected-row offset after the incoming control paints. Normal/Narrow TV remains the accepted `BrowserComponent`; this change may adapt only the TV-side handoff use needed to honor the existing protocol, not alter Browser seeding or anchor policy.

Alternative: accept target-only transfer. Rejected because the canonical contract explicitly transfers target plus row offset and the weakened assertions leave viewport continuity unverified.

### D6. TV workspace authority and one-painter composition remain intact

The parent retains legitimate TV workspace state and translation: seasons, episode content, pills, detail, images/effects/persistence, and typed activation/navigation semantics. The Wide and Normal/Narrow destination remains the sole mounted event boundary and exactly one canonical list painter owns each active surface; no second mounted identity, subscription, focus target, fallback painter, or base-frame underpaint is added. The bounded row closes only under umbrella D5 after merge and reviewer sign-off.

Alternative: migrate adjacent TV workspace state or render plumbing while editing the component. Rejected as scope expansion and contrary to the established Emby reference design.

## Risks / Trade-offs

- [Stable-target lookup fails after content changes between paint and shell handling] → treat the target as unresolved/no-op rather than falling back to the current cursor, and cover stale/invalid target behavior.
- [Episode-pane chrome is accidentally made clickable while replacing ordinal resolution] → assert retained row claims separately from detail/chrome claims and require blank/header no-op coverage.
- [Removing the cursor field exposes hidden shell assumptions] → characterize keyboard, click, double-click, and context-menu paths through component and shell-tick tests before deletion.
- [Anchor application occurs only after the incoming paint] → drive the complete draw/sync/tick sequence in both directions and assert post-paint target plus offset.
- [Shared ordinal helper has another consumer] → search before deletion; preserve it unchanged when TV is not the final caller.

## Migration Plan

1. Characterize retained geometry, click selection, wheel behavior, one-painter composition, and bidirectional handoff at Wide and Normal/Narrow.
2. Change `TvHit` to stable targets first, then transfer pointer and wheel authority to the retained controls and remove the parent/shell mirrors.
3. Delete `resolve_ordinal_at_y` only if the caller search proves it unused.
4. Run focused TV tests and the full bounded verification gates.
5. If needed, revert the bounded source change; no persisted-data migration is involved.
