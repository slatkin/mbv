## Why

The TuiRealm migration left one live keyboard route back through `App`, and it
is not removable as dead code. TuiRealm's `Application::tick` forwards every
event to the focused component **and** to every satisfied subscription, with no
consumed signal and no priority among subscriptions. ADR 0002's ordered,
first-match `Command` / `Swallow` / `FallThrough` model therefore has no
representation in the framework's delivery, and `GlobalViewKey` +
`Model::handle_legacy_key` are standing in for the missing ordering relation.

Task `5.3d.22` framed the cleanup as "delete when unreferenced" while converted
components still emitted `GlobalViewKey`, so it recorded a no-op; D15 declined
`perform(Cmd)` and left `key_policy.rs` a `#![allow(dead_code)]` shadow table
with no execution path. Both are consequences of the same missing relation, not
independent oversights.

ADR 0023 supplies it: one central Keyboard Router in `UiRoot`.

## What Changes

- Make `UiRoot` the single Keyboard Router (ADR 0023). It resolves every chord
  against ordered policy and returns `Command` / `Swallow` / `FallThrough`;
  `FallThrough` lets the focused leaf's own typed request stand. `key_policy.rs`
  becomes that live policy instead of a descriptive table.
- Stop leaves interpreting global chords. A leaf interprets only what its own
  surface means, emits a typed semantic request, and returns `None` otherwise —
  it never forwards, wraps, or re-emits a key.
- Replace every raw-key `ShellRequest` with the smallest semantic request set
  for that surface, including the cursor-carrying
  `ServiceRequest::SettingsKey` / `PersistRequest::SettingsKey`.
- Remove the TUI skip-intro and next-up prompts entirely; mpv's on-screen
  buttons become their sole interface. This deletes `PlaybackPromptComponent`,
  `App.skip_intro_end_ticks`, the `status`-as-prompt writes, and the
  notification-action input path for those two decisions.
- Remove `Model::handle_legacy_key`, `App::handle_key_with_home_context`,
  `CONTEXT_STACK`, the per-surface context handlers, `GlobalViewKey`, the raw
  `*Key` requests, and `typed_key.rs`.
- Preserve every other shortcut, modifier gate, double-tap behavior, modal
  blocking, focus behavior, and effect target. Apart from the removed prompts,
  this is an ownership and routing change, not a keybinding redesign.
- Add a production-style `Application::tick()` routing matrix and structural
  gates proving no legacy raw-key fallback remains.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `toast-notification-semantics` — its severity model carves out "interactive
  prompts (next-up, skip-intro, clear-queue confirmation)" as non-toasts using
  standard status-bar styling. Two of those three no longer exist in the TUI.
  The delta narrows both requirements to the clear-queue confirmation, which is
  a real modal.

This change otherwise makes the implementation conform to the
`interactive-component-framework` requirements introduced by
`migrate-tui-to-tuirealm`; that contract is unchanged and not restated here.

## Impact

- **New ADR 0023** (One Central Keyboard Router) records the routing decision
  and the rules a future change must not violate.
- **New** `docs/architecture/mpv-owned-playback-prompts.md` records the prompt
  removal, what stayed and why, the remote-daemon gap, and the conditions any
  re-added TUI affordance must satisfy.
- Affects `src/app/components/` (all 16 `to_crossterm_key_event` call sites),
  `key_policy.rs`, shell message handling, and the legacy input resolver and
  handlers under `src/app/input*.rs`.
- Removes internal raw-key message variants and adapters. No external API,
  protocol, configuration, dependency, daemon, Local-daemon, Service, or
  persistence behavior changes. The only user-visible behavior change is the
  removal of the TUI skip-intro/next-up prompts and their `Y`/`n` keys.
- `App.next_up_item` is retained — `PlayerEvent::NextUpPlay` reads it to resolve
  the `JumpTo` index when the user clicks mpv's button. It is player state, not
  prompt state.
