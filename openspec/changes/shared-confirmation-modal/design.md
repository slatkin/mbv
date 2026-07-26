## Context

mbv is a ratatui TUI. Overlays are rendered by `App` methods under `src/app/render/overlays/` and dispatched into via `render/mod.rs`; overlay-specific key handling is dispatched through the `CONTEXT_STACK` table in `src/app/input_resolver.rs`, where each stack entry is a `(name, guard/handler)` pair checked in priority order per keypress.

Today there is exactly one real modal: the save-playlist flow in `src/app/render/overlays/playlists.rs` (`render_dirty_playlist_modal` for unsaved changes, plus the `SavePlaylistStage::ConfirmOverwrite` dialog state). It centers a fixed `56x7` `Rect`, clears it, and draws a `Block` with rounded borders in `palette::YELLOW`.

Three other confirmations exist only as status-bar toast text (`self.status = "...(Y/n)"`, rendered by `toast_line` in `render/chrome.rs`):
- `confirm_clear_queue` (`input_confirm_keys.rs`)
- `confirm_remove_idx` (`queue_actions.rs`)
- `confirm_rescan` (`input_lib_power_keys.rs`)

Newer overlays (`context_menu.rs`, `multiselect.rs`) already use a rounded block bordered in `palette::IRIS`, which is the current design language the restyled modal should match.

## Goals / Non-Goals

**Goals:**
- One shared overlay component (render + centered-`Rect` layout + border/title/message/button-row styling) used by all confirmation prompts.
- Visual restyle to the `IRIS`-bordered rounded-block look already used by `context_menu.rs`/`multiselect.rs`.
- Migrate clear-queue, remove-now-playing-item, and rescan-library confirmations off the status-bar toast and onto the shared modal, preserving their existing key bindings and side effects.
- Migrate the existing save-playlist modal/dialog onto the same shared component (same visuals, same component, different call site).
- Keep the change additive to state shape where reasonable — avoid rewriting unrelated save-playlist business logic (renaming, overwrite id tracking, etc.), only its presentation.

