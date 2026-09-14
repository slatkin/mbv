## Context

See proposal.md — Why. The current `Application::tick()` returns the focused component's optional message before the permanently subscribed `UiRoot` observer's key message. `shell_run.rs` derives one `RouterOutcome` and `apply_router_outcome` either dispatches or discards the focused message. `RouterSnapshot` currently carries precomputed Space/Escape double-tap facts, while `command_for_policy` turns those facts into immediate playback commands.

`Msg` is both TuiRealm's outbound type and the typed cross-authority request vocabulary. A component that mutates only local state commonly returns `None`; mouse already has a framework-local `MouseClaimed` marker for the analogous post-mutation claim. `MediaList::delegate(input, pointer_target)` currently combines coordinate-bearing `RowLocalInput`, a separately supplied stable target, and an exclusive `RowLocalOutcome`.

The accepted constraints remain: one Keyboard Router, one terminal event per `PollStrategy::Once` tick, focused message before observer message, mouse eligibility before delivery, mounted-parent gesture recognition, presentation-owned current-frame point resolution, and no shell mirror of component-local interaction state.

This change is the architectural prerequisite for `add-media-list-multi-select`; that change's artifacts must be reconciled to consume these contracts rather than add `SelectionChanged` routing mirrors.

## Goals / Non-Goals

**Goals:**

- Make each key's policy candidate, leaf disposition, final disposition, and dispatched request independently inspectable in one central fold.
- Permit new component-local modes to consume context-sensitive keys without extending `RouterSnapshot`.
- Give shared media-list operations one result capable of describing multiple simultaneous consequences.
- Keep selection authority local while projecting only the focused list's summary to the Status bar panel.
- Preserve independent simultaneous Library and Queue selections and address delayed interactions back to their origin.

**Non-Goals:**

- Combining mouse subscriptions, gesture recognition, or hit testing with keyboard routing.
- Replacing TuiRealm's dispatcher, changing `PollStrategy::Once`, or supporting multiple terminal events in one arbitration batch.
- Generalizing every component interaction into one callback, command bus, or provider-erasing target type.
- Adding `Hash + Eq` to media-list targets or optimizing small linear selection scans without measurement.
- Changing existing key bindings, pointer gestures, context actions, or playback behavior outside the multi-select behavior separately specified by `add-media-list-multi-select`.

## Decisions

### D1 — Add one central semantic arbitration fold after TuiRealm delivery

Keep `UiRoot` as the sole Keyboard Router and replace the current direct `RouterOutcome` application with one named fold over the focused component result and router observation from the same tick. The fold produces a final disposition plus the requests to dispatch. It is the only place where a local claim can suppress a context-sensitive global candidate.

The fold captures focus before dispatch, preserves framework message order, asserts malformed combinations in debug builds, and exposes a pure truth-table seam for unit coverage. Real acceptance coverage still injects events through `Application::tick()`.

*Alternative:* let each component check global policy before handling a key. Rejected: this creates multiple routers and duplicates precedence.

*Alternative:* read component attributes or query local selection from the shell. Rejected: this mirrors local authority and makes routing depend on cached state.

### D2 — Represent leaf disposition independently from its optional request

Introduce a small internal leaf-key result with `Unhandled` or `Consumed` disposition and an optional existing `Msg`. Component-local key handlers return this result. At the TuiRealm boundary:

- consumed with a request emits that request and is classified as consumed;
- consumed without a request emits one framework-local key-claim marker;
- unhandled emits no focused message.

The central fold treats a focused typed request and the key-claim marker as consumed leaf results. Absence of a focused result is the defined unhandled representation at the TuiRealm boundary. The claim marker carries no local state and has no shell dispatch arm beyond explicit arbitration consumption.

This avoids recursively wrapping every existing `Msg` and avoids duplicating its Shell/Service/Queue/Playback request families while making the ambiguity explicit inside every key handler.

*Alternative:* wrap every `Msg` in an envelope carrying disposition. Rejected: it imposes persistent ceremony on user events and non-key messages that never participate in keyboard arbitration.

*Alternative:* add `SelectionChanged` messages to teach the router Visual state. Rejected: those messages make a presentation projection authoritative for input and require every destination to maintain the mirror correctly.

### D3 — Split immediate policy outcomes from deferred key candidates

Extend the router result vocabulary with a deferred playback candidate for context-sensitive Space/Escape double taps. Resolving policy does not mutate the candidate clock. The central fold applies this table:

| Router result | Leaf consumed | Final behavior |
|---|---:|---|
| Immediate command | either | run command; leaf must not own a global chord |
| Swallow | either | run nothing |
| Fall through | yes | keep local mutation/request |
| Fall through | no | no action |
| Deferred candidate | yes | keep local mutation/request; reset that candidate clock |
| Deferred candidate | no | advance the candidate clock; run only if completed |

Candidate clocks therefore advance only after arbitration proves the focused component did not consume the key. A consumed key resets its own clock so a later unhandled press cannot complete a sequence begun before or during the local mode.

Immediate global commands remain authoritative. Components must not mutate local state for their chords; existing routing-matrix coverage enforces that ownership. Blocking overlays retain `Swallow` semantics.

*Alternative:* keep precomputing `space_double_tap`/`esc_double_tap` before tick and cancel afterward. Rejected: cancellation cannot safely undo a fired command or explain whether the first press armed state.

### D4 — Separate pointer surface input from target-resolved list operations

Retain coordinate-bearing normalized pointer input at the mounted Panel/destination boundary because the Library panel delegates through type-erased destination owners. After the active presentation resolves current-frame geometry, translate it to a generic target-bearing media-list operation. Keyboard translation produces the same operation using the owner's current target and no coordinates.

