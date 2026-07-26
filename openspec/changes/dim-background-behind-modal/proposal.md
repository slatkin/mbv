## Why

Modal dialogs (confirm modal, save-playlist dialog, multiselect popup, library-routes popup) currently render on top of the app with no visual separation from the content behind them. The background stays at full brightness, so the modal's border is the only cue that input is now captured by the dialog rather than the underlying view. Dimming the background — as opencode does for its modals — makes the modal read clearly as a focused, blocking layer.

## What Changes

- Add a full-screen dim pass that darkens the already-rendered background cells immediately before any centered modal overlay is drawn on top.
- Apply the dim pass to every centered/blocking modal: confirm modal, save-playlist dialog, multiselect popup, and library-routes popup.
- Leave docked panels (sessions, playlists, help, settings) and the small anchored context menu unchanged — they are not blocking modals and are out of scope for this change.

## Capabilities

### New Capabilities
- `modal-backdrop-dim`: Defines the dimmed-backdrop treatment applied behind centered/blocking modal overlays.

### Modified Capabilities
(none — no existing spec covers modal rendering)

## Impact

- `src/app/render/overlays/confirm_modal.rs`
- `src/app/render/overlays/playlists.rs` (save-playlist dialog)
- `src/app/render/overlays/multiselect.rs`
- `src/app/render/overlays/library_routes.rs`
- `src/app/render/mod.rs` (render dispatch order, if a shared helper is introduced)
- No new dependencies; uses ratatui's existing `Buffer`/`Frame` APIs.
