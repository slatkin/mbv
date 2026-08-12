## Context

The grouped music album view currently mixes album rows and artist headers in one navigation-target list. `LibraryTab::artist_header_focus` represents header selection separately from the album cursor and `album_track_focus`, and action resolution branches on that state for artist-wide playback and queue operations. See `proposal.md` for motivation and the delta specs for the revised behavior.

## Goals / Non-Goals

**Goals:**

- Keep artist headers and settled grouping visually unchanged while removing header focus and action scope.
- Make album selection the only grouped-view item scope outside track selection.
- Remove obsolete state and branches rather than retaining an unreachable header-focus mode.

**Non-Goals:**

- Changing grouping identity, ordering, settlement, or snapshot replacement.
- Changing album or track cursor semantics.
- Replacing removed artist-header bulk actions elsewhere.
- Changing the responsive Music layout.

## Decisions

### 1. Remove header targets from navigation rather than skipping them after movement

The grouped-view navigation target projection will contain album targets only. Artist-header display rows remain in the render plan, but cursor movement, paging, visibility clamping, and mouse selection operate only on album targets.

This makes non-selectability structural. Retaining header targets and teaching every input path to skip them would preserve an invalid state and leave future actions vulnerable to resolving a header accidentally.

### 2. Remove artist-header focus state and action branches

`artist_header_focus` and its selection type will be deleted. Current-item and action-scope resolution will choose the focused track when `album_track_focus` is active and the selected album otherwise. Artist-wide Play, Shuffle, Enqueue, and context-menu branches tied to header focus will be removed.

Keeping the state as permanently `None` was rejected because it would leave dead model surface and force unrelated layout work to continue handling a third focus state.

### 3. Retire modified PageUp/PageDown artist jumps

The grouped-view Ctrl+PageUp/PageDown dispatch and jump helpers will be removed. Modified PageUp/PageDown remains consumed as an unmapped modified key rather than falling through to ordinary album paging. Existing unmodified paging and arrow navigation remain unchanged.

### 4. Make artist-header mouse rows inert

Artist headers may remain in render-derived row geometry for display and scroll calculations, but they will not produce selectable row targets. Clicking a header leaves the selected album unchanged. Album-row click and double-click behavior remains unchanged.

### 5. Prefer deletion and adjustment of existing tests

Tests whose only contract is selectable-header behavior will be deleted. Existing grouped-navigation, action-scope, and mouse tests will be adjusted only where they provide durable coverage that album selection remains stable across visual headers. No pixel-specific rendering tests are required for this behavioral removal.

## Risks / Trade-offs

- **[Removed bulk actions]** Artist-wide Play, Shuffle, and Enqueue are no longer reachable from grouped headers. -> This is intentional; no replacement affordance is part of this change.
- **[Navigation regression at group boundaries]** Removing mixed targets could make cursor movement or scroll anchoring skip an album. -> Keep display rows separate from album navigation targets and verify movement across at least one artist boundary.
- **[Stale dead branches]** Header-focus checks distributed across input, actions, and rendering could survive the state deletion. -> Use compiler errors and structural searches to remove all references before verification.
