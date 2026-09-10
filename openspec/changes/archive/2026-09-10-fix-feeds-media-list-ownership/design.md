# Fix Feeds Media-List Ownership Design

## Context

See `proposal.md` for motivation. `FeedsComponent` already holds persistent `WideMediaList<String>` and `InlineMediaBrowser<String>` fields, preserves stable FeedEntry targets on ordinary refresh, and transfers one `ViewportAnchor` when its presentation changes. The shell projection is already one-way content with typed, resolved FeedEntry intents, and the base frame reserves the Feeds area without underpainting it.

The remaining ownership violations are: `move_selection`, `select_target`, and reset drive both controls; wheel eligibility and row clicks use compatibility `claims_point` / `resolve_point` with caller-supplied rectangles; render rebuilds `left_row_map` / `left_item_rows`; and the parent copies the painter-resolved offset. Painting still enters through compatibility free functions rather than configuring and invoking the active control's `Component::view`. The shared retained-result seam and the Queue/Home precedents already provide the required replacement.

Feeds also retains bounded presentation deviations from the Emby reference required by umbrella D8: hand-painted watched-filter text, no canonical resume-progress projection, a misleading comment contradicting the intentional hero placeholder slot, and a Feeds-local pill truncation value that must be checked against the Emby policy. Current specifications already permit the converged result, so `.openspec.yaml` sets `skip_specs: true`.

## Goals / Non-Goals

**Goals:**

- Give the active persistent Feeds control exclusive live movement, selection, reset, scroll, and retained hit-geometry authority.
- Paint through the active control's retained-result seam and remove Feeds' parent row maps and painted-offset copy.
- Preserve one explicit target-plus-offset handoff across Wide and Normal/Narrow transitions.
- Converge the approved Feeds chrome and row presentation on shared Emby primitives without forking shared helpers.
- Prove the complete path through focused tests, rendered buffers, and real `Application::tick()` plus shell-sync integration at both presentations.

**Non-Goals:**

- Persist group or watched-filter selection across restarts; add an `App` slot, request variant, or component-to-shell mirror.
- Move group, heading, subscription-pill, or watched-filter ownership into a canonical control; change structural-row vocabulary; or change `feeds_manage`.
- Change the Emby homevideos feed view, right-click behavior, persistence formats, non-hero two-column policy, routing, mounting, focus, or subscriptions.
- Modify or fork shared media-list controls/painters, Wide hero arrangements, hero/detail-shell primitives, pill-bar helpers, theme roles, or mouse gesture state.

## Decisions

### D1. Only the active control moves; the existing anchor becomes the sole responsive transfer

Movement, target selection, reset, clicks, and wheel input operate only on the control selected by the current Wide predicate. The inactive control remains parked. On a presentation transition, `view()` captures one `ViewportAnchor { selected_target, selected_row_offset }` from the outgoing control and applies it to the incoming control before switching active presentation. Ordinary content refresh continues to preserve or clamp the stable target locally and never adopts shell position.

Alternative: keep lockstep as a warm standby and retain the anchor defensively. Rejected because two controls then remain live authorities and the handoff is not the mechanism that preserves target plus offset.

### D2. The active control paints through `Component::view` and retains the only list geometry

The Feeds render path establishes the active control's claim and row-flow geometry with `set_geometry`, applies the closed semantic paint policy with `set_paint_policy`, and calls that control's `Component::view` exactly once. It then reads only `current_selected_row_rect` and, for Inline, `current_detail_rect` to place parent-owned hero content. The compatibility free-function calls, offset return/copy, `rebuild_selectable_maps`, and Feeds-side `left_row_map` / `left_item_rows` clearing disappear. The base frame continues to reserve the area and paints no list rows.

Alternative: keep compatibility painters and merely stop publishing maps. Rejected because caller-supplied layout and painter outputs would remain a second geometry seam rather than the current control view being authoritative.

### D3. Pointer and wheel resolution use retained current-frame claims

