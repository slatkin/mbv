## 1. Shared modal component

- [x] 1.1 Add `ConfirmAction` enum and `ConfirmModal { title, message, hint, on_confirm }` struct (new small module, e.g. `src/app/types_confirm.rs`, per the `types_` naming convention).
- [x] 1.2 Add `confirm_modal: Option<ConfirmModal>` field to `App` (`app_struct.rs`) and initialize to `None` in `construct.rs`.
- [x] 1.3 Create `src/app/render/overlays/confirm_modal.rs` with the shared render function: centered `Rect` sized to content, `Clear`, `Block` with `BorderType::Rounded` + `palette::IRIS` border, title/message/hint layout matching `render_dirty_playlist_modal`'s existing spacing pattern.
- [x] 1.4 Wire the new render function into the overlay stack in `render/mod.rs` (rendered when `self.confirm_modal.is_some()`).
- [x] 1.5 Add one `CONTEXT_STACK` entry (`input_resolver.rs`) dispatching to a new `handle_key_confirm_modal` that matches on `self.confirm_modal`'s `on_confirm` and calls each action's existing effect function on `y`/`Y`/`Enter`, clearing `confirm_modal` either way.

## 2. Migrate clear-queue confirmation

- [x] 2.1 In `input_confirm_keys.rs`, replace `confirm_clear_queue: bool` trigger/consume with setting/clearing `confirm_modal` (`ConfirmAction::ClearQueue`); remove the `self.status = "Clear queue? (Y/n)"` line.
- [x] 2.2 Remove `confirm_clear_queue` field and its old `CONTEXT_STACK` entry (`handle_key_confirm_clear_queue`) once folded into 1.5.
- [x] 2.3 Update tests referencing `confirm_clear_queue` / the old status text to assert on `confirm_modal` instead.

## 3. Migrate remove-now-playing-item confirmation

- [x] 3.1 In `queue_actions.rs`, replace `confirm_remove_idx: Option<usize>` trigger with `confirm_modal` (`ConfirmAction::RemoveActiveQueueItem(pos)`); remove the `self.status = "Remove now-playing item and stop playback? (y/N)"` line.
- [x] 3.2 Update the consuming handler in `input_queue_keys.rs` (currently keyed on `self.confirm_remove_idx`) to read from `confirm_modal` via the shared dispatch from 1.5, or fold its effect into the shared handler directly.
- [x] 3.3 Remove `confirm_remove_idx` field once migrated.
- [x] 3.4 Update tests referencing `confirm_remove_idx` / the old status text.

## 4. Migrate rescan-library confirmation

- [x] 4.1 In `input_lib_power_keys.rs`, replace `confirm_rescan: bool` + `pending_rescan_lib_idx` trigger with `confirm_modal` (`ConfirmAction::RescanLibrary(lib_idx)`), folding the library index directly into the action instead of a separate pending field; remove the `self.status = format!("Rescan '{name}'? (Y/n)")` line.
- [x] 4.2 Remove `confirm_rescan` / `pending_rescan_lib_idx` fields and the old `CONTEXT_STACK` entry (`handle_key_confirm_rescan`) once folded into 1.5.
- [x] 4.3 Update tests referencing `confirm_rescan` / the old status text.

## 5. Migrate save-playlist confirmations

- [x] 5.1 Replace `render_dirty_playlist_modal`'s bespoke rendering in `playlists.rs` with a call that populates `confirm_modal` (`ConfirmAction::DiscardOrSaveDirtyPlaylist`) and renders via the shared component; preserve the `[s]Save [d]Discard [Esc]Cancel` hint and behavior.
- [x] 5.2 Replace the `SavePlaylistStage::ConfirmOverwrite` bespoke rendering with `confirm_modal` (`ConfirmAction::SaveOverwritePlaylist { existing_id }`), preserving the `y`/`Esc` behavior.
- [x] 5.3 Delete the now-unused bespoke rendering code (`render_dirty_playlist_modal` body, `ConfirmOverwrite` render branch) once both are migrated.
- [x] 5.4 Update playlist-flow tests to match the new modal state shape.

## 6. Cleanup and verification

