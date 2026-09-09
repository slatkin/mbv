# Design: add-configurable-keybinds

## Context

mbv's keyboard surface is two-tier: the central router (`router.rs` + `key_policy.rs`, ADR 0023) resolves chords against an ordered, gated `KEY_POLICY` as a pure function of a normalized `KeyChord` and a plain-data `RouterSnapshot`; focused leaf components (~470 `Key::` matches across 25+ components) own local semantic chords. The policy's chord matching is hard-coded in `KeyPolicyBinding::matches()`. App-owned timing state (double-tap windows) already reaches the policy via `RouterSnapshot` booleans. Config lives in mbv-core (`Config`), persisted by a read-patch-write TOML path that preserves unknown sections. See proposal.md for motivation; see `specs/configurable-keybinds/spec.md` for the behavior contract.

## Goals / Non-Goals

**Goals:**
- Config-driven chords for the 16 router-owned globals; a sticky prefix mode with full namespace capture; validation at load; armed indicator in the status bar; bindings rendered in help from the same data the router uses.
- ADR 0023 purity preserved: the policy stays a pure function of plain-data inputs; no process-global keybind state.

**Non-Goals:**
- Leaf-local rebinding (the prefix namespace is the saturation answer, not per-component remapping).
- New `Command` variants, timed prefix expiry, multi-chord sequences, per-destination binding sets.
- Adopting `crossterm-keybind`/`keybind-rs` (see Decisions).

## Decisions

### D1: Hand-rolled `Keybinds` type in mbv-core, not crossterm-keybind

The crate's value is its TOML plumbing (parse, struct-patch merge, example generation); its architecture is a flat chord→variant enum initialized once into process-global state via `init_and_load`. mbv's policy is ordered, gated, snapshot-driven, and const-pure; the crate cannot express gates, precedence, blocking, or the leaf tier, and would add a third parallel vocabulary beside `KeyPolicyBinding` and `Command`. Steal the shape (declared defaults + TOML override map), skip the dependency. Alternative considered: wrap `crossterm-keybind-core` for chord parsing only — rejected: the chord grammar is small, and keeping one owned parser avoids a versioned crossterm feature matrix (`crossterm_0_29_0`) for ~60 lines of code.

### D2: `Keybinds` is plain data passed into the policy

`resolve_policy`/`command_for_policy` gain a `&Keybinds` parameter (parsed once at startup from `Config`, stored on the shell `Model`). Chord matching for rebindable globals compares against configured chords; defaults are the current hard-coded chords. This keeps `KEY_POLICY` ordered and gates untouched — only `matches()` reads data instead of literals. Alternatives: baking bindings into `RouterSnapshot` (it is a `Copy` plain-data snapshot; a map does not belong there), or a trait-object policy (unjustified abstraction).

### D3: Prefix mode as two policy layers plus one App-owned bit

- `RouterSnapshot.prefix_armed: bool` mirrors an App-owned flag (precedent: `space_double_tap`/`esc_double_tap`).
- Top layer `prefix_arm` (before `selection_modal`'s blocking catch-all): matches the configured prefix chord, gated on `!text_entry_focused` + `!blocking_overlay_open`, `blocking: true` semantics — arms (shell side-effect) and Swallows. Arming is a shell state transition triggered by the router outcome, like other policy-dispatched effects.
- Armed-dispatch layer: while `prefix_armed`, the first matching layer redirects every chord to prefix-map lookup → `Command`, `Swallow`+disarm (Esc/unmapped), or re-arm. No FallThrough path exists while armed: namespace capture is total, which is what makes leaf keys safe underneath.
- Sticky disarm (tmux parity): Esc/unmapped key swallow+disarm; any mouse event disarms silently (mouse delivery itself unchanged — the shell clears the flag in its mouse path without consuming the event). No timer; no new clock dependency.

### D4: Single chord per action; defaults compiled in

`[keys.rebind]` maps an action name to exactly one chord; the default chord is compiled from the existing policy table so docs and behavior stay in lockstep. Multi-chord aliases are deferred (YAGNI; the map shape does not preclude them). `[keys.bind]` maps a chord to an existing `Command` variant name; unknown names are load errors.

### D5: Reserved chords are a const, validated at load

`RESERVED_CHORDS = ["Ctrl+q"]` (extensible const). Validation at config load: reserved chords, prefix/rebind collisions, unknown commands, unparseable chord strings — all reject the `[keys]` section with a named error via mbv-core's existing config error path. Reserved-chord motivation: `Ctrl+q` is currently unbound (every `q` handler guards `mods.is_empty()`) and is held for a future hard binding; validation costs nothing today and prevents users from discovering the collision later.

### D6: Indicator and help read the same `Keybinds`

Status bar: one `prefix_status_spans()` pill in the `chrome_status.rs` idiom, rendered only while the armed flag is set (chrome is shell-painted; ADR 0024 already covers shell chrome geometry). Help: the bindings section renders from `Keybinds` (defaults merged with overrides) plus the prefix map — display and dispatch share one source, so they cannot drift.

### D7: F-keys do not escape the armed namespace

While armed, every chord — including F1–F4 — resolves against the prefix map only. This is the multiplexer contract; the text-entry F-key exception exists to protect typing, and while armed nothing reaches the text entry anyway.

## Risks / Trade-offs

- [Armed mode surprises users who fat-finger the prefix] → unmapped/Esc disarm is one keystroke, mouse disarms silently, the status pill makes the state visible; sticky-until-key is exactly tmux's default and was chosen deliberately over timed expiry.
- [Rebinding breaks muscle memory documented in help/screens] → help renders live config (D6); rebind is opt-in per action.
- [Prefix chord chosen poorly collides with a default global] → load-time collision validation (D5) rejects prefix == any rebound or default global chord, not just reserved ones.
- [Policy signature change ripples through routing-matrix tests] → tests construct `Keybinds::defaults()`; the parameter is one constructor call away from today's fixtures.
- [Config round-trip must not corrupt unrelated sections] → reuse the existing read-patch-write path (`save_config_settings_at` pattern) rather than serializing the whole `Config`.

## Migration Plan

Additive: without `[keys]`, behavior is byte-identical to today (spec scenario pins this). Ship config type + validation → policy parameterization + prefix mode → indicator + help. Rollback is removing the section; no data migration. Tests: routing-matrix fixtures gain `Keybinds::defaults()`; new tick-integration tests prove arm/disarm/namespace capture through `Application::tick()` (component-local `on()` tests are insufficient for composition, per house rule).

## Open Questions

- Which status-bar slot the armed pill occupies when the bar is width-constrained (the pill family already has width-pressure behavior; follow the existing drop-order precedent).
- Exact chord grammar spelling (`Ctrl+Shift+b` ordering, `Del` vs `Delete`) — one canonical form documented with the config keys; parser accepts modifier order-insensitively.
