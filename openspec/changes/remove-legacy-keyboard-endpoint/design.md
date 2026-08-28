## Context

See `proposal.md` for motivation. ADR 0002 requires first-match `Command` / `Swallow` / `FallThrough` keyboard precedence, and ADR 0022 requires TuiRealm focus and subscriptions to replace the old loop without changing that behavior.

The remaining endpoint is not mechanically dead:

- `key_policy.rs` is a static proof only; production never installs the subscriptions it describes.
- D15's `Component::perform(Cmd)` path was explicitly declined while `LegacyInput` and `CONTEXT_STACK` were live.
- Converted media components still emit `GlobalViewKey` for unmatched keys, so `Model::handle_legacy_key` and every `CONTEXT_STACK` handler remain reachable by construction.
- The 5.3d.22 deletion rows required zero references rather than authorizing replacement of the live route. They therefore documented a no-op instead of breaking the cycle.
- `handle_legacy_key` also re-pushes Home, Emby browser, Audiobookshelf, and Music presentation after every key. Removing it requires each typed effect path to push only the content it actually changes.
- D7 assigned one parent binding to `ComponentId::Library`, but no `LibraryComponent` is mounted. That table cannot be wired literally.
- The endpoint is reached by two **direct** `handle_key_with_home_context` call sites in `shell_home.rs` (the Home context-menu `.` path under Queue focus), not only through `handle_legacy_key`; and F1 Help-open is handled inside `handle_legacy_key` itself, outside `CONTEXT_STACK`.
- Several `CONTEXT_STACK` gates are not static booleans: `confirm_skip_intro`/`confirm_next_up` depend on ephemeral App fields (`skip_intro_end_ticks`/`next_up_item`), `playback` is a per-key command table plus a 300ms double-tap that falls through on the first press, and `queue_column_width` is gated on `PanelMode::Both` + Shift+Left/Right — none of which the shadow `KEY_POLICY` table captures faithfully.

This is a deeper ownership and precedence entanglement, not a Rust, TuiRealm, or event-conversion limitation. The temporary endpoint survived because the replacement execution path was deferred and the final task was framed as dead-code deletion.

## Goals / Non-Goals

**Goals:**

- Give every keyboard chord one live TuiRealm owner and preserve current precedence and effects.
- Keep local interpretation in the focused Interactive Component and send only semantic cross-boundary requests to the shell.
- Make the key policy executable through TuiRealm subscriptions rather than retaining a descriptive shadow table.
- Delete all production raw-key fallback and TuiRealm-to-Crossterm reconstruction.
- Keep implementation units reviewable despite the broad consumer fanout.

**Non-Goals:**

- Changing shortcuts, fixing input quirks, adding configurable keybindings, or adopting `CmdResult` redraw gating.
- Creating a placeholder `LibraryComponent`; issue #607 owns the separate Library-parent architecture discrepancy.
- Moving shell/runtime, Player, Service, persistence, or canonical Queue effects into Interactive Components.
- Restoring deferred mouse paths or changing rendering.

## Decisions

### 1. Use the existing `on(Event)` path; do not revive wholesale `perform(Cmd)` adoption

Focused components already interpret native TuiRealm keyboard events in `on`. They will continue to update local state there and emit typed `Msg` values only for shell-owned effects. Shared parent/global chords will be delivered by concrete TuiRealm subscriptions installed from the live key policy.

This removes the legacy path without changing all components to a second command-entry API. `perform` remains `NoChange`; `CmdResult` redraw work stays out of scope.

**Alternative considered:** adopt D15's `perform(Cmd)` design now that wholesale conversion is possible. Rejected because it would replace a working component input API across every component without adding behavior needed by #608.

### 2. Replace the shadow policy with executable TuiRealm routing

`key_policy.rs` will become the single installation point for parent/global keyboard subscriptions. It will register concrete chord clauses and real TuiRealm gates on existing owners; descriptive `Custom("...")` gates and tests that compare the policy to `CONTEXT_STACK` will be removed.

Ownership follows the smallest existing authority:

- A focused blocking overlay receives and swallows every key; lower subscriptions are gated off while it is mounted.
- `UiRoot` owns application-wide overlay opening, force-clear, refresh, Panel-mode cycling, tab switching, and quit requests.
- `Playback` owns visualizer and playback chords, including player/route eligibility and the existing Space/Escape double-tap state.
- `Queue` owns queue-width and clear-Queue chords plus Queue-local navigation and actions.
- The focused Library destination owns selection-dependent actions such as opening a context menu for its selected item and emits the explicit target in its request.

No fake `LibraryComponent` is introduced solely to satisfy the old table. This change may remove the stale `ComponentId::Library` policy entry, but it does not resolve the broader parent/ledger discrepancy tracked by #607.

The shared globals (`q`, Tab/BackTab, `1`–`9`, `.`) are the precedence crux. They are currently claimed by `handle_global_view_key` *ahead of* panel dispatch, so they are neither a parent binding nor a destination-local key. The `.` context-menu key is selection-dependent and must move to the focused destination, which emits the explicit target — including the Home-only Continue Watching special case whose target (`home_cw_selected`/`cw_item`) is resolved at the Model boundary and threaded through every `CONTEXT_STACK` handler signature today. The destination-independent Alt-key path (`handle_key_alt`: Alt+Left/Right panel-focus switch, Alt+Up/Down tab cycle, catch-all swallow) is a separate global that must be assigned to `UiRoot`.

Two gates cannot be expressed as static `SubClause`s and must become real state: the ephemeral prompt fields (`skip_intro_end_ticks`/`next_up_item`) must be mirrored into Playback-component attributes, and the playback transport gate is a per-key command table (`resolve_key`) plus a 300ms double-tap that returns `None` on the first press — so the subscription cannot simply claim Space/Escape.

