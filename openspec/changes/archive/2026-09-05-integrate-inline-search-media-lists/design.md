## Context

See `proposal.md` — Why, and `specs/inline-library-search/spec.md` for the responsive-continuity delta. The existing `openspec/specs/inline-library-search/spec.md` remains the contract for input size, filtering, full-corpus loading, flat result painting, navigation, activation, and dismissal.

Inline Search is currently an independently mounted `InlineSearchComponent`. A destination emits `OpenInlineSearch`; the shell mounts and focuses the search component, pushes either plain Emby items or recursive album-index entries, chooses a `Rect` from `LayoutMain`, and paints the search after the destination. Wide destinations now own their internal arrangements, so that shell-selected rect is neither a stable placement contract nor consistent across destination kinds:

- Wide TV supplies the right rail, but the search component skips its own input in Wide presentation and the TV workspace is not given the query to paint it.
- Wide Music publishes its left Hero pane as `left_area`; `inline_search_area` selects that before `wide_music_browser_area`.
- Browser is the only destination onto which the shell projects the search query/loading state so an underlying destination painter can supply the missing Wide input.

The result is a two-painter protocol whose correctness depends on paint-derived geometry. That conflicts with ADR 0022's authority bar and the one-owner/one-painter contract. ADR 0022 also explicitly records Inline Search as a separate Interactive Component, so this change must supersede that clause rather than silently diverge from the accepted decision.

`BrowserComponent` owns generic, Movies, home-video, Emby podcast, and Normal TV lists. `MusicWorkspaceComponent` owns Music at all presentations. `TvWorkspaceComponent` owns Wide TV while `BrowserComponent` owns Normal TV; both stay mounted for the live library and `sync_active_destination` changes the active owner at the responsive transition. The shell remains responsible for full-library fetches, album-index construction, stale completion guards, navigation-stack mutation, and activation effects.

