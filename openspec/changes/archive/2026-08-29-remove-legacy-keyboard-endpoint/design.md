## Context

See `proposal.md` for motivation and `docs/adr/0023-one-central-keyboard-router.md`
for the routing decision this design implements.

The endpoint is not mechanically dead. Its removal was blocked by a missing
capability, and three separate symptoms were read as three separate problems:

- `key_policy.rs` is a static proof only; production never installs the
  subscriptions it describes, and the file still carries `#![allow(dead_code)]`.
- D15's `Component::perform(Cmd)` path was declined at task 5.4 while
  `LegacyInput` and `CONTEXT_STACK` were live.
- The `5.3d.22` deletion rows required zero references rather than authorizing a
  replacement for the live route, so they documented a no-op.

All three are the same fact: **TuiRealm's dispatch has no ordering relation.**
`Application::tick` forwards each event to the focused component *and* to every
satisfied subscription; there is no consumed signal, `sub_lock` is
all-or-nothing, and `SubClause` can only read `mounted()`, `state()`, and
`query(Attribute)`. `Swallow` and `FallThrough` — the two ADR 0002 outcomes that
*are* precedence — cannot be expressed. `GlobalViewKey` is the relation's
stand-in, which is why it is unremovable rather than merely unreferenced.

Two further entanglements are real and are handled explicitly below:

- `handle_legacy_key` re-pushes Home, Emby browser, ABS podcast, ABS book, and
  Music presentation after **every** key. Removing it requires each typed effect
  path to push only what it changes.
- The skip-intro/next-up prompts are a status-bar prompt, a desktop
  notification, and an invisible focus-stealing component for one decision that
  mpv already renders a button for. They are removed rather than routed.

D7's assignment of a parent binding to `ComponentId::Library` remains unwirable
— no `LibraryComponent` is mounted — and stays with #607.

## Goals / Non-Goals

**Goals:**

- One live keyboard routing authority, in `UiRoot`, implementing ADR 0002's
  three outcomes.
- Leaves interpret only their own surface and emit typed semantic requests.
- `key_policy.rs` becomes the router's executable policy.
- Delete all production raw-key fallback and Crossterm reconstruction.
- Keep implementation units reviewable despite the broad consumer fanout.

**Non-Goals:**

- Changing shortcuts (other than removing the skip-intro/next-up `Y`/`n`),
  fixing input quirks, adding configurable keybindings, or adopting `CmdResult`
  redraw gating.
- Adopting `perform(Cmd)` as the component input API. ADR 0023 records it as a
  separate, later decision.
- Creating a placeholder `LibraryComponent`.
- Moving shell/runtime, Player, Service, persistence, or canonical Queue effects
  into Interactive Components.
- Restoring deferred mouse paths or changing rendering.
- Re-adding a TUI skip-intro/next-up affordance.

## Decisions

### 1. `UiRoot` is the Keyboard Router; its resolution selects between two messages

`UiRoot` is permanently mounted and already subscribes with
`EventClause::Any` / `SubClause::Always`, so it observes every chord regardless
of focus. It is skipped by `forward_to_subscriptions` while focused and receives
the event as the active component instead, so delivery is exactly once either
way.

`Application::tick` returns the focused component's message before subscribers'.
With `PollStrategy::Once` there is at most one terminal event per tick, so the
leaf's request and the router's resolution for the same chord arrive together:

```
key ──┬─▶ focused leaf ─────▶ Option<Msg>   local meaning, or None
      │
      └─▶ UiRoot router ────▶ Command     → run it,  discard the leaf's Msg
                              Swallow     → run none, discard the leaf's Msg
                              FallThrough → the leaf's Msg stands
```

`shell_run.rs` already snapshots focus before folding messages and already
notes that `PollStrategy::Once` yields 0 or 1 terminal events per tick; the fold
becomes "apply the router's outcome to this tick's leaf message" rather than
"discard the observer key unless UiRoot is focused"
(`route_terminal_observer_message`).

