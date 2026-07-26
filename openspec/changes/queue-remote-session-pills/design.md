## Context

The power queue title currently renders scope pills only for a direct mbv remote queue. Its local and remote halves are interactive, and the active remote half currently uses `QUEUE_BUTTON_FOCUSED_BG` as foreground on `YELLOW`. A separately attached session is represented in the application state and status chrome, but is not represented by a queue-header scope pill.

The remote state model already distinguishes `AttachedSession` from `DirectRemote`. The attached-session status renderer also centralizes the remote icon and label resolution: use `connected_session_state.device_name` when available, then fall back to the host, matching the existing remote status behavior. The queue layout and input code must preserve the existing direct-remote interactions while ensuring the new attached-session pill cannot accidentally become switchable.

## Goals / Non-Goals

**Goals:**

- Render the active direct mbv remote pill with foreground `#dbbc7f` (the existing `YELLOW`) and background `#35a77c` (the existing `AQUA`).
- Render an attached non-mbv session as a right-side pill with the existing remote icon, the existing device-name-then-host label fallback, foreground `#1e2326` (`QUEUE_BUTTON_FOCUSED_BG`), and background `#dbbc7f` (`YELLOW`).
- Keep direct mbv local/remote pills interactive and preserve their existing hitboxes, selection, and queue behavior.
- Make the attached-session pill display-only: it has no queue-scope action, no keyboard selection/focus path, and mouse clicks are ignored.
- Keep layout widths and clipping deterministic for long labels and narrow terminals.

**Non-Goals:**

- No changes to remote connection protocols, session persistence, route selection, or attached-session queue semantics.
- No changes to the chrome status pill itself unless a shared helper is needed to guarantee icon and label consistency.
- No new color constants or dependencies when the existing palette and rendering utilities express the required colors.

## Decisions

### 1. Branch rendering by remote state, not by individual connection fields

Use the existing remote-slot classification so the queue title has two mutually exclusive modes: `DirectRemote` renders the current local/remote split, and `AttachedSession` renders the local context as before plus one right-side display-only remote pill. `LocalDaemon` and `Off` retain the current local-only title. This prevents an attached session from being mistaken for a direct remote queue merely because both have connection-related fields populated.

Alternative considered: infer the attached-session mode directly from `connected_session_id` in the title renderer. Rejected because the state enum already encodes the distinction and centralizing the branch avoids divergence from remote-slot behavior.

### 2. Reuse the existing remote icon and host-label resolution

Extract or reuse the existing small rendering data path used by `remote_status_spans`: the icon remains the same, and an attached session label is `connected_session_state.device_name` when non-empty, otherwise its host. The queue pill must not invent a second label format or use a route label intended for direct remotes.

Alternative considered: display a fixed `Emby` label. Rejected because it loses the useful host identity requested by the feature and differs from the established status chrome.

### 3. Keep display-only attached pills outside interactive scope hitboxes

The attached pill may occupy the right-side queue-header layout area, but `queue_scope_remote_area` and any keyboard/action mapping used to switch between local and direct remote scopes must remain associated only with `DirectRemote`. Attached-session rendering must not create a selectable remote target. Input tests will assert no-op behavior for both mouse and keyboard/action dispatch.

Alternative considered: reuse the remote hitbox and dispatch a no-op action. Rejected because a hitbox would falsely communicate interactivity and risks future code turning the no-op into a connection switch.

### 4. Use the existing palette values exactly

The direct remote active state uses `YELLOW` foreground on `AQUA` background. The attached display state uses `QUEUE_BUTTON_FOCUSED_BG` foreground (`#1e2326`) on `YELLOW` background. Inactive direct remote styling remains unchanged. Tests should inspect rendered spans/styles rather than only screen glyphs so the exact foreground/background contract is pinned.

## Risks / Trade-offs

- [A long attached host can consume the title's right-side width] → Apply the same width calculation, truncation, and minimum-width behavior already used for remote labels; test narrow areas and long labels without allowing the pill to overlap the queue title or direct scope controls.
- [Shared icon/label logic could subtly change existing chrome status output] → Prefer a read-only helper or reuse path with tests covering existing `remote_status_spans` behavior; do not alter label precedence or icon glyph.
- [An attached pill could accidentally enter direct-remote navigation] → Keep its layout area out of scope hitbox registration and add mouse plus keyboard/action no-op tests.
- [Remote-state precedence could regress when fields overlap] → Add state-matrix tests proving `AttachedSession` takes the attached display path, `DirectRemote` retains the split, and local/off states remain unchanged.

## Migration Plan

1. Update queue title rendering and any shared remote-pill data/helper code.
2. Update layout allocation and input dispatch only where needed to keep attached display space separate from interactive direct-remote scope areas.
3. Add rendering, label/icon, layout, and input regression tests, then run formatting and targeted Rust tests.
4. No data migration or rollback procedure is required; reverting the code restores the prior rendering behavior.

## Open Questions

None. The supplied requirements define the state distinction, exact colors, no-op interaction, label fallback, icon reuse, and preserved attached-session behavior.