Implementation should follow `replace-wide-paint-inference` (#643). That change removes the current one-frame resize staleness from the same breakpoint decisions; this change then removes the Inline Search `set_wide` consumer rather than reopening paint inference.

## Goals / Non-Goals

**Goals:**

- Give the destination Interactive Component one ownership boundary for its ordinary list and Inline Search presentation.
- Reuse one embedded search control and one host contract across Browser, Music, and TV rather than copy query/filter/navigation logic.
- Preserve search state through the TV Normal/Wide owner handoff without a per-frame shell mirror.
- Keep keyboard and mouse resolution inside the component that painted the search result geometry.
- Make the existing specification executable again, including empty and one-character queries, score ordering, page movement, empty-query Backspace dismissal, and the three-row admission rule.

**Non-Goals:**

- Add Inline Search to Home, Audiobookshelf, Feeds, or the Search sidebar.
- Change the Search sidebar or merge its server-backed search model with library-local fuzzy search.
- Change full-library fetches, recursive album-index construction, recursive album activation, or Emby navigation semantics.
- Add new mouse gestures; existing left-click selection remains the required parity.
- Update the Hero pane from the highlighted search result. The ordinary destination selection and detail remain unchanged until activation, and dismissal restores them unchanged.
- Persist an open search across process restarts or destination changes.

## Decisions

### 1. Inline Search becomes an embedded plain TuiRealm Component

Keep `src/app/components/inline_search.rs`, but replace the independently mounted `InlineSearchComponent` with a plain embedded `InlineSearch` control. It owns:

- active/inactive state,
- query,
- plain or recursive-album candidate pool,
- scored result order,
- result cursor and scroll,
- loading state,
- its last painted result geometry, and
- its private mouse gesture state.

`BrowserComponent`, `MusicWorkspaceComponent`, and `TvWorkspaceComponent` each embed one control. A small `InlineSearchHost` trait exposes the common control to shell adapters; it does not define another application framework, choose a destination, or expose Service/runtime objects.

The mounted destination remains the sole event boundary. It gives search first refusal while search is active, translates search activation into the existing typed shell request, and otherwise runs its ordinary keyboard or mouse path. The embedded control is never mounted, focused, subscribed, or given a `ComponentId`.

This matches the existing `WideMediaList` and `InlineMediaBrowser` composition rule: the mounted parent owns the surface and delegates local mechanics to a reusable plain Component.

**Alternatives rejected:**

- Keep the mounted overlay and repair `inline_search_area`: it leaves the two-painter protocol and paint-derived placement that caused the bug.
- Project search results/cursor into destination components while the mounted search component keeps input state: that creates two owners and a per-frame interaction-state mirror.
- Put fuzzy matching in `WideMediaList` or `InlineMediaBrowser`: those controls are provider-neutral list mechanics, while Inline Search has Emby-specific full-corpus and recursive-album inputs.
- Copy search fields and handlers into all three destinations: less initial plumbing, but three implementations of the same specification would drift.

### 2. One shared control filters an explicit candidate pool

Retain two candidate-pool forms:

- plain Emby items, matched against `display_name()`;
- recursive album entries, matched against indexed `search_text` and displayed with the indexed label.

On every query change, compute `(original_index, score)` for every match and stable-sort by descending score. Stable ties retain corpus order. An empty query uses all original indices in corpus order; there is no two-character threshold. Store result indices instead of cloning a new result vector for every cursor move, selection, and frame.

A query edit resets result cursor and scroll to zero. A candidate-pool replacement preserves the selected stable target when it still exists and otherwise clamps to the first valid result. Loading is independent of whether the currently supplied pool is empty or partial, so an in-flight full-library fetch cannot look like a final empty result.

The control resolves activation to the selected item's stable opaque identity (`id` plus item type). The shell keeps recursive-album lookup and all navigation effects; no album ancestors or protocol object move into the destination.

### 3. The destination owns placement and invokes one search painter

Add one shared arrangement helper that takes the exact library-list area owned by the destination and returns either:

- a three-row input area plus the remaining result area, when at least three rows are available; or
- no input area plus the original full result area, when the input cannot fit.

The shared search Render Component paints the bordered input/loading state and flat column-aware results into those areas. It publishes result row/cell geometry back only to the embedded control that invoked it.

Each destination branches at its existing list composition point:

- Normal and non-Hero catalogs pass their list area.
- Hero-on-left Browser, Music, and TV pass the right-rail library-list area; the Hero pane remains visible.
- While search is active, the ordinary canonical list/grouped painter does not paint that same area.

There is no shell-supplied internal search rect, no Wide flag on the control, and no Browser-only underpaint of the input. The parent still paints any surrounding destination frame that it owns.

**Alternative rejected:** always paint the input at the top of the destination's whole outer area. That is simple but wrong for Hero-on-left because it crosses the Hero and browser ownership boundary.

### 4. Search lifecycle is local; shell work remains request-driven

Pressing `/` in an eligible destination starts the embedded control locally and emits `OpenInlineSearch` only for work outside its authority. The shell then selects and pushes the current plain/recursive candidate pool and starts a full-library fetch or album-index build when needed.

While active, the destination returns immediately from its ordinary key handler after delegating to search, even when search emits no message. This ensures printable characters and list shortcut letters edit the query instead of reaching the ordinary destination path. Up, Down, PageUp, PageDown, Home, End, Enter, Escape, and Backspace are resolved by the shared control. Escape and Backspace on an empty query dismiss locally; Enter emits the resolved activation request.

The central Keyboard Router remains the sole precedence authority. Its plain-data snapshot derives `text_entry_focused` from the active destination host's search state, replacing the current inference from a focused `ComponentId::InlineSearch`. No raw key crosses into the shell and no second router is introduced.

The shell pushes pool/loading changes only at lifecycle boundaries: open, validated full-library completion, validated album-index completion, and navigation events that replace the applicable corpus. A completion for a closed search or a different selected library is ignored by the existing identity guards.

### 5. TV transfers one snapshot on the responsive owner switch

Most destinations retain one Interactive Component across presentation changes. TV is the exception: Normal uses `BrowserComponent`, Wide uses `TvWorkspaceComponent`.

Extend the existing TV active-destination handoff with one `InlineSearchTransfer` containing:

- whether search is open,
- query,
- selected stable target, and
- selected result's viewport row offset.

When the breakpoint changes without changing the selected destination, take the transfer from the outgoing owner, clear its search control, push the shell-owned candidate pool/loading state to the incoming owner, and apply the transfer once. The incoming control recomputes scores from its pool, restores the selected target, and keeps it visible at the prior row offset when the new viewport can represent that offset.

This is a discrete responsive transition, analogous to `ViewportAnchor`; it is not a live shell mirror. A tab change dismisses search instead of transferring it. Async load/index work continues because it is keyed to the same selected library rather than to a component ID.

**Alternatives rejected:**

- Keep both TV controls synchronized every frame: violates single-owner local state.
- Leave search mounted as a state holder only for transitions: preserves a phantom Interactive Component and splits event/paint ownership.
- Dismiss on resize: simpler, but violates the responsive-continuity requirement.

### 6. Mouse handling stays with the painted destination

The active destination is already mouse-eligible under ADR 0024. While search is active, its mouse handler delegates first to the embedded search control and does not let the ordinary list mutate for points in the search area. The control resolves a left click through the result geometry it painted, including columns, and moves its own cursor. Other gestures keep their current no-op behavior.

Removing the separate mount also removes Inline Search from mouse-eligibility and destination-mount reconciliation. No subscription is added for the embedded control.

### 7. Remove the obsolete overlay protocol completely

Delete:

- `ComponentId::InlineSearch` and its mount/reconciliation cases,
- `inline_search_expected_id`, `inline_search_component_id`, `inline_search_area`, separate mount/focus/render functions, and `set_wide`,
- the `draw_frame` search-overlay render pass,
- Browser-only query/loading projection,
- `App::inline_search_active` and its mount-state projection when no remaining legacy painter requires it,
- `InlineSearchDismiss` if dismissal has no shell effect, and
- tests whose only purpose is the removed mount/overlay protocol.

Retain and adapt the shell's full-load, album-index, stale-result, and activation-effect paths. Prefer strengthening existing destination and shell integration tests over preserving tests of deleted getters or mount IDs.

### 8. Record the changed architecture and vocabulary

Add ADR 0025 recording that Inline Search is an embedded destination capability because list placement and local interaction must have one owner. Mark ADR 0022's sentence that requires a separate `InlineSearchComponent` as superseded by ADR 0025; leave the rest of ADR 0022 accepted.

Update the Inline Search row in `docs/architecture/interactive-surface-ledger.md` to record Browser/Music/TV ownership per presentation and remove the independent surface row if the ledger's combine-row rule is satisfied by ADR 0025.

Correct `CONTEXT.md` so **Inline Search** is described as library-scoped filtering of the selected Emby destination and remains explicitly distinct from the cross-library **Search sidebar**. This clarifies an existing contradiction; it does not rename either concept.

## Risks / Trade-offs

- **TV transfer can accidentally become a per-frame mirror.** → Limit it to the existing active-destination breakpoint edge and consume the transfer once.
- **A host may handle ordinary keys after search has already consumed them.** → Return immediately from each host's search-active branch and verify shortcut letters through a real `Application::tick()` test.
- **Ordinary and search painters may both paint the list area.** → Branch at each destination's existing list composition point and add one buffer test spanning Browser, Music, and TV Wide placement rather than duplicating implementation-shaped tests.
- **A late full-library or album-index completion may reopen or overwrite another search.** → Keep the selected-library and parent/index generation guards; ignore pushes when the active host is closed.
- **Restoring by numeric cursor selects the wrong item after score recomputation.** → Transfer and preserve stable target identity, then derive the new cursor.
- **Three-row input admission reduces already-small result viewports.** → Follow the existing specification exactly: omit the input and use the full list area when three rows do not fit.
- **Fuzzy scoring allocates and sorts on each edit.** → Recompute only on query/pool changes and store result indices; do not add debounce, worker, or cache machinery unless measured library sizes make local edits visibly slow.
- **The plan overlaps #643's `InlineSearchComponent::set_wide` work.** → Land/rebase after `replace-wide-paint-inference`; accept deletion of that now-correct adapter rather than retaining compatibility code.

## Migration Plan

1. Land or rebase onto `replace-wide-paint-inference` so responsive owner selection is paint-free.
2. Extract the current search state/filter/navigation/painter into the embedded control and make its existing specification tests pass independently.
3. Embed and route it through Browser, Music, and TV one destination at a time, keeping the old mounted path only until all three destination paths are verified.
4. Add the TV one-shot transfer and responsive integration check.
5. Delete the mounted overlay protocol and obsolete projections in the same change; a mixed ownership state is not mergeable.
6. Update ADR 0022, add ADR 0025, correct `CONTEXT.md`, and update the interactive-surface ledger.
7. Run component, shell-tick, full package, lint, architecture, formatting, and file-size checks.

Rollback is a normal source revert; there is no data, configuration, dependency, or protocol migration.
