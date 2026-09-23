# Design

## Context

See proposal.md — Why and the existing delta specs. A `canonical-media-lists` delta must also be created before implementation to replace its flat-TV-browser clauses; that artifact does not yet exist. The shared `TreeBrowser<Target>` already owns tree reconciliation, expansion, selection, filtering, marks, and paint, but currently turns every visible node into a selectable `Row`. TV currently owns a flat series `MediaListCarrier`, a separate episode carrier in the Hero Workspace, season cursor, and Inline Search. Shell-projected `TvWideRenderCtx` supplies series detail asynchronously. The completed TV pills change defines flat `Latest`/`Upcoming` modes and show-oriented `All`/range modes; its code is not present in this checkout, so implementation must start from the branch containing it rather than replace it.

## Goals / Non-Goals

**Goals:** One tree owner for TV show modes at every geometry; optional structural headings in the shared tree; keep all Hero and flat-mode owners and effects intact.

**Non-Goals:** No replacement of season pills or Workspace episodes, no new tree-specific fetching endpoint, no Music behavior change, no new global key-routing site, no nested Latest/Upcoming list.

## Decisions

### D1: Represent headings as structural projected rows, not fake targets

Extend the tree's input vocabulary with plain structural heading and spacer entries, separate from selectable `TreeNode<Target>`. Interleave headings before root shows and spacers between consecutive groups in one settled visible row flow, preserving TV's current grouped-list separators. Structural rows have no destination target, exposed tree-library node handle, or parent/child edges. The shared cursor, paging, filter, viewport, painting, and point resolution use that same flow; structural rows are skipped by selection, marks, hit resolution, and effects. Stripe and heading appearance follow the shared visual contract confirmed against the rendered surface: zebra striping resets at each heading (a row's stripe derives from its position within its group, not its absolute flow or viewport position, so stripes hold stable across scroll and never run continuously across a group boundary), a heading sits on the depth-zero title baseline via the shared two-column content inset, and ordinary node titles fall back by depth through the cream, yellow, and aqua roles. Reconciliation validates node targets/edges atomically and also invalidates retained geometry when heading content changes. Without headings the old Music projection and pixels remain unchanged.

Alternative: synthesize selectable group roots with `TreeMarkPolicy::Excluded`; rejected because excluded marks do not prevent cursor, expansion, context, and activation. Alternative: have TV paint headings around the tree; rejected because it creates a second scroll/hit owner.

### D2: TV provides typed identities and plain projection, not a TV tree engine

Use a closed TV target type for Show, Season(show, season), and Episode(show, season, episode) so repeated child ids cannot collide. Project sorted show roots in existing mode/bucket order, one heading per existing group boundary and a spacer before each subsequent heading, with seasons and loaded episodes underneath. The TV owner retains Emby snapshots for translating `TreeExternalIntent` into the existing shell requests; it does not implement a tree model or view. Only show modes hand the tree to the Library Panel's generic `PanelList` integration; Latest/Upcoming and Inline Search continue to use their flat controls. All geometries refer to the same tree owner.

Alternative: make a TV wrapper around `TreeBrowser`; rejected because it would re-create the duplicate state owner the extraction removed.

### D3: Expand against shell-owned details without losing expansion

The shell remains responsible for fetching show detail and season episodes; the tree holds only projected rows. A `TreeNode` declares whether it can have children even before they are projected. TreeBrowser's `ToggleExpansion` and `ToggleExpansionTarget` must accept a declared-expandable node with zero loaded children (both currently return `Unhandled` in that case), retaining its expanded state without inventing a selectable placeholder. Music nodes without that declaration retain their child-derived expansion behavior. Opening an uncached show or season requests its data through the existing shell fetch path (deduplicating a show fetch already started for the Hero). An empty completed result removes the pending expandability unless actual children exist. While a request is in flight, retain the expanded branch and selected stable target; when data lands, reconcile its children without resetting selection or viewport. Existing shell identity checks on completed detail remain in effect. The existing Hero projection continues to track the containing show even when the tree selection is a season/episode, while its own season and episode cursors remain separate. On show switch, do not let a previously fetched detail overwrite the current Hero. On entering a flat mode, retain the show tree locally for return without painting its rows; switching modes still applies the existing mode selection rules.

Alternative: preload every season's episodes on library entry; rejected for large libraries and because it changes fetch volume. Alternative: share one episode cursor with the Hero; rejected because the user explicitly keeps both interactive copies.

### D4: Translate resolved tree actions at the TV boundary

Show activation retains current show/Workspace behavior, season activation toggles expansion, and episode activation emits the same episode playback request as the Hero Workspace. Navigation uses `apply(TreeOperation)` transitions and selected stable target rather than translating back through the former flat-series cursor. Keep context/playback effects resolved from the selected target's Emby snapshot; headings return no target. Preserve the single central keyboard router and Library Panel's latest-frame mouse eligibility and hit geometry. TV Inline Search remains independent from the shared tree filter session.

Alternative: reuse `TvHit::SeriesRow` for every tree depth; rejected because it conflates episode and show activation. Reuse existing typed requests when possible, adding distinct TV tree intents only where the current vocabulary cannot represent the child target.

## Risks / Trade-offs

- Heading geometry drifts from target geometry → One settled flow and focused shared-component buffer, pointer, and invalidation tests.
- Child fetch responses race selection or refresh → Shell guards completion by show/season identity; preserve expansion during loading and assert late-response behavior.
- Two visible episode locations can diverge in selection → Independent tree and Hero Workspace cursors by design; playback resolves the cursor that produced the intent.
- Existing pill change is on a separate branch → Apply only after that change is incorporated and verify flat modes and selector persistence as regressions.

## Migration Plan

1. Land or incorporate the completed TV pills change **and sync its spec deltas** before touching TV's projection. Create the missing `canonical-media-lists` delta replacing its TV-specific flat-browser obligations before implementation.
2. Add structural heading/spacer and pending-expandability support to the shared tree and establish Music behavior-neutral coverage.
3. Convert TV show modes to the tree using existing detail fetches; keep flat modes, Inline Search, and Hero Workspace.
4. Verify focused tree/TV behavior, panel tick and mouse routing, Narrow/Wide/Mini, then full package tests and formatting. No persisted format or protocol migration is needed; rollback is a commit revert.
