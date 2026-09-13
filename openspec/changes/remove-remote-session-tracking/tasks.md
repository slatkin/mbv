## 1. Preserve the Attached-Session Boundary

- [ ] 1.1 Simplify attached generic Emby Session sequence submission and transport dispatch so they create no tracker, expected transition, epoch, or correlated reconciliation result; amend the existing attached-session command tests to verify multi-item play, pause/play, seek, stop, next, previous, and direct selection still dispatch and report command errors, then run `cargo nextest run -p mbv` for those test modules.

## 2. Remove Observer-Driven Queue Mutation

- [ ] 2.1 Remove poll-time occurrence observation, completion consume, queue-slot projection, queue lineage used only by Tracking, content-ID cursor movement, and the previous-item watched/progress refresh event; amend one existing Session integration test to prove item changes update `connected_session_state` while queue slots, cursor, dirty state, and playlist mutation state stay unchanged, then run its focused `cargo nextest run -p mbv` filter.
- [ ] 2.2 Remove Tracking-specific retirement calls, queue-edit confirmation and pending-action branches while preserving now-playing removal and unsaved-playlist safeguards; delete superseded reconciliation and mutation tests, then run the focused queue mutation, playlist save, Session lifecycle, and queue-scope tests with `cargo nextest run -p mbv`.

## 3. Remove Tracking Presentation and Interaction

- [ ] 3.1 Delete Tracking labels and reasons from the Queue panel and the Tracking marker from the Sessions sidebar, preserving ordinary connected-target and directly observed playback presentation; update or delete existing render characterizations and verify relevant Narrow and Wide buffer tests with `cargo nextest run -p mbv`.
- [ ] 3.2 Delete Stop Tracking and re-anchor intents, popup Interactive Component, mount/focus registration, router/modal policy entries, shell dispatch, and tests without adding fallback routing; run focused component tests plus the existing `Application::tick()` overlay/routing integration tests with `cargo nextest run -p mbv`.

## 4. Delete the Reconciliation Model

- [ ] 4.1 Remove remaining App tracker/projection fields, reconciliation-only event and overlay types, constructors, exports, helper methods, and dedicated application tests; verify Session watch, Session switching/disappearance, Direct remote control, and local/Local daemon queue tests with `cargo nextest run -p mbv`.
- [ ] 4.2 Delete `mbv-core`'s remote reconciliation modules and dedicated tests after callers are gone, then run `cargo nextest run -p mbv-core` and `cargo check -p mbv-core`.

## 5. Align Documentation and Verify the Deletion

- [ ] 5.1 Update `CONTEXT.md` to remove Tracking-specific terms, make Session watch explicitly read-only with respect to mbv queue state, and limit Consume to authoritative Player-owner lifecycle; verify exact references to removed Tracking and re-anchor concepts remain only in archived history and this change.
- [ ] 5.2 Run `cargo fmt`, `cargo nextest run -p mbv`, `cargo nextest run -p mbv-core`, `cargo clippy --workspace --all-targets -- -D warnings`, and `openspec validate remove-remote-session-tracking --strict`; confirm local and Player-owner progress/consume code was not changed and the worktree contains only the planned deletion and documentation/spec updates.