**Alternatives considered:** (a) gated subscriptions per owner — rejected, see
ADR 0023: it cannot express `Swallow`/`FallThrough` without a distributed
attribute mirror of shell state. (b) a shell pre-router in `Model` — rejected:
the same relation sited outside the component framework. (c) `perform(Cmd)` as
the policy execution path — rejected as orthogonal; `Cmd` carries no modifiers,
so modifier-sensitive chords resolve router-side regardless.

### 2. Router policy reads a snapshot, not component attributes

`key_policy.rs` becomes the ordered policy the router evaluates, with real
runtime conditions rather than `Custom("...")` descriptions. It reads a
plain-data snapshot — ADR 0002's `InputSnapshot`, grown to cover the conditions
the current `CONTEXT_STACK` gates actually use — so policy stays a pure,
testable function.

The shadow table's gates are lossy today and are corrected while activating:

| entry | shadow gate | actual condition |
| --- | --- | --- |
| `queue_column_width` | `IsMounted(Queue)` | `PanelMode::Both` + Shift+Left/Right |
| `playback` | boolean | per-key `resolve_key` table + 300 ms double-tap |
| `clear_queue_prompt_c` | `Always` | `'c'` sans Alt, gated on no open context menu |
| `confirm_skip_intro` / `confirm_next_up` | `Custom(...)` | **deleted** — see Decision 4 |

No `SubClause` gate is added. `UiRoot`'s universal observer subscription remains
the only keyboard subscription, and it carries no precedence.

### 3. Double-tap fall-through needs no special case

The Space/Escape 300 ms double-tap already returns `None` on the first press and
`Some(dispatch(cmd))` on the second (`input_lib_keys.rs`). Under Decision 1 that
maps directly onto the router's outcomes with no rewrite of meaning:

- **first press** → `FallThrough` → the leaf's own request stands (browse
  `go_back` via `BrowserBack`/`TvBack`, Audiobookshelf
  `AudiobookshelfBookIntent::Play` / `PodcastEpisodeIntent::FocusOrPlay`).
- **second press within 300 ms** → `Command(Stop)` /
  `Command(TogglePlayPause)` → the leaf's request is discarded.

Leaves keep their ordinary Escape/Space meanings; nothing mirrors a timer, and
the router does not need a focus→effect dispatch table for these chords.

This supersedes the earlier plan in which one global handler owned Space/Escape
outright and re-dispatched the leaf's first-press action by focus. That was a
workaround for fan-out; with an ordering relation it is unnecessary.

### 4. The skip-intro and next-up TUI prompts are removed, not routed

They are three UIs for one decision that mpv already renders a clickable button
for (`scripts/mbv_intro.lua`, `scripts/mbv_visibility.lua`), with its own
show/accept/self-dismiss lifecycle. Routing them would have meant preserving:

- a prompt written into `App.status` with `status_expires = None` as a sentinel;
- a component mounted **and** `active()` on prompt state but rendered only when
  desktop notifications are off or failed — an invisible focus owner that
  swallowed every key, so `q` during a skip-intro window did not quit;
- a fourth input path (`notif_action_tx` → `drain_notif_actions`) mutating the
  same state outside any routing policy.

Removed: both `CONTEXT_STACK`/`KEY_POLICY` entries,
`handle_key_confirm_skip_intro`, `handle_key_confirm_next_up`,
`PlaybackPromptComponent`, `ComponentId::PlaybackPrompt`,
`ShellRequest::PlaybackPromptKey`, `sync_playback_prompt`,
`render_playback_prompt`, `render_playback_prompt_content`, the dead
`ATTR_SKIP_INTRO_PROMPT_VISIBLE` / `ATTR_NEXT_UP_PROMPT_VISIBLE` attributes,
both `self.status = "... (Y/n)"` writes, the two `notify_with_actions` calls,
and the `skip_intro:*` / `next_up:*` arms of `drain_notif_actions`.

