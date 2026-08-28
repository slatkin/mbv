## Why

The TuiRealm migration left one live keyboard route back through `App`: D15 explicitly chose static precedence proofs instead of wiring the replacement policy while the legacy bridge was active, then the teardown rows required handlers to be unreferenced even though converted components still emitted `GlobalViewKey`. This circular gate made the cleanup a documented no-op, leaving the migration short of ADR 0022 and issue #607's completion contract.

## What Changes

- Replace component-to-shell raw key forwarding with component-local TuiRealm key interpretation and typed semantic requests for shell-owned effects.
- Make the preserved ADR 0002 precedence behavior execute through TuiRealm focus and guarded subscriptions, including blocking-overlay swallow and parent/global bindings.
- Remove `Model::handle_legacy_key`, `App::handle_key_with_home_context`, `CONTEXT_STACK`, obsolete per-surface context handlers, `GlobalViewKey`, raw `*Key` shell requests (including the cursor-carrying `ServiceRequest::SettingsKey`/`PersistRequest::SettingsKey`), and the TuiRealm-to-Crossterm reconstruction adapter.
- Resolve the shared-globals crux (`q`, Tab/BackTab, `1`–`9`, `.`) and the destination-independent Alt-key path; own the global playback keys (Space/Escape double-tap) by one global handler that dispatches the existing typed first-press leaf request by focus (no per-screen playback-timer mirror). The skip-intro/next-up prompts are already a focused modal, so no attribute mirror is required for them.
- Preserve all existing shortcuts, modifier gates, double-tap behavior, modal blocking, focus behavior, and effect targets; this is an ownership/routing change, not a keybinding redesign.
- Add production-style TuiRealm integration coverage and structural gates proving that no legacy raw-key fallback remains.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- None. This change makes the implementation conform to the existing `interactive-component-framework` requirements introduced by `migrate-tui-to-tuirealm`; it preserves externally observable keyboard behavior, so `.openspec.yaml` declares `skip_specs: true` rather than duplicating that contract.

## Impact

- Affects `src/app/components/`, shell message handling, the legacy input resolver/handlers under `src/app/input*.rs` (including the `handle_key_alt`, `handle_global_view_key`, `handle_key_emby_library`, and `handle_lib_key` precedence branches), key-policy wiring, and keyboard integration/architecture tests. (The `handle_key_with_home_context` call sites in `shell_home.rs` are `#[cfg(test)]`-only, not production bypasses.)
- Removes internal raw-key message variants and adapters; no external API, protocol, configuration, dependency, daemon, Local-daemon, Service, playback, or persistence behavior changes.