- [x] 6.1 Confirm `toast_line`/status-bar rendering in `render/chrome.rs` no longer needs to special-case confirmation text (it should now only ever show transient info/error/skip-intro/next-up toasts).
- [x] 6.2 Run `cargo fmt --all -- --check`.
- [x] 6.3 Run `cargo test` (or the narrowest relevant subset covering `input_confirm_keys`, `queue_actions`, `input_lib_power_keys`, `playlists` overlay, and `input_resolver_handle_key_tests`).
- [ ] 6.4 Manually verify each of the four migrated confirmations in a live TUI session: clear queue, remove now-playing item, rescan library, unsaved-playlist-changes / overwrite-playlist — confirm visuals match the `IRIS`-bordered style and keybindings/effects are unchanged.

## 7. Code-review follow-up

A code-reviewer pass on the implementation (1-6 above) surfaced these findings. The
HIGH item was fixed inline; the rest are intentionally parked (not blocking) per
explicit user decision to fix only the HIGH finding now:

- [x] 7.1 **(HIGH, fixed)** `handle_key_save_playlist_entry` (`input_playlist_keys.rs`) swallowed all
  keyboard input with no modal rendered if `confirm_modal` was cleared/replaced while
  `save_playlist_dialog` was stuck at `ConfirmOverwrite` (reachable via a mouse click
  that fires `remove_from_queue`/`replace_queue_or_prompt` while the overwrite prompt
  is up) — an unrecoverable soft-lock. Fixed by gating the guard on
  `SavePlaylistStage::EnterName` specifically so a stranded `ConfirmOverwrite` falls
  through (`return None`) instead of swallowing input.
- [ ] 7.2 **(MEDIUM, parked)** Remove-item confirmation (`input_confirm_keys.rs`) only accepts
  lowercase `y`, not `y`/`Y`/`Enter` as `specs/confirmation-modal/spec.md` states — spec
  and code contradict each other. Needs an author ruling: widen the code to match the
  spec, or amend the spec scenario to say `y` only (original toast was `(y/N)`,
  default-no, since the effect is destructive).
- [ ] 7.3 **(MEDIUM, parked)** `RemoveActiveQueueItem`'s confirmation was promoted from the very
  bottom of key dispatch to `CONTEXT_STACK` rank 0 (`input_resolver.rs`), so it now
  swallows keys that used to pass through while the prompt was up (Space/pause, seek,
  F5 refresh, Ctrl+L, skip-intro/next-up banners). Likely the correct semantics for a
  blocking modal, but design.md's Decision 2 never analyzed this case (only the three
  existing `CONTEXT_STACK` entries). Needs either a design.md note accepting this as
  intended, or a regression test pinning the desired behavior.
- [ ] 7.4 **(MEDIUM, parked)** No dispatch tests cover `ConfirmAction::RemoveActiveQueueItem`,
  `SaveOverwritePlaylist`, or `DiscardOrSaveDirtyPlaylist` — the overwrite path (the
  most-refactored one, carrying 7.1's bug) has zero coverage. Add tests mirroring the
  existing `ClearQueue` dispatch-test pattern in `input_resolver_handle_key_tests.rs`.
- [ ] 7.5 **(LOW, parked)** `do_overwrite_playlist` (`input_playlist_keys.rs`) silently no-ops if
  the dialog's name is gone by confirm time; should surface a status message instead.
- [ ] 7.6 **(LOW, parked)** Modal hint text reads "[Esc] Cancel" for `ClearQueue`/`RescanLibrary`/
  `RemoveActiveQueueItem`, but any key actually cancels (the non-confirm arm is `_`).
  Either reword the hint or narrow the cancel arm to `Esc`/`n`/`N`.
- [ ] 7.7 **(LOW, parked)** Rescan confirmation message (`input_lib_power_keys.rs`) isn't
  `trunc_str`-truncated like the other three interpolated messages, so a long library
  name can overflow the modal's fixed width.
- [ ] 7.8 **(LOW, parked)** Hint line in `confirm_modal.rs` can render outside the bordered block
  on terminals shorter than ~6 rows; clamp `base_y + 2` or skip the hint when
  `inner.height < 3`.
- [ ] 7.9 **(LOW, parked)** Wording drift: modal says "Clear the queue?" while the desktop
  notification two lines above still says "Clear queue?" — align the wording.
