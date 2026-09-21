# Design

## Context

See `proposal.md` for motivation. Today `prefs.json` stores selected-tab position, Panel focus, pane widths, and audio UI values; `library_position_state.json` stores a map of per-library browse snapshots. `App::save_prefs` rewrites preferences from many live event paths, and library positions are copied into shell-owned snapshots during navigation then flushed after a short idle delay. Local-daemon Clients and Bare-mode TUIs use the same per-user paths.

Destination interaction state now belongs to mounted Interactive Components. Main Selector pills use several destination-specific identities (for example a Home section source, a letter range, a Feed group/filter, an Audiobookshelf podcast state/show, or a surname bucket), and canonical lists already select stable targets. The shell owns persistence but must not regain continuous mirrors of these component-local values.

Startup is asynchronous: the saved tab can be resolved only after the live Service library catalog exists, and pill/item identities can be resolved only after the selected destination's current content exists. Restoration therefore needs one bounded pending intent rather than eager index assignment.

## Goals / Non-Goals

**Goals:**

- Represent one selected-tab launch location with stable, destination-aware identities.
- Extract component-owned pill/item state once at orderly teardown without live reverse synchronization.
- Resolve restoration in tab -> main Selector pill -> library item order as current catalogs and content become available.
- Keep the saved file safe from partial writes and from two Clients sharing one temporary pathname.
- Retire live disk writes and the per-library persistence map for this UI state.

**Non-Goals:**

- Session identity, per-terminal profiles, field-level merging, inter-process locking, or daemon-mediated UI state.
- Crash recovery for navigation performed since startup.
- Restoring Queue selection, scroll offsets, nested Workspace selectors, or state for unselected tabs.
- Changing persistence lifecycles for configuration, playback progress, queue state, auto-reconnect, or caches.

## Decisions

### 1. Store one typed launch snapshot in one state file

Add a versioned `TuiLaunchState` containing:

- `TabIdentity`: fixed Home/Feeds identities or Service kind plus library ID;
- `PanelFocus`;
- an optional destination-tagged main `SelectorIdentity`;
- an optional destination-tagged stable `LibraryItemIdentity`.

The selector and item identities are tagged so an identity from one destination cannot accidentally resolve in another. A tab with no main Selector pills records no selector. The snapshot contains no indices except where the presented choice itself is a fixed, closed ordered value whose identity is that value.

This replaces using `prefs.json` and the per-library position map for these four launch concerns. Unrelated preferences remain in their existing file and lifecycle.

Alternatives considered:

- Extend the per-library map: rejected because the required state is only for the selected tab and retaining every tab recreates the undesired scope.
- Store display strings or numeric positions: rejected because catalog ordering and labels can change.
- One generic string for every target: rejected because it discards destination/type boundaries and makes accidental cross-resolution easy.

Known limitation: canonical Music album targets derive from the `music_wide.rs` `album_targets` dedupe, so when duplicate album ids exist the target becomes `albumId\0<rowIndex>` — an order-derived suffix. A saved `LibraryItemIdentity` for such a duplicate album may therefore degrade gracefully to fallback selection at restore rather than exact restoration. This is pre-existing canonical-target behavior, not introduced by this change.

### 2. Components expose a read-only exit snapshot, not live persistence messages

Extend the existing selected destination/content-owner boundary with a small query that returns its current main Selector and selected library-item identities. The shell invokes it only while assembling orderly teardown state. It does not query unselected destinations.

Tab selection and Panel focus remain shell-known because they already govern composition. Component values remain private during the run: local pill or item movement emits no persistence-only request, and no render/sync pass copies those values into `App`.

Alternative considered: emit a message after every local movement to maintain a shell snapshot. Rejected because it is the live mirror and disk-oriented interaction traffic this change is meant to remove.

### 3. Restore through a one-shot pending launch intent

Load the snapshot once at startup into a pending restoration value. Resolve it in stages:

1. After the current tab catalog is known, select the saved `TabIdentity` or the first guaranteed tab.
2. After that destination's main Selector choices are known, select the saved `SelectorIdentity` or the first guaranteed pill. If the destination has no main Selector pills, continue in its unfiltered scope.
3. After rows for that scope are known, select the saved `LibraryItemIdentity` if it is present and selectable; otherwise select the first selectable row. An empty row set produces no selection.
4. Restore Panel focus without supplying a Queue target; Queue runs its normal initialization.
5. Consume the pending intent so ordinary refreshes cannot reapply it over later user movement.

Each component receives restoration through an explicit discrete re-anchor method. This is permitted by the Interactive Component contract and is distinct from ordinary content projection.

Alternative considered: clamp old indices during construction. Rejected because indices do not identify the same content after catalog or filter changes and because some content does not exist until asynchronous loading completes.

### 4. Exit replacement is intentionally last-completed-exit wins

Orderly teardown reads the selected destination once, serializes the complete snapshot, writes a process-unique temporary file in the state directory, and atomically renames it over the launch-state file. Process-unique temporary names prevent concurrent Clients from interfering before rename; no lock or merge is added. The last successful rename is the snapshot a later launch reads.

This intentionally treats the file as a future-launch seed, not synchronized shared state. A killed or crashed TUI leaves the previous completed snapshot intact.

Alternatives considered:

- Lock plus read/merge/write: rejected because there is one coherent selected-session snapshot, not independently mergeable fields.
- Per-Client files or identities: rejected because concurrent TUIs are normally sessions of the same user and the product does not need session profiles.
- Continue live debounce writes: rejected because it leaves avoidable contention throughout both sessions.

### 5. Legacy files provide at most a one-time seed

When the new launch-state file is absent, startup may derive one initial location from the existing selected tab/Panel-focus preferences and that selected tab's legacy browse position where a stable identity can be recovered. It must not restore state for unselected tabs or infer a missing identity from a stale numeric cursor. After the first orderly exit writes the new snapshot, the new file is authoritative.

Legacy launch-related keys and `library_position_state.json` stop receiving writes. They can be ignored after successful migration and removed once no unrelated behavior reads them. Malformed or unresolvable legacy state falls through to the same first-valid-choice rules rather than preventing startup.

Alternative considered: require a clean default after upgrade. Rejected because the existing state can preserve the user's current launch location without perpetuating the old model.

## Risks / Trade-offs

- **A crash loses navigation changes from the current run** -> Keep the previous completed snapshot; this state is convenience state, while valuable configuration and progress retain their existing persistence.
- **Some current pills are represented only by positions** -> Introduce the narrowest stable destination-specific identity, using fixed enum values for closed pill sets and Service content IDs for dynamic pills.
- **Asynchronous restoration can overwrite early user input** -> Consume or cancel pending restoration for a level when the user explicitly changes that level before its saved identity resolves.
- **Legacy browse snapshots contain more detail than the new snapshot** -> Migrate only identities required by the new contract and deliberately discard depth, scroll, and unselected-tab state.
- **Two exits can finish nearly simultaneously** -> Process-unique temporary files plus atomic replacement avoid corruption; whichever replacement completes last intentionally wins.

## Migration Plan

1. Introduce the versioned launch-state type, path, atomic save/load functions, and backward-compatible legacy seed reader.
2. Add bounded snapshot and explicit restore/re-anchor APIs to destination owners, starting with identity types already used by their canonical lists and Selector rows.
3. Route startup through the pending hierarchical restore and make Queue initialization independent.
4. Assemble and save one snapshot during orderly teardown.
5. Remove launch-state disk writes from live tab, focus, pill, item, refresh, and timer paths; stop writing the per-library map and remove obsolete dirty/flush state.
6. After compatibility coverage passes, leave old files readable only for the new-file-absent migration path; rollback remains possible because older builds ignore the new file and retain their existing defaults/legacy data.
