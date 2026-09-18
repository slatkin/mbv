# Design: add-configurable-keybinds

## Context

mbv's keyboard surface is two-tier: the central router (`router.rs` + `key_policy.rs`, ADR 0023) resolves chords against an ordered, gated `KEY_POLICY` as a pure function of a normalized `KeyChord` and a plain-data `RouterSnapshot`; focused leaf components (~470 `Key::` matches across 25+ components) own local semantic chords. The policy's chord matching is hard-coded in `KeyPolicyBinding::matches()`, and the playback transport letters resolve through one bucketed `Playback` matcher (`KeyPolicyGate::Playback` → `resolve_key(InputContext::Playback, …)`), with help's Playback rows hand-written in `PLAYBACK_HELP_BINDINGS`.

Config lives in mbv-core (`Config`), persisted by a read-patch-write TOML path (`save_config_settings_at`) that patches named sections and preserves unknown keys (but not comments). The settings UI (`SETTING_SECTIONS` + `SettingKey`) is the user-facing grouping of configurable behavior, with a `Services` child destination as the precedent for a nested screen. App-owned timing state already reaches the policy via `RouterSnapshot` booleans (`space_double_tap`/`esc_double_tap` precedent), and the shell holds the candidate clocks in `apply_deferred_candidate`.

See proposal.md for motivation; see `specs/configurable-keybinds/spec.md` for the behavior contract and `specs/semantic-input-arbitration/spec.md` for the modified arbitration requirement.

## Goals / Non-Goals

**Goals:**
- One declared keybind action table that config parsing, defaults, policy matching, help, and the settings UI all enumerate.
- Config-driven chords for every router-owned action (globals + transport), with defaults compiled from that declaration.
- A sectioned config surface whose sections are the settings UI's sections plus `Global`, and a read-only Keys destination that shows exactly that set.
- A sticky prefix mode with total namespace capture.
- Validation at load; no process-global keybind state; ADR 0023 purity preserved.

**Non-Goals:**
- Leaf-local rebinding (the prefix namespace is the saturation answer, not per-component remapping).
- New `Command` variants; in-place key capture in the settings UI; chord sequences; per-destination binding sets.
- Adopting `crossterm-keybind`/`keybind-rs` (see D1).

## Decisions

### D1: One `KeybindAction` declaration in mbv-core is the source of truth

A single table declares each configurable action and everything derived from it:

```rust
struct KeybindAction {
    id: &'static str,            // config key; also the policy entry's name
    section: KeySection,         // SETTING_SECTIONS vocabulary + Global
    default_chords: &'static [&'static str],
    gate: KeyPolicyGate,         // shared with key_policy.rs semantics
    policy: KeyPolicyBinding,    // the existing matcher identity it replaces
    rebindable: bool,            // router-scope override allowed
    prefix_addressable: bool,    // may be assigned in the prefix namespace
}
```

Consumers: `Keybinds::defaults()`, config parse + validation, `resolve_policy` matching, `command_for_policy`, help's Playback/Global rows, and the settings Keys destination. Nothing else may spell a chord or an action name. Alternatives rejected: two hand-maintained maps plus an equality test (drifts again on the next change), and per-surface lists (the state this change exists to end).

Hand-rolled rather than `crossterm-keybind`: that crate's value is TOML plumbing, but its architecture is a flat chord→variant enum initialized once into process-global state. mbv's policy is ordered, gated, snapshot-driven, and pure; the crate cannot express gates, precedence, blocking, or the leaf tier, and would add a third vocabulary beside `KeyPolicyBinding` and `Command`. The chord grammar is small; one owned parser avoids a versioned crossterm feature matrix.

### D2: Action identity reuses the policy entry names; the transport bucket splits

`KeyPolicyEntry.name` is already the stable, unique name of each layer (asserted by `policy_entries_have_unique_ordered_names`) and is currently `allow(dead_code)` outside tests. Those names become the registry ids and the config keys — no new vocabulary, and the dead-code allowance goes away.

The one entry that cannot carry a single identity is `KeyPolicyBinding::Playback`, whose gate resolves any chord through `resolve_key(InputContext::Playback, …)`. It splits into one registry action per transport behavior (`toggle_play_pause`, `stop`, `seek_back`/`seek_forward`, `next_track`, `previous_track`, `volume_down`/`volume_up`, `toggle_mute`, `toggle_mute_or_cycle_audio`, `cycle_subtitle`, `open_idle_feed_link`, …), each with `key_policy`'s `gate: Playback`. Payload-carrying variants (`SeekRelative(f64)`, `AdjustVolume(i64)`) are bound to fixed values at their single key sites today, so they become parameterless named actions (`seek_back` = `SeekRelative(-5.0)`). `QueuePlayCursor(usize)`, `SetLibraryTab(usize)` and `FocusPanel(..)` are already parameterless in practice by their call sites; the tab-jump entry (`library_tab_jump`, chars `1-9`) keeps its positional resolution, which is why registry matching is per action with an optional positional resolver rather than a plain chord compare.

