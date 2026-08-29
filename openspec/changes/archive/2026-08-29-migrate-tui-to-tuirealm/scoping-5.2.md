# Scoping: task 5.2 (Root UI + overlay routing + deferred precedence gates)

Written after 5.1 landed (37/45). Read before sending an agent at 5.2.

## Why 5.2 needed a scoping pass

Tasks 3.x/4.x each deferred one precedence question to 5.2 on the grounds that a
mirror-first surface keeps legacy input and therefore needs no guard yet. That
was right, but it concentrated every deferral into one task. This note resolves
them on paper so the agent implements rather than investigates.

## The gates that actually remain

`src/app/key_policy.rs` still carries six `KeyPolicyGate::Custom` entries:

| line | entry | gate string | status |
|---|---|---|---|
| 84 | (blocking overlay) | `Not(IsMounted(blocking overlay))` | open — set-valued, see below |
| 107 | `lib_search` | active library tab's LibSearch is Some | **solved by 3.3's mount lifecycle** |
| 124 | `confirm_skip_intro` | `skip_intro_end_ticks.is_some()` | **already carried as an attribute** |
| 130 | `confirm_next_up` | `next_up_item.is_some()` | **already carried as an attribute** |
| 163 | `playback` | player_active/remote gate | irreducible — see below |
| 185 | `album_track_mode` | `album_track_focus.is_some()` | **solved by 4.4's component-local state** |
| 192 | (library leaf) | focused Library leaf | **solved by 5.1's LibraryComponent** |

Two of these were resolved before this change even reached phase 3:
`src/app/components/playback_gates.rs` documents that of the six original
`Custom` entries, `clear_queue_prompt_c` and `visualizer` turned out to be
unconditional key matches and were corrected to `KeyPolicyGate::Always`.

## The pattern that dissolves the "runtime-parameterized key" problem

`lib_search` and `album_track_mode` were both marked as blocked on the same
limitation: `SubClause::IsMounted` takes one concrete `ComponentId`, and these
surfaces mount under a key derived at runtime (`ComponentId::InlineSearch(
BrowserKey { service, library_id, kind })`). You cannot write a static clause
against a key you do not know at compile time.

**Do not build per-instance `SubClause`s at mount time.** The repo already has a
working answer: `PlaybackGatesComponent` is a component that paints nothing,
is never activated or subscribed, and exists solely to hold attributes the shell
refreshes every tick (`shell_gates.rs::sync_precedence_gates`). A clause then
reads `SubClause::HasAttrValue(<stable carrier id>, ATTR, Flag(true))`.

That indirection removes the runtime-key problem entirely: the clause names a
*stable* carrier, and the shell — which already knows the active library —
resolves the parameterized question when it sets the flag. Extend the existing
carrier with `lib_search_active` and `album_track_focused` rather than inventing
a second mechanism.

The same trick covers line 84's blocking-overlay gate, which is set-valued
(`IsMounted` cannot express "none of these N ids"): carry one
`blocking_overlay_active` flag the shell sets from the overlay stack.

## What stays irreducible

`playback` (line 163) resolves **per key** through `resolve_key`. It is not a
predicate over state, so no attribute can represent it. Leave it as legacy
dispatch and record that in the ledger. Do not spend the session trying to
decompose it — the design already concedes this.

## Known stale scaffold — fix in 5.2

`shell.rs:106-117` mounts `PlaybackComponent::new()` at `ComponentId::Playback`
under a comment describing the gates carrier, with `.expect("mount PlaybackGates")`.
Task 4.10 superseded the scaffold as its own TODO predicted, but:

- `PlaybackGatesComponent` (`components/playback_gates.rs`) is now referenced
  nowhere outside its own file. Clippy cannot see it because it is `pub`.
- The mount comment and the `.expect` string are stale.
- `shell_gates.rs` sets `ATTR_SKIP_INTRO_PROMPT_VISIBLE` /
  `ATTR_NEXT_UP_PROMPT_VISIBLE` on `ComponentId::Playback`, which now resolves
  to `PlaybackComponent`. **Checked: this is correct.** `components/playback.rs`
  keeps a `Props` field, seeds both attributes in `new()`, and implements
  `query`/`attr` against it. The two gates read real state.

So `PlaybackGatesComponent` is dead code, not a live carrier. Delete it, and fix
the stale mount comment and `.expect("mount PlaybackGates")` string at
`shell.rs:106-117`. `PlaybackComponent` becomes the carrier for the new flags
below.

## Scope boundary

5.2 converts Root UI and overlay-stack routing onto TuiRealm's native LIFO focus
stack (open = `active`, dismiss = `umount` -> auto-blur/restore; no shell-owned
focus stack) and wires the clauses above. It does **not** delete `App` state —
that is 5.3a-5.3d.