Retained deliberately:

| | disposition |
| --- | --- |
| `App.next_up_item` | **kept.** `PlayerEvent::NextUpPlay` takes it to resolve the `JumpTo` index when the user clicks mpv's button. Player state, not prompt state; all existing clear sites stand. |
| `App.skip_intro_end_ticks` | **deleted.** Lua performs the seek itself; after the prompt is gone the field has writes and clears but no reader. |
| `always_skip_intro` | unaffected — `IntroStarted` still auto-seeks and never prompted. |

Consequence for `App.status`: these were its only writers with
`status_expires: None`. Afterwards `status` is a toast slot with a TTL,
unconditionally. Prompts that require an answer stay modals (`ConfirmModal`,
`clear_queue_prompt_c`), which is the only blocking shape the router offers.

The remote-daemon gap (mpv's button renders on the daemon's display) is
accepted and recorded in `docs/architecture/mpv-owned-playback-prompts.md`,
along with the conditions any re-added affordance must satisfy.

**Alternative considered:** keep the notification path and drop only the
status-bar prompt. Deferred rather than rejected — it is the recorded remedy if
the remote-daemon gap needs covering. It is out of scope here because it is a
fourth input path into state the router cannot see.

### 5. Raw keys stop at the component boundary

Every `ShellRequest` carrying a Crossterm `KeyEvent` is replaced by the smallest
semantic request set for that surface. Components decide
accept/cancel/move/submit/dismiss locally; the shell performs only effects
outside component authority. Existing target-bearing request types are reused.

This covers the bare forwards (`ConfirmKey`, `DaemonLostKey`,
`RemoteReanchorKey`, `ContextMenuKey`, `FeedsManageKey`, `SavePlaylistKey`,
`QueueKey`, `GlobalViewKey`) and the cursor-carrying
`ServiceRequest::SettingsKey { cursor, key }` /
`PersistRequest::SettingsKey { cursor, key }`, which are raw-key payloads under
a different shape. `PlaybackPromptKey` is deleted with its component.

Shared key matching uses native TuiRealm key values. Framework-neutral action
helpers may be retained only where multiple owners already share the same
semantic mapping; no new generic dispatcher is added.

**Alternative considered:** change raw payloads from Crossterm keys to TuiRealm
keys. Rejected — it renames the forwarding bridge instead of removing it.

### 6. Selection-dependent chords are leaf requests, not router commands

`.` (context menu) is selection-dependent, so it is a leaf meaning: the focused
destination resolves its own selected item and emits the explicit target,
following `browser.rs` (`BrowserContextMenu { item }`) and `music_workspace.rs`
(`MusicTrackContextMenu`). The Home Continue Watching target
(`home_cw_selected` / `cw_item`) is resolved by `HomeComponent` from
Model-owned `home_content` — the same resolution site as today, emitted by the
component rather than threaded through every `CONTEXT_STACK` handler signature.

The router's policy therefore does not claim `.`; it falls through. The
genuinely destination-independent globals — `q`, Tab/BackTab, `1`–`9`, Ctrl+L,
F5, F1 Help-open with its blocking-overlay guard, and the `handle_key_alt` path
(Alt+Left/Right panel focus, Alt+Up/Down tab cycle, catch-all Alt swallow) —
are router `Command`s and `Swallow`s.

### 7. Split the work by routing responsibility, then delete globally

The consumer fanout exceeds one safe writer unit. Serial, compile-complete
units, each ending green:

1. router seam + integration harness (behavior-preserving);
2. prompt removal (isolated, and it removes the invisible focus owner that makes
   the rest hard to reason about);
3. executable policy + globals moved into the router;
4. blocking overlays, dialogs, and forms off raw-key requests;
5. Queue;
6. Library destinations and media workspaces;
7. global deletion and architecture gates.

