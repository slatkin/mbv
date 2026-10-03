# Design

## Context

See `proposal.md` for motivation and [issue #745](https://github.com/slatkin/mbv/issues/745) for tracking. This design is grounded in the current checkout; no new remote API contract is proposed.

- `src/app/dispatch/library/load.rs::refresh_current_view` branches on effective Panel focus. Its Library arm clears `list_pane_width` and saves preferences; `refresh_lib` removes saved library position. The Queue arm dispatches an owner `QueueOp::Refresh`.
- Audiobookshelf refresh in `dispatch/audiobookshelf/browse.rs` and `browse/books.rs` clears the catalog before re-requesting page zero. Podcast refresh also clears episodes and `committed_show_pill`. Empty content pushes thereby reset component state.
- Audiobookshelf result handling appends pages, deduplicating existing identities rather than replacing their metadata. Merely keeping old rows and calling the existing append helpers would not fetch a genuinely current catalog.
- `LibraryPanel` retains seven concrete content-owner families through its existing `LibraryOwners` map: Home, generic Emby, Music, TV, Feeds, Audiobookshelf podcasts and books. Each owns local interaction; shell navigation/data and persisted resting positions remain distinct.
- `SettingsIntent::Activate(cursor)` already crosses the component boundary. `Model::handle_settings_intent` resolves it to `SettingKey`; the ordinary App activation path schedules a configuration save. The reset action must bypass that path.
- `prefs.json` mixes layout (`queue_column_width`, `list_pane_width`, `visual_slot_hidden`) with volume/mute. Deleting it wholesale would violate the protected-state contract.
- `tui_launch_state.json` is loaded once and saved at orderly exit. `LaunchRestore` can remain pending through Service startup. `library_position_state.json` is separate and covers per-library resting positions.

## Goals / Non-Goals

**Goals:** Keep data replacement separate from presentation reset; retain one owner for each live UI value; target reset persistence narrowly and report failures.

**Non-Goals:** No generic refresh framework, global UI store, dependency addition, new router, API endpoint, ctrl message or Owner-process reset. Do not change timers/tab-activation fetch policy, other global-key bugs, destination layout design, or unrelated stale-completion mechanisms.

## Decisions

### 1. F5 keeps its existing global input path, with destination-only effect dispatch

Keep the `f5_refresh` keybind ID, default chord and central `NoBlockingOverlay` gate. Remove the Panel-focus branch from `refresh_current_view` and dispatch exhaustively from the normalized selected `TabSelection`. Keep `force_clear` if needed to redraw terminal images; a repaint is not a reset. Delete the now-unused private `refresh_queue` helper if it has no remaining callers; do not delete owner Queue refresh support.

Remove the split reset and preference save from F5, and remove `clear_saved_library_position` from the shared `refresh_lib` path so bare `r`/context refresh cannot recreate the same reset indirectly. Keep independent callers of that invalidation helper. Preserve stale-destination normalization: an absent selected library normalizes to Home and does no fetch for that triggering invocation.

Alternative rejected: leaving focus-dependent Queue refresh in place. The user explicitly selected a global, selected-destination data action, including while Queue is focused or the Library is hidden.

### 2. Refetch content without publishing destructive loading snapshots

Use the existing Service-specific requests and result/projection seams. Keep component-owned selection, pill, expansion, focus and viewport in the mounted owner, not a shell snapshot saved and reapplied on every push.

For Emby/Home/Feeds, preserve existing loaded content during requests and update it on success. Audit their shared result paths for destructive re-anchors or clearing; retain only necessary current-content reconciliation. Refetch sources already belonging to the selected destination, including its existing Latest source and active detail context, rather than broadening into other libraries or artwork-cache eviction. Local Inline Search remains open; refresh the data/index it uses, not the cross-library Search sidebar or its query.

For Audiobookshelf, use a library-scoped replacement batch within the existing catalog state/result handling:

1. Keep the published catalog and episode/detail content while loading. Collect replacement pages separately, using the existing bounded page traversal (which currently loads the catalog to completion), rather than appending to old published rows.
2. Publish the replacement catalog only once traversal succeeds. A failure discards the incomplete replacement and keeps the prior published catalog. Resolve show-pill existence only against a complete replacement, not its first page.
3. Podcast fan-out follows the current committed pill: one show for a show pill; the existing bounded all-show fan-out for a state pill. Retain each show's old episodes until its fresh result succeeds. A failed show keeps its prior episodes. Successful results replace that show's episodes, including deletions; unrelated library caches are untouched. Latest keeps its existing shelf-specific success/failure behavior.
4. Refresh the selected book's chapter/audio-file detail through the existing request, retaining old detail until success. Existing book selection-by-identity behavior (including following a selected book into its new surname bucket) remains authoritative.
5. Do not treat a selected episode as gone merely because its owning show's fresh result has not arrived. Allow disappearance reconciliation after that show's authoritative successful result; preserve the latest local selection if the user moves during the fetch.

A per-library in-flight refresh state prevents duplicate F5 batches; existing loading feedback suffices. Reuse the podcast episode request identity. Where page/book-detail events currently carry only a Service setup generation, carry a minimal library-local request identity so a pre-refresh result cannot overwrite newer refreshed content in the same setup. Keep the existing setup-generation guard as well. No reusable global generation framework is needed.

Alternative rejected: keep old rows but append fresh pages through deduplication. It preserves stale metadata and deleted records. Alternative rejected: clear first then restore a shell-owned cursor/pill mirror. It creates two UI owners and drops late-page selection.

### 3. Reset defaults are explicit and owner-local

The `tui-state-reset` spec defines the observable inventory. Implement minimal content-preserving reset methods in the existing owners/embedded controls; use normal constructor/default choices against current content. A tree reset clears selection/marks/filter/expansion/viewport and paint geometry without discarding its node data. A list reset preserves rows and restores initial selection/scroll. Destination methods also reset pills, local Workspace focus, Hero scroll and Inline Search.

Once concrete methods exist, expose one required `LibraryContentOwner` reset operation and let `LibraryOwners` apply it to every retained owner, including inactive ones. Do not use a default no-op as the completed implementation. Reset `LibraryPanel`'s Hero overlay and gesture/retained geometry alongside owners. Reset Queue-local presentation via its component, never through queue operations. Content owners stay mounted, preserving the catalog-retention contract; App/Model are not recreated.

Shell-owned browse stacks return to root, sort/filter/content-mode defaults use existing library-kind/count rules, and saved resting positions are cleared. Keep data caches and Service state. Do not mass-fetch default scopes: ordinary activation can lazily load an uncached root when needed. Home is selected. Restore default queue-column width, no custom Hero split, visible artwork, artwork rather than visualizer, default Panel mode, and existing geometry-specific startup focus (Library in ordinary two-panel geometry; Queue in geometry-forced Mini). Queue scope and playback-target relationships are preserved.

Clear pending launch restore, UI navigation/landing/re-anchor intents, local marks/status selection summaries, prefix/Esc timing and pointer gesture state. Dismiss overlays through existing TuiRealm lifecycle helpers, including Settings, so transient drafts/search workers cannot re-open them. Do not clear accepted domain mutations, queue-edit undo, Owner-request state or independent content fetches. Existing query/navigation identity checks must reject late UI-targeted completions after reset; add only a missing specific guard, not a new global cancellation framework. Clear any already-produced same-tick transient UI intent before the reset sync.

Keep launch-window timestamps and Latest acknowledgements: they define already-seen content for the run, not disposable cursor/layout state. Stopping a visualizer capture worker is permissible presentation cleanup; it sends no Player transport command.

Alternative rejected: reconstruct App or unmount/rebuild every destination. That risks destroying runtime state/caches and violates retained-owner boundaries. A single semantic reset at the existing ownership seams is sufficient.

### 4. Clear saved presentation selectively, with a narrow launch-state exception

Use `mbv-config` for fallible presentation-state persistence operations and domain errors. Remove the saved launch snapshot (missing is success), clear persisted per-library positions with the existing fallible save boundary, and patch only presentation entries in preferences. Remove legacy tab/focus/layout aliases that could restore the old location or old widths; preserve volume/mute and unknown unrelated keys. Do not delete the whole state directory, credentials, queue files, config, image cache or auto-reconnect target.

Update live layout first and save its default preferences through a fallible path; the existing `save_prefs` silently discards failures, so it is not adequate evidence of a successful reset. Do not write a default launch snapshot: explicitly clearing it is the narrow exception captured in `tui-launch-state`, while new snapshots remain exit-only. Pending in-memory `LaunchRestore` becomes Done before another sync pass can restore it.

Attempt all independent presentation-clear operations and report a concise partial-failure error if any fails. The live interface stays reset. Do not promise transactional rollback across these independent files or retry with a destructive broad delete. Success feedback is emitted only when all required clears succeeded. Another Client's later exit can save its own snapshot, unchanged from current last-completed-exit behavior.

Alternative rejected: wait for orderly exit. The user confirmed that another Client launched immediately after reset must see cleared saved UI state. Alternative rejected: permanently disable saves after reset; subsequent navigation/exit should work normally.

### 5. F2 reuses its existing action/message machinery

Add `SettingKey::ResetUiState` as a value-less action in Settings' Actions section. Generalize the snapshot's currently hard-coded final LogOut row so both action rows participate in the same cursor mapping. Reuse `SettingsIntent::Activate`; intercept the resolved reset key in `Model::handle_settings_intent` and call the shell reset coordinator before falling through to App's configuration-edit path. No new Msg variant, config setting, dedicated popup or keyboard binding is needed. Execution is immediate and Settings closes; this discards presentation only, so no confirmation dialog is proposed.

The coordinator belongs in `src/app/shell/ui_state_reset.rs`, with focused child modules only if responsibility/size requires them. Reuse sidebar/modal unmount helpers and normal sync/focus composition. Record the UI-state-reset term and the launch-state-clear exception in CONTEXT.md during implementation, without renaming existing terms. No ADR is needed for this reversible separation.

## Risks / Trade-offs

- Partial or old Audiobookshelf results could falsely remove selection or retain obsolete content --> stage catalog replacement and use existing bounded traversal plus narrowly scoped request identities.
- Full catalog replacement keeps old and new metadata briefly in memory --> temporary library-local buffers only, no retained second UI store; release on success/failure.
- Reset could silently overwrite volume or erase credentials because storage is shared --> patch explicit presentation keys and use a hermetic selective-persistence regression.
- Old navigation/restoration could undo reset --> clear pending UI intents before sync; leave ordinary domain/content completions eligible.
- Persistence can fail partway --> live reset remains applied, failures are visible, unrelated state is never used as rollback collateral.
- Another Client can later overwrite cleared launch state --> intentional existing last-completed-exit semantics, not a synchronization feature.

## Migration Plan

Keep keybind IDs and on-disk formats compatible. This changes F5's focus-sensitive behavior deliberately; document it in help/registry descriptions and issue #745. Ship the refresh separation first, prepare reset methods/persistence next, then expose the F2 action only after its global coordinator is complete. Apply the change's deltas to main specs and reconcile affected glossary text at completion. No remote-service migration is required. Reverting code restores prior commands, but deliberately forgotten UI locations cannot be recovered; protected domain state is never deleted.