The shared `gate: Playback` label is a routing-eligibility bucket, not a shared eligibility *condition* — today's `playback_command_for_key` gates each key individually (`Space`/`Esc`/`<`/`>`/`N`/`P`/`a` require `active || has_remote_session`; `z` only excludes Ctrl; `m`/`-`/`+`/`=` are ungated). Splitting the bucket must carry each action's own eligibility condition forward, not just its chord and label; matching a rebound chord directly (instead of re-deriving through `playback_command_for_key`) is only correct if each split action keeps the condition its key had today.

### D3: `Keybinds` is plain data passed into the policy, and its file shape is sectioned

`resolve_policy`/`command_for_policy` gain a `&Keybinds` parameter (parsed once at startup from `Config`, stored on the shell `Model`). This keeps `KEY_POLICY` ordered and gates untouched; only matching reads data instead of literals. Alternatives: baking bindings into `RouterSnapshot` (it is a `Copy` plain-data snapshot; a map does not belong there), or a trait-object policy (unjustified abstraction).

File shape — section outer, prefix as a sub-table:

```toml
[keys]
prefix = "Ctrl+b"

[keys.global]                 # Global section, router-scope overrides
help_open     = "F1"
settings_open = "F2"

[keys.library]                # Library section, router-scope overrides
next_library_tab = "Tab"

[keys.library.prefix]         # same section, prefix-namespace assignments
next_library_tab = "n"
```

Rationale: a section named `Global` makes scope-outer nesting (`[keys.global.global]`) ambiguous; section-outer reads as "this part of the keyboard", with `.prefix` the alternate namespace. A chord-keyed prefix map (`"n" = "next_library_tab"`) was rejected: it cannot be grouped by section without indirection, and it cannot be validated for "the action's declared section". `default_chords` is a list so aliases (`+`/`=`, `<`/`>`) survive; a configured action collapses to its single configured chord. Multi-chord aliases in config are deferred (YAGNI; the shape does not preclude them).

### D4: Sections are the settings UI's sections plus `Global`

`KeySection` mirrors `SETTING_SECTIONS`' names and order (`Services`, `Playback`, `Display`, `Session`, `Library`, `Queue`, `Mpv`, `Feeds`, `Actions`) and adds `Global` for chrome chords (help, settings, panel focus, panel layout, force clear). `Global` is existing mbv vocabulary: CONTEXT.md defines *Global chord*, and the help panel already prints a `Global` section. `SETTING_SECTIONS` gains the `Global` entry for the Keys destination; it has no ordinary config rows, so the Keys content is what fills it.

Chrome assignment: `F1`, `F2`, `F3`, `F4`, `x`, `←`/`→`, `Ctrl+L`, `Alt+↑/↓`, `1-9`, `F5`, `Tab`/`BackTab`, `q`, `c`, `v` distribute across the existing sections by domain (Session: `q`, `F3`; Library: `Tab`, `BackTab`, `1-9`, `Alt+↑/↓`, `F5`, `Ctrl+/`; Queue: `c`, `F4`; Playback: `v` and the transport set; Display: `x`, `←`/`→`, `Ctrl+L`; Global: `F1`, `F2`).

### D5: Validation happens at load, against the declaration

Rejections, each naming the offending entry: reserved chord (`Ctrl+q`, an extensible const), unparseable chord string, unknown action id, an action placed in a section other than its declared one, a prefix-namespace assignment for a non-`prefix_addressable` action, a collision between the prefix chord and any other configured/default chord, **a collision between two configured router-scope chords (any two rebindable actions resolving to the same chord)**, and **a collision between two configured prefix-namespace chords (any two prefix-addressable actions assigned the same chord across `[keys.*.prefix]` tables)**. Without these last two, two actions could silently share a chord — router-scope order would pick whichever `KEY_POLICY` layer is ordered first, and prefix-namespace order is unspecified — which is exactly the drift this decision exists to prevent. Reserved-chord motivation: `Ctrl+q` is not globally bound today (every ordinary `q`/`Q` handler outside a blocking overlay guards `mods.is_empty()`), though `DaemonLostComponent` already matches bare `q`/`Q` with no modifier guard while its overlay is focused, so `Ctrl+q` already triggers quit there; reserving it globally is still uncontested and is held for a future hard binding.

### D6: Prefix mode is two policy layers plus one App-owned bit