Each unit replaces the blanket post-key presentation pushes with targeted pushes
at that request's handler. Final deletion happens only after repository searches
show no production raw-key consumer.

**Alternative considered:** delete `CONTEXT_STACK` first and fix compile errors
outward. Rejected — it obscures precedence regressions and creates one
unreviewable edit.

### 8. Behavior is pinned by one production-style routing matrix

Direct `component.on(...)` tests prove local interpretation but cannot prove
routing. One table-driven `Application::tick()`-level matrix covers:

- a blocking overlay `Swallow`s an unbound and a global chord;
- a router `Command` discards the focused leaf's message for that tick;
- a router `FallThrough` lets exactly one leaf message stand, and fires no
  global effect;
- Queue and Library focus route representative chords to the correct owner;
- playback gating, and the double-tap's first-press fall-through / second-press
  claim.

Existing characterization tests are repointed or removed as this matrix
supersedes their legacy-loop assertions. The component boundary architecture
gate rejects production Crossterm `KeyEvent` payloads and raw fallback variants
under `src/app/components/`.

## Risks / Trade-offs

- **[The router and a leaf both act on one chord]** → The outcome is a single
  selection over one tick's messages, not two independent gates. Pinned by the
  `Command`-discards-leaf and `FallThrough`-keeps-leaf rows of the matrix.
- **[A typed replacement loses a side effect hidden in `handle_legacy_key`]** →
  Inventory each raw request's mutations (task 1.1) and add the corresponding
  targeted presentation push before removing that family.
- **[The blanket re-projection masks a mutation]** → The five `push_*_content`
  calls fire after *every* key today. Each unit replaces only the pushes its own
  handlers need; the inventory records which handler mutates which surface.
- **[A load-bearing precedence quirk is silently dropped]** → The
  `clear_queue_prompt_c` vs context-menu mutual exclusion (#135), the `Ctrl+a`
  enqueue-before-playback claim (#209), the `[`/`]` Queue-vs-Library meaning
  split, the `handle_lib_key` Ctrl/Alt catch-all swallow, the Space/Escape
  first-press fall-through, and the Ctrl+/ terminal-encoding ambiguity are each
  pinned by a matrix row (task 1.3) before any unit converts.
- **[Prompt removal is noticed as a regression]** → It is a deliberate,
  documented removal with a named re-add contract
  (`docs/architecture/mpv-owned-playback-prompts.md`) and a spec delta, not a
  silent drop.
- **[The remote-daemon prompt gap]** → Accepted and recorded. The remedy, if
  needed, is restoring the notification path, not the status-bar prompt.
- **[The missing Library parent blocks literal D7 ownership]** → Route only
  application-wide bindings through the router; keep selection-dependent
  behavior in the focused destination; leave the Library-parent decision to
  #607.

## Migration Plan

1. Record the exact raw-key producer/consumer and mutation matrix from current
   HEAD; treat it as the checklist for the serial units.
2. Land the router seam and the `Application::tick()` integration harness with
   policy still empty, so behavior is unchanged and the harness is trustworthy.
3. Remove the skip-intro/next-up prompts, with the spec delta and the
   architecture note.
4. Activate `key_policy.rs` as the router's policy and move the globals in.
5. Convert each remaining unit to component-local interpretation and semantic
   requests, deleting its raw forwarding and blanket re-projection as it reaches
   zero consumers.
6. Delete `GlobalViewKey`, the remaining raw `*Key` requests, `typed_key.rs`,
   `Model::handle_legacy_key`, `App::handle_key_with_home_context`,
   `CONTEXT_STACK`, obsolete handlers, and static-only scaffolding.
7. Run formatting, package checks and tests, Clippy, architecture gates, and the
   code-size gate; verify searches return no production legacy keyboard
   endpoint.

All steps are internal to one development branch. Before the final deletion,
rollback is a normal commit revert of the latest unit. After deletion, rollback
is the revert of the complete change so the old and new authorities are not
mixed.
