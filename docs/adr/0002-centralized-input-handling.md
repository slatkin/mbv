# Centralized Input Handling

## Problem

Key handling is highly decentralized. `App::handle_key` (`src/app/input.rs`) is a
~300-line function whose branch order *is* the shortcut-precedence engine: nothing
else records which context wins over which, so precedence can only be reconstructed
by mentally executing the function. Help text in `render/overlays/help.rs` is
hand-written with no structural link to the handlers, so it drifts. The three large
view handlers (`handle_combined_key`, `handle_lib_key`, `handle_queue_key`)
re-implement the same "global" keys (`q`, `Tab`, `1`–`9`, `.`, `/`, `Ctrl+q`)
independently, and `handle_queue_key` is a second nested precedence engine that fakes
context by swapping `self.tab_idx` and hand-maintains an `is_lib_key` mirror of another
handler's key set. The mouse path (`handle_mouse`) has the same implicit precedence
problem spatially, yet already calls the shared `dispatch(Action)` seam for transport
controls — and has already drifted from the keyboard on a shared intent (double-click
a queue item vs. `Enter` on it dispatch different logic). New shortcut handling keeps
being added ad hoc because there is no single front door.

## Decision

Introduce a **context-aware command registry** as the single input authority. The core
is not "a key resolver" but a shared executor fed by two front-ends:

```
KeyEvent   --> chord resolver  --\
                                  >-- Command --> dispatch (shared executor)
MouseEvent --> region resolver --/          \-- Swallow / FallThrough (per context)
```

- **`Command`** — semantic intent (e.g. `TogglePlayPause`, `QueuePlayCursor`,
  `LibrarySearchStart`). A superset of today's `Action` enum. Dispatch stays "thick":
  it keeps the stateful/branchy execution (session-vs-local, scope/empty checks).
- **Chord resolver** — pure `resolve_key(snapshot, chord) -> KeyResolution`, where
  `KeyResolution` is `Command(Command) | Swallow | FallThrough`. Precedence is a
  **context priority stack**: an ordered, first-match list of active contexts, which is
  today's `handle_key` branch order made explicit and assertable.
- **Region resolver** — the mouse front-end. Emits the *same* `Command`s for
  command-like clicks (transport, context-menu, queue-play); genuinely spatial logic
  (hit-testing, double-click timing, drag-seek, hover) stays local, not forced into a
  binding table.
- **`InputSnapshot`** — the plain-data view of app state the resolver reads (~20 fields,
  one behind a lock). Keeping it a data snapshot (not `&App`) is what makes the resolver
  pure and testable, and it becomes the written-down answer to "what does input depend on?"

Contexts share one taxonomy across keyboard and mouse (`handle_mouse_panels` already
mirrors the keyboard overlay stack). Text-entry contexts (search boxes, the save-name
dialog) and modal state machines (save-playlist stages, settings sub-modes) are *routed
to* by the registry but are not expressed as bindings — they own local state and a
catch-all `Char` capture.

## Scope

- **In scope:** internal centralization of all keyboard and mouse input onto the shared
  `Command`/`dispatch` seam and the context-priority resolver; help rendered from the
  registry; elimination of duplicated global keys, the `tab_idx`-swap hack, and the
  `is_lib_key` mirror.
- **Out of scope (now):** user-configurable keybindings.
- **Not a flat global shortcut table.** mbv's shortcuts are context-sensitive; a flat
  table would lose precedence and the `Command`/`Swallow`/`FallThrough` distinction.

## Phased roadmap

Each phase is independently shippable and green; all converge on full centralization.
Tracked as separate issues under a GitHub Project.

1. **Vocabulary + resolver skeleton** — `Command`, `KeyChord` (incl. "any printable
   char"), `InputContext`, `KeyResolution`, `InputSnapshot`. Pure, unit-tested. No
   behavior change.
2. **Done (#131). Keyboard spine** — centralize keyboard precedence as a
   `CONTEXT_STACK` loop over the existing imperative handlers; characterization tests
   lock current behavior and quirks in place.
3. **Done (#132). Migrate view handlers** — collapse duplicated global keys; dissolve
   the `tab_idx`-swap and `is_lib_key` mirror. Behavior-preserving.
4. **Help from the registry** — render covered sections from binding data; converges as
   handlers migrate.
5. **Done (#134). Mouse onto shared `Command`** — region resolver emits shared
   `Command`s; fixes the queue `Enter`/double-click divergence. Spatial mechanics stay
   local.
6. **Done (#135). Quirk-fix pass** — each accidental precedence bleed-through fixed as
   a separate, labeled behavior change: `c`/`h` reaching past an open context menu,
   each gated with its own `context_menu.is_some()` check and a dedicated regression
   test (matching the guard `home_search` already used).
7. **Done (#136). Guardrail docs** — no raw shortcut handling outside the registry
   except text entry and external setup (e.g. login); see AGENTS.md's "Rules" and
   CONTEXT.md's "Input handling" section.

## Roadmap (not scheduled): user-configurable keybindings

Recorded as a future phase, not part of #129. Rationale: a user spans multiple terminals
and DE/WM global shortcuts, so bind conflicts are plausible and being able to remap is
genuinely useful. Because the registry makes bindings *data*, this becomes a later phase
that loads user overrides over the same table (with conflict detection, validation,
config migration, and help rendering of overrides) rather than a re-architecture — which
is precisely why it is safe to defer until the internal model is stable.

## Consequences

- Precedence becomes data you can read and assert on; help cannot silently drift for
  covered shortcuts.
- One front door for future shortcuts, which is the guardrail against re-decentralization.
- The `Command` vocabulary grows large (~40–50 commands) — that is the real surface area
  of the work, spread across phases.
- Full mouse and full help coverage *converge* across phases rather than landing at once;
  keyboard behavior has the strongest coverage so far (phase 2's characterization
  tests), with mouse verified per-region.