`MediaList` receives operations such as movement, activation of current target, selection of a resolved target, context over current/resolved target, and future toggle/range selection. It does not receive raw terminal events, gesture timing, or coordinates paired with a second optional target.

*Alternative:* make `LibrarySlotEvent` generic over every destination target. Rejected: it breaks the type-erased Library content-owner boundary and moves destination identity into the Panel.

*Alternative:* keep `RowLocalInput` plus `pointer_target: Option<Target>`. Rejected: invalid input/target combinations remain representable and every caller must remember which variants require prior resolution.

### D5 — Return an orthogonal media-list transition

Replace exclusive `RowLocalOutcome` arms with one `MediaListTransition<Target>` containing independent facts:

- input disposition;
- optional selected-target change;
- optional read-only selection-summary change;
- optional provider-neutral external row intent.

The transition is produced atomically from one operation. Destinations handle generic disposition and summary facts uniformly, then exhaustively translate only target-bearing external intents. A transition may contain both local changes and an external intent; callers must not reproduce that by delegating a synthetic Click before DoubleClick/Context behavior.

The selection summary contains count and the minimum stable origin needed for cross-surface presentation; it never contains membership. Target vectors remain ordered `Vec<Target>` values. Keep `Clone + PartialEq` bounds and linear scans because canonical lists are small and list order is required.

*Alternative:* add more exclusive enum variants for every combination. Rejected: combinations multiply and force destination sequencing back into the API.

### D6 — Distinguish authoritative state, presentation projection, and delayed-action snapshot

There are three deliberately different values:

```text
MediaList selection       authoritative membership + anchor
Selection summary         current count/origin for Status bar presentation
Context action snapshot   origin + ordered resolved action values
```

The shell may cache the focused summary only to project Status bar content. It never pushes the summary into a list and the Keyboard Router never reads it. Focus synchronization selects the summary from the focused Panel/list; focus changes do not clear either selection.

A context menu receives an owned action snapshot when opened. It does not resolve targets again after overlay focus changes. Its `SelectionOrigin` addresses the Library destination/list or Queue list that produced it. Running or dismissing according to the existing menu policy sends a clear command to that origin only. Origin is coordination identity, not membership authority.

*Alternative:* infer origin from `PanelFocus` when an action runs. Rejected: overlays own focus while open and panel focus may change independently.

### D7 — Centralize generic transition handling at the nearest common owner

The Library panel's owner boundary handles generic disposition and focused-summary propagation once for Library destinations. Queue handles the same generic transition contract at its Panel boundary. Destination owners retain only exhaustive translation of stable target intents into provider-specific messages and coherent content-snapshot lookup.

Do not add closure-based generic target materialization. Browser, Home, Music, TV, Feeds, Audiobookshelf, and Queue retain different source snapshots and domain values; explicit translation is the debuggable authority boundary.

### D8 — Make diagnostics describe the arbitration chain

Debug output and test failures use one compact record containing the chord, captured focus, router result, leaf disposition, final disposition, and whether a request/command was dispatched. It carries no credentials, content payloads, or selection membership. Debug assertions reject duplicate router observations, multiple focused key claims, deferred candidates without their observer event, and claim markers escaping arbitration into ordinary shell dispatch.

This record is diagnostic data, not persisted telemetry or a new logging subsystem.

## Risks / Trade-offs

- [Converting key handlers touches many Interactive Components] → Introduce the leaf-result contract centrally, migrate component families in bounded commits, and keep routing-matrix rows green after each family.
- [A consumed marker could be mistaken for an ordinary request] → Keep it framework-local, consume it only in the arbitration fold, and assert if it reaches top-level request dispatch.
- [Changing double-tap clock timing causes regressions] → Characterize first press, second press, timeout, consumed press, overlay, text entry, and post-consumption reset through live ticks before replacing the old snapshot facts.
- [Library type erasure encourages coordinates to leak into `MediaList`] → Keep a named surface-input-to-target-operation translation boundary and prohibit coordinate-bearing operations below it.
- [Status summary becomes another owner] → Store no target membership, never push it back, and prove stale summaries cannot affect routing.
- [Menu target snapshots become stale during asynchronous effects] → Materialize against one coherent destination snapshot at menu-open/action preparation and preserve existing stale/missing-target reporting; never substitute current focus or cursor.
- [Overlap with `add-media-list-multi-select`] → Reconcile that change's proposal, design, and specs after this plan is accepted; implementation order is this change first, multi-select second.

## Migration Plan

1. Add characterization tests for existing immediate router outcomes, double-tap behavior, focused-message ordering, and representative local-only mutations through `Application::tick()`.
2. Introduce the leaf-key result and framework-local consumed marker; convert key-handler families while preserving current router behavior.
3. Introduce the central arbitration fold and diagnostics, initially mapping existing outcomes without changing double-tap timing.
4. Convert Space/Escape to deferred candidates, move clock mutation after leaf arbitration, and remove double-tap facts that existed only in the pre-arbitration snapshot.
5. Add target-resolved media-list operations and the orthogonal transition; migrate shared presentations, Library destination families, and Queue without changing visible behavior.
6. Add summary/origin plumbing required by multi-select, including independent Library/Queue retention and focused Status bar projection, but leave gesture/action behavior to `add-media-list-multi-select` where it is not already present.
7. Reconcile `add-media-list-multi-select` planning artifacts to remove state mirrors and consume these contracts before applying it.

Each migration step remains buildable and testable, but the change is not complete while compatibility outcomes, duplicate delegation sequences, or router-authoritative selection summaries remain. Rollback is by reverting the change before applying the dependent multi-select change; there is no persisted-data or protocol migration.
