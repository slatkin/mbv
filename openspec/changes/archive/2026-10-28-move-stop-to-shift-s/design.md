## Context

`stop` is a registry action in the Playback section with default chord `Esc`
(`crates/mbv-keybinds/src/registry.rs`). The router resolves `Esc` for Stop as a
`Deferred` candidate (`src/app/input/router.rs`, the `chord.code == KeyCode::Esc`
arm). `Model::router_outcome` (`src/app/shell/arbitration.rs`) then rewrites that
candidate with a 600 ms window held in `Model::last_esc`: first press
`FallThrough`, second press `Command(Stop)`, or `Swallow` when the leaf already
stops (`leaf_stops`). Commit `a65d18b06` added this. The playback panel
(`library_playback_panel.rs`) also hardcodes `Esc` as `PlaybackRequest::Stop`
and consumes `Esc` even when it emits nothing.

Facts checked in code:

- The `stop` policy entry is `global: false` and gated by `Playback`. That gate
  is closed for blocking overlays and text entry (`playback_allowed`,
  `key_policy.rs`). So a plain `S` typed in Search or Settings never stops.
- `Shift+N` and `Shift+P` already use the same chord grammar and gate.
- No registry default and no component handler uses `Shift+S` or `Char('S')`.
  Lowercase `s` is Shuffle in music, TV, and playlist lists. That is why `s`
  was not chosen.
- The help overlay builds its Stop row from the registry chord. It needs no edit.
- The playback panel's `key()` ignores any modified key, so `Shift+S` reaches the
  router untouched. The same holds in a Visual selection, where the list's
  Visual handler only acts on `Esc`, `Space`, `Ctrl+A`, and `Shift+V`.

## Goals / Non-Goals

**Goals:**
- `Esc` is a leaf key everywhere. It carries no shell timing state.
- The default Stop chord stops on one press, wherever the Playback gate is open.
- A config that assigns `Esc` to `stop` keeps loading, with defined behavior.

**Non-Goals:**
- Changing which other chord does what, or the policy table order.
- Making `stop` prefix-addressable.
- Moving the keyboard decision behind a single seam. That is the separate change
  `keyboard-resolution-seam`.
- Adding an `Esc` alias or a migration warning for old configs.

## Decisions

**D1. The default chord is `Shift+S`.** It needs a deliberate Shift, so it is
hard to hit by accident. It sits with the `Shift+N` and `Shift+P` transport
keys. Alternatives: `s` (taken by Shuffle in three lists, and easy to
fat-finger), `End` (taken by list navigation), `Ctrl+s` (some terminals read it
as flow control).

**D2. `Esc` gets no alias.** The point of the change is to free `Esc`. A user who
wants it back assigns it in config (D4).

**D3. Remove the window completely.** Delete `last_esc`, the
`DOUBLE_ESC_STOP_WINDOW` constant, the reset in `ui_state_reset`, the `now` and
`double_esc` locals, the `leaf_stops` scan, and the `match` that rewrites
`Deferred(Stop)`. Remove imports that this leaves unused. The result for a
`Deferred` candidate is the router's own outcome, with no rewrite.

**D4. A configured `Esc` stop chord stays a deferred candidate.** The router's
`Esc` arm stays. With the rewrite gone, `Deferred(Stop)` goes to
`apply_deferred_candidate`: an unhandled `Esc` stops on that press, a consumed
`Esc` cancels it. This is the behavior the main `semantic-input-arbitration` spec
states, and it needs no clock. The alternative is a load error for `stop = "Esc"`.
Rejected: it breaks configs that load today and adds a validation rule for one
chord. The alternative of firing `Command(Stop)` immediately for `Esc` is also
rejected: the router would discard the leaf's back or close request on every
`Esc` while playback is active.

The doc comment on `apply_deferred_candidate` says a deferred candidate is "only
ever `TogglePlayPause`". That stops being true. Reword it, and keep the
`debug_assert!(!quit, ...)`: Stop does not quit.

**D5. The playback panel stops claiming `Esc`.** Remove the `Key::Esc` arm in
`key()` and the `Key::Esc` term in the `Consumed(None)` list in `key_result`.
`Esc` is then `Unhandled` there. Alternative: map `Shift+S` in the panel too.
Rejected: the panel's `key()` ignores modified keys on purpose, and the router
already handles `Shift+S` from every focus. This also makes the panel agree with
a rebind of `stop`, which it ignores today.

**D6. Leave `stop` non-prefix-addressable.** The registry comment gives
"Escape while armed must disarm" as the reason. With a non-`Esc` default that
reason no longer describes the default. Keep the flag as is and reword the
comment to say what is still true: an `Esc` stop chord and the armed disarm
chord would collide. Making `stop` prefix-addressable is a separate change.

**D7. Docs follow the code in the same change.** ADR 0023 Amendment: drop the
300 ms double-tap description and the candidate-clock sentence. `CONTEXT.md`
*Arbitration fold*: drop "the only place where deferred-candidate clocks advance
or reset". Both are plain text edits and need no new term.

**D8. Test ownership.** Each contract has one owner:
- Default chord table: `crates/mbv-keybinds/src/tests.rs` (change `Esc` to
  `Shift+S`). The `override_count` test used `stop = Esc` as its "redundant
  default-equal override"; swap in `Shift+S` so that case keeps its meaning.
- Router outcome per chord and snapshot: `src/app/tests/routing_matrix/playback.rs`.
  `Shift+S` gives `Command(Stop)` when the gate is open and `FallThrough` in text
  entry. `Esc` gives `FallThrough` with defaults. A keybinds fixture with
  `stop = Esc` gives `Deferred(Stop)`.
- Deferred fire and cancel: `src/app/shell/tests.rs` already owns it for Space
  (`consumed_space_cancels_the_playback_candidate_and_leaves_no_state`). The
  command is not part of that contract, so no Escape twin is needed.
- Delete the three double-Esc tests in `src/app/shell/tests.rs` and
  `live_tick_escape_stops_on_the_second_press` in
  `src/app/tests/tick_integration.rs`. Their contract (the window) no longer
  exists.
- Playback panel: the existing `Esc` assertion becomes "`Esc` is unhandled".
- Settings row: `src/app/shell/settings.rs` expects the default text `Esc` for
  `stop`. Expect `Shift+S`.

## Risks / Trade-offs

- **[Muscle memory]** Users who press `Esc` to stop get no stop and no message.
  This is the requested behavior. Mitigation: the help overlay shows the new
  chord, and `stop = "Esc"` restores the old key (without the window).
- **[Configured `Esc` loses the double press]** A config with `stop = "Esc"` now
  stops on the first unhandled `Esc`. That is the older, documented behavior and
  the spec keeps it.
- **[Pending change overlap]** `keyboard-resolution-seam` plans an Escape clock
  and spec delta that this change makes obsolete. Reduce that change after this
  one is accepted. Do not archive both with overlapping `semantic-input-arbitration`
  deltas.
- **[Unchecked scope]** Checked only by reading: `Char('S')` use in components
  and the registry defaults. A live run of help and Visual mode is for the user.

## Migration Plan

One change. Order: registry default and its table test; panel `Esc` removal;
shell window removal and test deletion; docs. Each step keeps the tree green.
Rollback is a revert. No data or config migration. An old config with
`stop = "Esc"` loads as before.

## Open decisions

None. Decisions D1, D2, and D4 were assumptions of this proposal. Confirm or
change D4 before apply, since it decides what a configured `Esc` stop does.