- `RouterSnapshot.prefix_armed: bool` mirrors an App-owned flag (new snapshot field and new App-owned bit; the closest existing shape is the double-tap clocks this change removes — see D8 — which live as `App.last_space_press`/`last_esc_press`, read entirely inside `shell.rs`'s `apply_deferred_candidate` and never mirrored into `RouterSnapshot`. `prefix_armed` differs from that precedent in exactly the way it needs to: the policy must see it, so it is mirrored; the old double-tap clocks never needed router visibility).
- Top layer `prefix_arm` (before the blocking catch-all): matches the configured prefix chord, gated on `!text_entry_focused && !blocking_overlay_open`, `blocking: true` semantics — arms and swallows.
- Armed-dispatch layer: while armed, every chord resolves against the prefix namespace only. Mapped → `Command` + disarm; Esc or unmapped → Swallow + disarm; the prefix chord → re-arm. No `FallThrough` path exists while armed, which is what makes every leaf key safe underneath. A mapped chord's `Command` fires only if the action's declared `gate` currently allows it — armed dispatch reuses each action's normal eligibility check rather than bypassing it; a mapped chord whose gate is currently closed (e.g. a `Playback`-gated action assigned in the prefix namespace with nothing playing) is treated as unmapped: Swallow + disarm, no `Command`.
- Disarm on any mouse event is a shell-side flag clear in the mouse path; the mouse event's own delivery is unchanged.

### D7: A read-only Keys destination, and help rendered from the registry

The F2 settings panel gains a `Keys` row (value = configured prefix plus override count) and a `Keys` destination, following the `Services` child-destination precedent. The destination lists exactly the registry's configurable set, grouped by `KeySection`, one row per action showing the router chord and, when assigned, the prefix chord. Leaf-local keys stay in F1 help, where they are already grouped by destination. Display and dispatch therefore share one source; in-place chord capture is deferred because it would need a capture mode that consumes the next chord before normal policy — a routing exception ADR 0023 guards deliberately.

Help: the Playback section's rows and the Global section's configurable chords are generated from the registry rather than `PLAYBACK_HELP_BINDINGS.keys` string literals — that table and its hand-written strings live in `src/app/action.rs`, not `help.rs`; `render/components/help.rs` only imports and renders it. The declaration makes help-match-behavior structural instead of asserted.

### D8: The double-tap deferral is removed

`Space` x2 → pause/resume and `Esc` x2 → stop exist in two places: the router's `Deferred` path (shell `apply_deferred_candidate` + `App.last_space_press`/`last_esc_press`, 300 ms) and the leaf `LibraryPlaybackPanel` (`last_space`/`last_escape` + `double_tap()`). This is a prerequisite for remappable transport: a rebound `stop` chord would otherwise route through the window and need two presses.

Removal: delete both clocks and the timing branch; `Deferred` keeps its meaning ("leaf first refusal; the router runs it only if the leaf did not claim the chord") and dispatches immediately. `App` loses `last_space_press`/`last_esc_press`, and `apply_deferred_candidate` loses the timing branch that reads them (these fields were never mirrored into `RouterSnapshot` — there is nothing to remove there). Consequences: an unclaimed `Space` toggles play/pause on the first press, an unclaimed `Esc` stops on the first press, and in the focused playback panel both fire single-press. `Esc`'s dismiss/back duties are unaffected because those leaves claim the chord before arbitration. The semantic-input-arbitration delta records the new contract; the characterization families that lock the old behavior become behavior-change records.

### D9: `alt_swallow` is excluded from the registry

`alt_swallow` is a `global: true` policy entry whose `command_for_policy` arm is `None` — it is a swallow guard, not an action. It has no action identity, so it is not configurable, not prefix-addressable, and not listed; it keeps its literal match. The same exclusion applies to any future policy entry that blocks rather than commands.

## Risks / Trade-offs

- [Armed mode surprises users who fat-finger the prefix] → unmapped/Esc disarm is one keystroke, mouse disarms silently, the status pill makes the state visible; sticky-until-key is tmux's default and was chosen over timed expiry.
- [Single-press `Space`/`Esc` can pause or stop a surface the user did not mean to control] → only fires when the focused leaf does not claim the chord, so activate/dismiss surfaces are unaffected; the behavior change is pinned by its own spec scenarios and rewritten characterization tests rather than slipped in.
- [Rebinding breaks muscle memory documented in help/screens] → help and the Keys destination render live configuration.
- [Config round-trip must not corrupt unrelated sections] → reuse the read-patch-write path; `[keys]` tables are patched like any other section, with prune-on-empty for absent sections. Comments are not preserved by that path (existing behavior), so section and action names must be self-describing.
- [Policy signature change ripples through routing-matrix tests] → fixtures construct registry defaults; the parameter is one constructor call away from today's fixtures.
- [A registry edit that forgets a consumer reintroduces drift] → the consumers are enumerated in D1 and the Keys destination plus help are asserted against the same table in tests.

## Migration Plan

Additive for the file: without `[keys]`, every binding behaves as today's defaults (the spec pins this) except the double-tap change, which is a deliberate behavior change with its own requirement. Ship registry → config + validation → policy parameterization + transport split → double-tap removal → prefix mode → Keys destination + help. Rollback is removing the section; no data migration. The existing `PLAYBACK_HELP_BINDINGS.keys` strings are deleted, not preserved.

## Open Questions

- Exact chord grammar spelling (`Ctrl+Shift+b` ordering, `Del` vs `Delete`) — one canonical form documented with the config keys; parser accepts modifier order-insensitively.
- Whether the Keys destination marks gated actions ("only while playing") in the row or a footer; the gate is declared, so the data is available either way.
- Where the armed pill sits when the status bar is width-constrained (follow the existing drop-order precedent).
