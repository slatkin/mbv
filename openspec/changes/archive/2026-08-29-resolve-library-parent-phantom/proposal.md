## Why

`ComponentId::Library` names a `LibraryComponent` (`src/app/components/library.rs`)
that was never implemented or mounted — the migration landed Library-parent
routing as pure derivation over `App.tab` (`shell_library.rs::sync_active_destination`
+ `library_child_id`), which is the better outcome, but left the enum variant,
one `key_policy.rs` owner tag, and a "Library parent | migrated" ledger row
(`docs/architecture/interactive-surface-ledger.md:64`) pointing at the phantom.

This is not cosmetic. The one live use of the phantom — `panel_mode_cycle_x`'s
owner at `src/app/key_policy.rs:267` — is `KeyPolicyOwner::Sub(ComponentId::Library)`,
so `router::is_global_binding` returns `false` for it. The router therefore does
**not** suppress the `x` chord while a text-entry surface is focused
(`RouterSnapshot.text_entry_focused` is true for `Overlay(Search)`,
`Overlay(Settings)`, and `InlineSearch(_)` — `shell.rs:162`). Result: typing a
literal `x` into the global search field, the settings filter, or an inline
library search cycles the panel layout instead of inserting the character. `x`
is a global chrome toggle and belongs to `UiRoot` like every other global chord.

This is issue #613's Library-parent slice, split out from the deleted
`resolve-migrated-surface-correctness` bundle so it can land immediately and
unblock the Library-parent portion of #614.

## What Changes

- Remove `ComponentId::Library` from the enum (`src/app/components/component_id.rs:20`).
- Reassign `panel_mode_cycle_x`'s owner from `Sub(ComponentId::Library)` to
  `Sub(ComponentId::UiRoot)` (`src/app/key_policy.rs:267`), making `x` a global
  binding that is correctly suppressed during text entry. **User-visible fix:**
  `x` now types into focused search/settings/inline-search fields.
- Rewrite ledger row 64 ("Root | Library parent") to describe the actual
  mechanism: no component, routing is pure derivation over `App.tab` in
  `shell_library.rs`, and focus routes to the mounted destination child or
  `UiRoot`. Drop the reference to the non-existent `LibraryComponent` and
  `src/app/components/library.rs`.
- Fix the stale comment in `shell_library.rs:12-14` that still references
  `sync_library_parent` / `LibraryComponent` mirroring.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `interactive-component-framework`: the "Complete conversion with no
  mixed-framework endpoint" requirement's ledger-accuracy clause now has a
  concrete scenario — a `ComponentId` variant and ledger row naming a
  component that does not exist is a documentation/source contradiction the
  completion gate forbids. Adds a scenario that keyboard-policy owner tags
  must name a real mounted component (or `UiRoot`), because the router's
  global-vs-focused classification keys off the owner.

## Impact

- `src/app/components/component_id.rs`, `src/app/key_policy.rs`,
  `src/app/shell_library.rs` (comment only).
- `docs/architecture/interactive-surface-ledger.md` row 64.
- No `ComponentId::Library` matches exist outside `key_policy.rs:267` (verified
  by `rg`); the compiler will flag any non-exhaustive `match` the removal
  exposes. No protocol, persistence, or `App` state changes.
- The router's `panel_mode_cycle_x` behavior changes only for the
  text-entry-focused case; with no text entry focused, `x` continues to cycle
  panel mode exactly as before (`command_for_policy` maps `PanelModeCycle` →
  `Command::CyclePanelMode` regardless of owner).
