## 1. Remote state and shared pill data

- [x] 1.1 Confirm the queue-header branch uses `remote_slot_state` classification so `DirectRemote`, `AttachedSession`, `LocalDaemon`, and `Off` cannot be conflated by overlapping connection fields.
- [x] 1.2 Reuse or extract the existing remote-status icon and attached-session label resolution from `remote_status_spans`: prefer `connected_session_state.device_name` when non-empty and fall back to the session host, without changing the established chrome output.
- [x] 1.3 Preserve the existing direct-remote route/label resolution and current attached-session queue behavior while wiring the shared pill data into queue-header rendering.

## 2. Queue header rendering and layout

- [x] 2.1 Update `App::render_power_queue_title` in `src/app/render/queue.rs` so a direct mbv remote still renders the local/remote interactive split and the active remote span uses `YELLOW` foreground on `AQUA` background.
- [x] 2.2 Add the attached-session branch to render one right-side display-only pill with the reused remote icon and resolved device/host label, using `QUEUE_BUTTON_FOCUSED_BG` (`#1e2326`) foreground on `YELLOW` (`#dbbc7f`) background.
- [x] 2.3 Keep direct-remote inactive styling, local/remote order, labels, and selection semantics unchanged; ensure local-only and disconnected states retain their existing queue-header output.
- [x] 2.4 Update `LayoutMain` queue-scope area calculation and width/truncation handling as needed so the attached pill fits on the right without overlapping the title or direct-remote controls, including narrow-terminal and long-host cases.
- [x] 2.5 Ensure the attached-session display pill does not populate `queue_scope_local_area` or `queue_scope_remote_area` as an interactive target, while direct-remote areas remain unchanged.

## 3. Input behavior

- [ ] 3.1 Update mouse dispatch in `input_mouse_dispatch.rs` so clicks in the attached-session pill area are ignored and do not alter queue scope, route, connection state, or attached-session queue behavior.
- [ ] 3.2 Verify keyboard focus/navigation and queue-scope actions expose only the existing direct-remote controls; an attached-session pill cannot be focused, selected, or switched to and all attempted actions are no-ops.
- [ ] 3.3 Preserve and regression-test direct-remote local and remote mouse hitboxes plus keyboard/action selection after the attached display layout is added.

## 4. Tests

- [ ] 4.1 Extend `src/app/render/tests_queue.rs` or its queue-title test coverage to assert exact active direct-remote colors (`YELLOW` on `AQUA`), unchanged inactive styling, and the distinct attached-session pill colors (`#1e2326` on `YELLOW`).
- [ ] 4.2 Add attached-session rendering tests for the existing remote icon, non-empty device-name label, host fallback when device name is absent/empty, right-side placement, and the state-matrix distinction from `DirectRemote`.
- [ ] 4.3 Add layout tests covering sufficient width, long-label clipping/truncation, narrow terminals, no overlap with the title, and no interactive attached hitbox; verify local-only layout is unchanged.
- [ ] 4.4 Extend `src/app/tests_queue_scope.rs` and `input_mouse_dispatch.rs` tests to prove attached-pill mouse clicks and keyboard/action attempts are no-ops while direct-remote local/remote controls remain interactive.
- [ ] 4.5 Keep `src/app/render/tests.rs` remote-status tests green and add a shared icon/label regression assertion if helper extraction changes the status rendering path.

## 5. Verification

- [ ] 5.1 Run `cargo fmt --all -- --check`.
- [ ] 5.2 Run the narrowest relevant Rust tests covering queue rendering, queue scope, remote status, layout, and mouse/keyboard dispatch, including the new exact-color and no-op cases.
- [ ] 5.3 Run the full project test suite if targeted tests pass, and manually verify direct mbv-to-mbv active/inactive pills and attached mbv-to-emby display-only pill behavior in a live TUI session.