**Alternative considered:** retain a shell pre-router for parent/global keys. Rejected because it would be the same parallel endpoint under a new name and would violate ADR 0022.

### 3. Raw keys stop at the component boundary

Every `ShellRequest` that carries a Crossterm `KeyEvent` will be replaced by the smallest semantic request set for that surface. Components decide accept/cancel/move/submit/dismiss locally; the shell performs only effects outside component authority. Existing target-bearing request types are reused. This includes not only the bare `*Key` forwards (`ConfirmKey`, `DaemonLostKey`, `RemoteReanchorKey`, `ContextMenuKey`, `FeedsManageKey`, `PlaybackPromptKey`, `SavePlaylistKey`, `QueueKey`, `GlobalViewKey`) but also the cursor-carrying `ServiceRequest::SettingsKey { cursor, key }` and `PersistRequest::SettingsKey { cursor, key }` variants, which are raw-key payloads under a different shape.

The conversion proceeds by behavior family while the old endpoint remains available only to not-yet-converted families. A family is complete only when its component has no `to_crossterm_key_event` call and emits neither `GlobalViewKey` nor another raw `*Key` request.

Shared key matching will use native TuiRealm key values. Framework-neutral action helpers may be retained only where multiple owners already share the same semantic command mapping; no new generic dispatcher is added.

**Alternative considered:** change raw request payloads from Crossterm keys to TuiRealm keys. Rejected because that renames the forwarding bridge instead of removing it.

### 4. Split the work by precedence/effect family, then delete globally

The consumer fanout is larger than one safe writer unit. Implementation will use serial, compile-complete families:

1. root/global policy and integration harness;
2. blocking overlays and dialogs;
3. Playback and playback prompts;
4. Queue;
5. Library destinations/media workspaces;
6. global deletion and architecture gates.

Each family reuses its existing shell effect entry points and replaces the blanket post-key presentation pushes with targeted pushes at that request's handler. The final deletion happens only after repository searches show no production raw-key request or adapter consumer.

**Alternative considered:** delete `CONTEXT_STACK` first and fix compile errors outward. Rejected because it obscures precedence regressions and creates one unreviewable cross-repository edit.

### 5. Preserve behavior with one production-style routing matrix

Existing direct `component.on(...)` tests remain useful for local interpretation, but they cannot prove focus/subscription behavior. Extend the current TuiRealm shell integration coverage with one table-driven `Application::tick()`-level matrix that catches realistic failures:

- a blocking overlay swallows an unbound/global key;
- a focused leaf falls through to exactly one eligible parent/global subscription;
- a locally claimed key does not also trigger a global effect;
- Queue and Library focus route their representative keys to the correct owner;
- playback gating and double-tap behavior remain unchanged.

Existing characterization tests will be repointed or removed when this integration matrix supersedes their legacy-loop assertions. The component boundary architecture gate will reject production Crossterm `KeyEvent` payloads/raw fallback variants under `src/app/components/`.

## Risks / Trade-offs

- **[A subscription and the active component both claim a chord]** → Use mutually exclusive runtime gates and the integration matrix to prove exactly one effect.
- **[A typed replacement loses a legacy side effect hidden in `handle_legacy_key`]** → Inventory each raw request's mutations and add only the corresponding targeted presentation push before removing that family.
- **[Broad fanout produces an unreviewable change]** → Land serial behavior families and run focused component/shell checks after each; keep the global deletion last.
- **[The missing Library parent blocks literal D7 ownership]** → Route only application-wide bindings through existing `UiRoot`; keep selection-dependent behavior in the focused destination and leave the separate Library-parent decision to #607.
- **[Static characterization tests pass while production routing is wrong]** → Require the production-style `Application::tick()` routing matrix before deleting the endpoint.
- **[A load-bearing precedence quirk is silently dropped]** → The `clear_queue_prompt_c` vs context-menu mutual exclusion (#135), the `Ctrl+a` enqueue-before-playback claim (#209), the `[`/`]` Queue-vs-Library meaning split, the `handle_lib_key` Ctrl/Alt catch-all swallow, the Space/Escape first-press fall-through, and the Ctrl+/ terminal-encoding ambiguity are each pinned by a routing-matrix row (task 1.3) before any family converts.
- **[The blanket re-projection masks a mutation]** → The five `push_*_content` calls in `handle_legacy_key` re-project Home, Emby browser, ABS podcast, ABS book, and Music workspace after *every* key; each family must replace only the pushes its handlers actually need (task 6.1), and the inventory (task 1.1) records which handler mutates which surface.

## Migration Plan

1. Record the exact raw-key producer/consumer and mutation matrix from current HEAD; treat it as the checklist for the serial families.
2. Land the TuiRealm routing integration harness and executable global policy without changing current observable behavior.
3. Convert each behavior family to native component interpretation and semantic requests, deleting its raw forwarding and blanket re-projection as it reaches zero consumers.
4. Delete `GlobalViewKey`, remaining raw `*Key` requests, `typed_key.rs`, `Model::handle_legacy_key`, `App::handle_key_with_home_context`, `CONTEXT_STACK`, obsolete handlers, and static-only policy scaffolding.
5. Run formatting, package checks/tests, Clippy, architecture gates, and the code-size gate; verify searches return no production legacy keyboard endpoint.

All steps are internal to one development branch. Before the final deletion, rollback is a normal commit revert of the latest family. After deletion, rollback is the revert of the complete change so the old and new authorities are not mixed.
