## Why

Confirmation UX in mbv is inconsistent today: destructive/blocking questions (remove now-playing item, clear queue, rescan library) are rendered as one-line status-bar toast text ("Clear queue? (Y/n)"), while unsaved-playlist changes get a real centered modal. The one modal that exists (`render_dirty_playlist_modal` / the save-playlist overwrite dialog in `src/app/render/overlays/playlists.rs`) also predates the app's current overlay design language — it borders in `palette::YELLOW` where newer overlays (context menu, multiselect) use the rounded `palette::IRIS` style. Consolidating all confirmation-style prompts onto one shared, correctly-styled modal component removes visual inconsistency and gives future confirmations a single place to plug into instead of hand-rolling another toast string or another bespoke `Rect`/`Block` layout.

## What Changes

- Introduce a single shared confirmation-modal overlay (rendering + centered-rect layout + border/title/button-row styling) used by every yes/no confirmation in the app.
- Restyle the modal to match current overlay design language (rounded `IRIS`-bordered block, consistent with `context_menu.rs` / `multiselect.rs`), replacing the `YELLOW`-bordered look.
- Migrate the existing save-playlist confirmations (`render_dirty_playlist_modal`, `SavePlaylistStage::ConfirmOverwrite`) onto the shared component instead of their bespoke rendering code.
- Migrate the following status-bar toast confirmations to the shared modal:
  - Remove now-playing item from queue (`confirm_remove_idx`, `queue_actions.rs`)
  - Clear queue (`confirm_clear_queue`, `input_confirm_keys.rs`)
  - Rescan library (`confirm_rescan`, `input_lib_power_keys.rs`)
- Each migrated confirmation keeps its existing key bindings (y/Y/Enter to confirm, Esc/n to cancel) and existing side effects (system notification via `notify_with_actions` where already present); only the on-screen presentation moves from the status-bar toast line to the modal.
- **BREAKING** (internal only, no persisted state): `self.status = "...(Y/n)"` string-based prompts for the three toasts above are removed in favor of modal state; any code or test asserting on that status text must be updated to check modal state instead.

Out of scope (explicitly not migrated, left as-is):
- `confirm_logout`, which already renders inline inside the settings overlay rather than as a status-bar toast.
- Skip-intro and next-up prompts (`skip_intro_end_ticks`, `next_up_item`), which are timed, non-blocking, playback-progress-linked banners rather than blocking confirmations, and are intentionally left as toast-style so the user can keep browsing while they're visible.

## Capabilities

### New Capabilities
- `confirmation-modal`: a shared, reusable modal overlay for yes/no confirmation prompts — layout, styling, and the state contract (title, message, key bindings, confirm/cancel callbacks) that any call site can use to ask a blocking confirmation question.

### Modified Capabilities
(none — no existing `openspec/specs/` capabilities are defined yet for this app; this is the first spec-driven change for its overlay system)

## Impact

- Affected code:
  - `src/app/render/overlays/playlists.rs` (`render_dirty_playlist_modal`, `SavePlaylistStage::ConfirmOverwrite` rendering) — replaced with calls into the shared modal.
  - `src/app/queue_actions.rs`, `src/app/input_confirm_keys.rs`, `src/app/input_lib_power_keys.rs` — toast-string prompts replaced with modal-state prompts.
  - `src/app/render/mod.rs`, `src/app/render/chrome.rs` — toast/status-bar rendering path loses these three confirmation cases; a new render call for the shared modal is added to the overlay stack.
  - `src/app/input_resolver.rs` (`CONTEXT_STACK`) — confirmation key handlers may be consolidated under one modal-dispatch entry instead of three separate stack entries.
  - `src/app/app_struct.rs`, `src/app/construct.rs` — new shared modal state field(s) replacing/wrapping `confirm_clear_queue`, `confirm_remove_idx`, `confirm_rescan`.
  - New overlay module (e.g. `src/app/render/overlays/confirm_modal.rs`) for the shared rendering code.
- Affected tests: existing tests asserting on `self.status` text for clear-queue/remove-item/rescan confirmations (e.g. in `input_power_movie_detail_tests.rs`, `tests.rs`, `tests_queue_mutation.rs`) need updating to assert on the new modal state instead.
- No persisted data, network protocol, or external API is affected — this is purely in-process TUI state and rendering.