Wheel handling first asks the active control `claims_current_point`; only a retained claim moves that control and consumes the gesture. Click resolution uses `resolve_current_point`, so blank space, headings, and spacers remain unclaimed. For Inline selected-row replacement, a point within `current_detail_rect` resolves to `current_selected_target`, following the Home hero-cover rule; explicit canonical row resolution otherwise wins. No shell or parent re-derives a target from coordinates or a row map.

Alternative: retain explicit-rectangle `claims_point` / `resolve_point` for Feeds. Rejected because recomputed geometry can diverge from what the active control painted in the latest frame.

### D4. Watched-filter chrome reuses the shared pill-bar primitive while remaining parent-owned

The All / Watched / Unwatched selector is projected into the shared `render_pill_bar` presentation instead of hand-built text spans and separators. Subscription/group pills and watched-filter state remain Feeds-owned chrome outside canonical rows. The implementation reuses the existing helper and semantic roles; it does not introduce a Feeds-specific pill painter.

Alternative: preserve the hand-rolled selector because ownership is already correct. Rejected because umbrella D8 requires full visual convergence on the Emby reference, not only correct state ownership.

### D5. Resume state is projected through canonical row slots

An unplayed FeedEntry with resumable progress projects the bounded percentage through the canonical Active/trailing presentation already defined for media rows; played and ordinary entries keep their existing semantic states. Duration remains in the canonical duration slot. Playback lifecycle and FeedEntry authority stay outside the control.

Alternative: add a Feeds-only progress suffix or hero-only progress. Rejected because the canonical model already expresses resume progress and a destination-specific rendering would fork the Emby reference.

### D6. The placeholder hero image slot is intentional, and truncation follows the Emby policy

Feeds keeps the existing artwork placeholder box when images are enabled. The contradictory `feeds_model.rs` comment is corrected to describe that intentional chrome rather than claiming the hero has no image. The Feeds `MAX_LABEL = 12` value is compared with the Emby pill truncation policy and aligned only if it diverges; no new truncation helper or policy is introduced.

Alternative: remove the image slot because FeedEntries carry no artwork. Rejected by the approved product decision to keep the placeholder as intentional visual chrome.

### D7. Feeds workspace and shell boundaries remain unchanged

Groups, watched-filter selection, structural-row projection, pills, hero content, image enablement, effects, and typed FeedEntry intent translation remain in the Feeds parent and shell. Group and filter selection remain component-local for the mounted lifetime and are not persisted across restarts. No new router, mounted identity, subscription, focus target, or request variant is added.

Alternative: add restart persistence while touching local selection. Rejected because it introduces new shell authority and persisted behavior outside this bounded ownership repair.

## Risks / Trade-offs

- [Removing lockstep exposes a broken breakpoint transfer] → characterize and integration-test Wide↔Normal/Narrow round-trips preserving stable target plus selected-row offset through one anchor.
- [Retained geometry expires or a non-item row is accidentally claimed] → test clicks and wheel after the current draw, including blank/header no-ops and Inline detail-cover resolution.
- [The retained seam accidentally leaves a compatibility underpaint] → assert the base frame paints no Feeds rows and exactly one canonical list painter runs per frame at each presentation.
- [Progress projection displaces duration or misclassifies played entries] → add buffer evidence for resumable, played, and ordinary row states through canonical slots.
- [Shared pill rendering changes spacing or truncation unexpectedly] → compare against the Emby reference and cover watched-filter pills and long labels in rendered buffers.

## Migration Plan

1. Characterize active/inactive control behavior, retained geometry, responsive handoff, current framing, and presentation output at Wide and Normal/Narrow.
2. Transfer movement and geometry authority to the active persistent control and delete Feeds compatibility maps and offset writeback.
3. Converge watched-filter, resume badge, hero-slot documentation, and pill truncation through existing shared presentation primitives.
4. Run focused, render-buffer, and shell-tick integration tests plus the required repository gates.
5. If needed, revert the bounded source change; no persisted data migration or specification rollback is involved.