**Non-Goals:**
- Not migrating `confirm_logout` (already inline in the settings overlay, not a toast) or the timed skip-intro/next-up banners (intentionally non-blocking; see proposal's "Out of scope").
- Not building a generic "toast queue" or animation system — this is a single-modal-at-a-time confirmation primitive, matching how `context_menu`/`multiselect` already behave (one overlay active at a time).
- Not changing keybindings, wording, or side effects (system notifications, bell) of the migrated confirmations beyond where they render.

## Decisions

### 1. One `ConfirmModal` state struct + one render function
Add a single `Option<ConfirmModal>` field on `App` (e.g. `pub(super) confirm_modal: Option<ConfirmModal>`) holding `{ title: String, message: String, hint: String, on_confirm: ConfirmAction }`, where `ConfirmAction` is a small enum (`ClearQueue`, `RemoveActiveQueueItem(usize)`, `RescanLibrary(usize)`, `SaveOverwritePlaylist { existing_id: String }`, `DiscardOrSaveDirtyPlaylist`, ...) identifying what "yes" does. This replaces `confirm_clear_queue: bool`, `confirm_remove_idx: Option<usize>`, `confirm_rescan: bool` as booleans/options scattered across the struct, and gives the save-playlist flow the same shape the other three already need.

Alternative considered: keep each confirmation's own bool/option field and only share the *rendering* function (pass title/message/hint as args from each call site, keyed off whichever field is `Some`/`true`). Rejected because it still requires the render code and the input-dispatch code to check N separate fields in priority order (as `CONTEXT_STACK` does today for the 3 toast confirmations) — a single `Option<ConfirmModal>` collapses that to one check and one enum match, which is less code and removes the risk of two confirmations racing to be shown at once.

### 2. Single `CONTEXT_STACK` entry replacing three
Replace the `confirm_clear_queue`, `confirm_rescan` (and the queue-remove and save-playlist dialog checks, wherever they currently sit) stack entries with one `confirm_modal` entry near the top of `CONTEXT_STACK` (modals should out-rank most other input, matching where the save-playlist dialog and the toast confirmations are already prioritized). The handler matches on `self.confirm_modal.as_ref().map(|m| &m.on_confirm)` and re-uses each action's existing effect code (`replace_queue_or_prompt`, `trigger_lib_rescan`, etc.) — only the trigger/dismiss glue changes, not the underlying effects.

Alternative considered: keep 3+ stack entries but have them all delegate to shared render/layout helpers. Rejected — doesn't remove the duplicated bool-juggling this change is meant to clean up, and leaves open the same "two confirmations shown at once" hazard noted above.

### 3. Visual style: reuse `palette::IRIS` rounded block, drop `palette::YELLOW`
The shared modal borrows the exact block styling already used by `context_menu.rs`/`multiselect.rs` (`BorderType::Rounded`, `border_style(Style::default().fg(palette::IRIS))`), sized to fit the longest of the migrated messages (a bit wider than the current `56x7` to comfortably fit rescan's library-name interpolation and the save-playlist filename line). Title text becomes the confirmation's short label (e.g. `" Clear Queue "`, `" Remove Item "`, `" Rescan Library "`, `" Unsaved Playlist Changes "`) styled in `palette::TEXT`/`WHITE` rather than the current yellow-bold title, consistent with how `context_menu` titles are styled.

Alternative considered: introduce a brand-new color pairing distinct from both `YELLOW` and `IRIS`. Rejected — the ask is specifically to stop looking like "old design language," and `IRIS` is already the established modern-overlay color; inventing a third style would add inconsistency rather than remove it.

### 4. Button/hint row stays plain text, not interactive widgets
Keep the confirm/cancel affordance as a single styled hint line (e.g. `"[y] Confirm    [Esc] Cancel"`), matching the existing pattern (`"[s]Save  [d]Discard  [Esc]Cancel"`) rather than introducing focusable button widgets. This is a keyboard-only TUI; a text hint row costs far less than building/testing focus-traversal for two-to-three "buttons," and no interactive-button behavior was requested.

## Risks / Trade-offs

- [Collapsing 3 bools into one `Option<ConfirmModal>` touches every call site and every test asserting the old fields/status text] → Mitigate by grepping all read/write sites before renaming (already inventoried in the proposal's Impact section) and updating tests in the same commits that change behavior, not as a follow-up.
- [`CONTEXT_STACK` re-ordering could change priority relative to other handlers if the new single entry isn't placed at the same rank as the highest-priority one it replaces] → Mitigate by placing the new entry at the topmost of the three replaced ranks and running the existing `input_resolver_handle_key_tests.rs` suite to catch ordering regressions.
- [Widening the modal rect to fit rescan/save-playlist text could visually clip on very small terminal sizes] → Mitigate by clamping width/height to `f.area()` the same way `render_dirty_playlist_modal` already does (`full.width.saturating_sub(w)`).
- [Removing `self.status = "...(Y/n)"` toast text changes what `render/tests.rs`/`tests_queue_mutation.rs` assert on] → Mitigate by updating those assertions to check `self.confirm_modal` instead, in the same change (no dangling references to removed fields).

## Migration Plan

1. Add `ConfirmModal` struct + `ConfirmAction` enum + `confirm_modal: Option<ConfirmModal>` field; keep old fields temporarily unused-but-present is not necessary — remove them in the same step they're replaced, per call site, to avoid dead code lingering mid-migration.
2. Build the shared render function (new module, e.g. `src/app/render/overlays/confirm_modal.rs`) styled per Decision 3, wired into `render/mod.rs`'s overlay stack.
3. Migrate one confirmation at a time (suggest order: clear-queue → remove-item → rescan → save-playlist), each as its own commit: swap the field, update the trigger site, update `CONTEXT_STACK`, update tests.
4. Delete `render_dirty_playlist_modal`'s bespoke rendering and `toast_line`'s role in showing confirmation text once all four are migrated.
5. No feature flag / rollback plan needed beyond normal git revert — this is presentation-layer-only with no persisted state or external contract.

## Open Questions

- Exact modal width/height and message wording per confirmation are left to implementation (tasks.md) rather than pinned here, since they're cosmetic details, not architectural decisions.
- Whether `confirm_logout` should eventually move onto this same component (it's currently inline in settings, not a toast) is left for a future change — flagged here so it isn't forgotten, not to be done in this change.
