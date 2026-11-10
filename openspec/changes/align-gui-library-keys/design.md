# Design: align-gui-library-keys

## Context

The GUI Library panel routes its keys in the window content: `crates/mbv-gui/ui/src/lib/library/keys.ts` and five per-destination siblings (`emby-keys.ts`, `shows-keys.ts`, `audiobooks-keys.ts`, `podcasts-keys.ts`, `feeds-keys.ts`). The front end hardcodes every chord it takes; only keys it forwards reach the shared Keyboard Router, where `[keys]` and the TUI's defaults apply (`src/app/input/key_policy.rs:728-760`). There is no chord carrier in `GuiFrame`, so the front end cannot honour a rebind.

The GUI-only core literals (`gui_collapse_panel` `z`, `gui_fold_queue` `q`, `gui_cycle_subtitle` `c`, `gui_right_column_toggle` `t`, `gui_queue_filter_open` `/`, `gui_open_target_chooser` `r`, `src/app/input/key_policy.rs:235-247`) are canvas-fixed key-policy entries gated `GuiClient`. They resolve ahead of the registry actions whose defaults they collide with, under any panel focus, so `q`, `z`, `c`, and `r` shadow the TUI's Library and Queue meanings inside the GUI.

The confirm modal's chord set exists twice: the TUI's Confirm component matches `Enter`, `Esc`, and `d` or `D` for the three-way dirty-playlist prompt (`crates/mbv-components/src/confirm.rs:195`), while the GUI overlay re-derives a narrower set that drops `D` (`src/app/gui_client_state/overlay.rs:203-233`). The confirm button's hint parser also exists twice: the TUI's `button_keys`/`key_from_hint` (`crates/mbv-components/src/confirm.rs:164-176`) and the GUI's `confirm_button_key` (`src/app/gui_client_state/confirm.rs:46-57`).

`TUI binding` in the tables below names the chord the TUI uses for the action, with the owning site. Leaf chords (the enqueue family, the context menu, the Selector-row cycle, the refresh) are leaf-local in `mbv-components` and are not registry actions, so `[keys]` cannot rebind them; a match means the same literal, not a read-through. Registry actions are rebindable, and the GUI reads the configured chord where the front end itself must match.

Permissions:

- User rule: "all shortcut keys should match unless specifically authorised."
- The user answered Yes to: propose keys for GUI actions that have no TUI key, mark each NEEDS APPROVAL, and do not implement before approval. That pass completed on 2026-11-10: the user approved all eight proposals as written (D3).
- The pending `add-gui-library-panel` change stays open. Its delta requirement bodies name the current GUI keys (`Ctrl K` hint, window keys). Until it archives, this change's deltas supersede those bodies at sync; this change does not edit its files.

Status vocabulary for the tables:

- **MATCH-BY-DEFAULT** — no change: every key reaches the core and the shared router resolves it identically.
- **ALIGN** — GUI work adopts the TUI chord or wires a missing chord; no user decision.
- **AUTHORISED** — the TUI uses the chord for another action in the same context, or the action has no TUI key: the user approved the chord, recorded per row in D3 as `AUTHORISED: <quote/date>`.

## Goals / Non-Goals

**Goals:**

- One chord per action, the TUI's, readable from `[keys]` where the TUI's binding is a registry action.
- One source for the confirm chord set and the confirm hint parser.
- A recorded authorisation trail for every GUI key that differs from the TUI.

**Non-Goals:**

- No TUI key, default, or behaviour change.
- No new dependency. The shared confirm module uses `mbv-ui-model` types only.
- No seek-chord redesign beyond moving `[`/`]` off the TUI's literal.
- No leaf chord becomes rebindable; that is a separate configurable-keybinds work item if the user asks for it.

## Decisions

### D1. Forward-first routing shrinks the front end

The front end keeps local interaction state only where the TUI has no counterpart, and forwards every other key. Three mechanisms replace today's hardcoded take:

1. **Router-owned keys — forwarded, no front-end chord matching.** `Tab`/`BackTab` (tab selection cycle by the `next_library_tab`/`previous_library_tab` policy entries), the digits (the `library_tab_jump` policy entry, whose payload is the pressed digit), `search_open` (the registry chord; `Ctrl+K` is dropped), and every core action that a moved GUI key leaves behind (`fold queue`, `collapse panel`, `target chooser`) go to the core. The App resolves each for the GUI: a `search_open` dispatch sets a GUI fact that the frame carries, and the front end's already-mounted Search field takes DOM focus; a digit for a destination the GUI does not present opens nothing, as today.
2. **Carried-chord keys — front-end-owned local actions.** Pane cycling and pressed-pill selection are front-end state (the `add-gui-library-panel` design D1), so their chords must reach the front end. The frame gains a `chords` list: zero or more rows of a registry action id and its configured chord, filled by the core from `Keybinds` for the ids the front end matches (`search_open` for the hint, and the new GUI-only action ids, approved 2026-11-10). A chord that `[keys]` rebinds re-carries on the next frame. A front-end route that names no row falls to `'core'`.
3. **Fixed-literal leaf chords — one front-end table.** The leaf-local TUI chords (`Ctrl+A` enqueue, `Ctrl+P`/`Ctrl+S` result actions, `.` context menu, `r` refresh, `[`/`]` Selector-row cycle) bind in one TS table beside `keys.ts`, one entry per chord and Library command, with the Rust anchor site in each comment. They are forwarding-only: the front end sends the Library command, command behavior stays the shared `App` paths. A regression vitest asserts the table sends the command its Rust anchor sends for the same chord.

*Alternative:* give the GUI a real leaf mount per destination so the shared router routes leaf chords too. Rejected: it reopens the ADR 0036 boundary the pending change settled.

### D2. Per-action alignment

GUI-only non-declinable core literals after D1 are exactly `t` (right-column toggle) and `/` (Queue filter): neither shadows a TUI chord.

| Action | TUI binding | Current GUI | Target | Status |
| --- | --- | --- | --- | --- |
| Play / pause | `Space` = `toggle_play_pause` (`registry.rs:342`) | `Space` with no focused pill forwarded | none | MATCH-BY-DEFAULT |
| Cursor movement | `j`/`k`/arrows/PageUp/PageDown/Home/End in the leaves (`music_content/owner.rs:425-451`) | same keys in `keys-shared.ts:27-34` | none | MATCH-BY-DEFAULT |
| Open / close a tree node | `Right` expand / `Left` `tree_move_left` (`music_content/owner.rs:180-190`) | same (`keys.ts:182-188`) | none | MATCH-BY-DEFAULT |
| Panel focus across panels | `Ctrl+Right`/`Ctrl+Left` (`registry.rs:514`, `:523`) | forwarded to core | none | MATCH-BY-DEFAULT |
| Tab-cycle tab selection | `Tab`/`BackTab` = `next_library_tab`/`previous_library_tab` (`registry.rs:218`, `:227`) | `Tab`/`Shift+Tab` cycle panes (`keys.ts:476-478`) | forwarded to core (D1 route 1) | ALIGN |
| Jump to a tab by digit | `1`–`9` = `library_tab_jump`, digit is the tab position (`registry.rs:236`, `src/app/input/key_policy/commands.rs:70-73`) | `1`–`7` by place in the carried tabs; a digit past them is swallowed (`keys.ts:88-93`, `:480-490`) | forwarded to core; digit is the position; a non-presented position opens nothing | ALIGN |
| Enqueue the cursor item | `Ctrl+A` on the leaves that bind it: Music (`music_content/owner.rs:472`), Emby (`emby_library_content/input.rs:25`), TV (`tv_content/keyboard.rs:23`); the book leaf's `Ctrl+A` multi-selects and its enqueue stays on the context menu (`book_content.rs`) | bare `a` sends add on Music, Home, TV, Emby, Audiobooks, Podcasts, and Feeds (`keys.ts:222-229`, per-destination siblings) | `Ctrl+A` sends the add command where the TUI leaf binds it (D1 route 3); the `a`-add on the destinations without a TUI enqueue (Home, Audiobooks, Podcasts, Feeds) is GUI-only and moves to row H | ALIGN |
| Refresh the destination | bare `r` per leaf (`music_content/owner.rs:409-413`, `tv_content/keyboard.rs:158-163`); `F5` global; `Ctrl+R` rescan per leaf | `r` reaches the core literal `gui_open_target_chooser` first (`key_policy.rs:245`) | drop the `r` literal; `r` sends the refresh command (D1 route 3); the chooser moves (row A) | ALIGN |
| Clear-queue prompt | `c` = `clear_queue_prompt_c` (`registry.rs:289`) | `c` reaches `gui_cycle_subtitle` first (`key_policy.rs:241`) | drop the `c` literal; `c` forwarded opens the confirm modal through the core prompt (D1 route 1). Subtitle stays on `z` | ALIGN |
| Quit | `q` = `quit` (`registry.rs:190`) | `q` reaches `gui_fold_queue` first (`key_policy.rs:239`) | drop the `q` literal; `q` forwarded quits with the Client-exit teardown (D1 route 1). The fold moves (row D) | ALIGN |
| Cycle subtitle | `z` = `cycle_subtitle` (`registry.rs:488`) | `z` reaches `gui_collapse_panel` first (`key_policy.rs:239`) | drop the `z` literal; `z` forwarded to the registry action (D1 route 1). Collapse moves (row E) | ALIGN |
| Cycle the Selector row | bare `[`/`]` per leaf cycle the letter or group selector (`music_content/owner.rs:420-431`, `emby_library_content/input.rs:150-155`, `tv_content/keyboard.rs:167-174`) | not bound; `[`/`]` reach the deep-seek registry chords (`registry.rs:385`, `:394`) | `[`/`]` cycle the focused pill row: the front end moves the focus and sends the same select-collection command a pill press sends (D1 route 3). Needs row F first | ALIGN |
| Play the cursor item | `Enter` activates then plays (`music_content/owner.rs:177`) | `Enter` plays and never picks (D7 of the pending change, `keys.ts:190`, `:385`) | keep the chord; authorise the effect (row J) | AUTHORISED |
| Close with `Esc` | `Esc` leaf back (`EmbyLibraryBack`, `TvBack`), Inline-Search close, and the router's deferred stop (`emby_library_content/input.rs:225`, `src/app/input/router.rs:117-121`) | consumed by the panel when a field, menu, or overlay is open, and consumed when nothing is open — the core's `Esc` never runs (`keys.ts:267-284`, comment at `:253-256`) | forward `Esc` when nothing panel-owned is open: leaf back / stop work under Library focus | ALIGN |
| Play next the cursor item | none — no TUI producer (pending design D8) | bare `n` (`keys.ts:222`) | keep `n` (row G) | AUTHORISED |
| Select the focused Collection pill | none — TUI pills have no select key | `Space`, `Alt+Space`, `Enter` (`keys.ts:295`, `:313`) | move off `Space` (play/pause) and `Enter` (activate/play); keep the pointer press (row B) | AUTHORISED |
| Pane focus cycle | none — TUI panels have no pane-cycle key; `Tab`/`Shift+Tab` belong to the tab selection | `Tab`/`Shift+Tab` (`keys.ts:476-478`) | move off the tab-selection chords (row A) | AUTHORISED |
| Open the playback-target chooser | none — GUI-only (canvas transport) | core literal bare `r` (`key_policy.rs:245`) | move off the refresh chord (row C) | AUTHORISED |
| Fold the Queue panel | none — GUI-only (canvas footer) | core literal bare `q` (`key_policy.rs:239`) | move off the quit chord (row D) | AUTHORISED |
| Collapse the Queue panel | none — GUI-only (pending design D4's rail) | core literal bare `z` (`key_policy.rs:239`) | move off the subtitle chord (row E) | AUTHORISED |
| Deep seek back / forward | `<`/`>` — the TUI's ±5 s chords; `[`/`]` are `seek_back_10`/`seek_forward_30`, `KeyGate::GuiClient` (`registry.rs:385`, `:394`) | `[`/`]` reach the deep-seek chords | retune the deep-seek defaults off the Selector cycle chords (row F) | AUTHORISED |
| Home row add / play next | none — the TUI Home keys are movement, `Enter`, `Ctrl+Enter`, `Ctrl+W`, `Delete`, `.` (`home_content/keyboard.rs:69-85`) | `a` and `n` per row (`keys.ts:392`, `:399`); `a` additionally collides with the `GuiClient`-gated `open_audio_popover` global | keep `a` and `n` on Home (row H) | AUTHORISED |
| Search field focus | `search_open` = `Ctrl+/`, `Ctrl+_` (`registry.rs:211`) | hardcoded `Ctrl+K` (`keys.ts:452-458`) | the configured chord; `Ctrl+K` dropped (D1 route 1) | ALIGN |
| Search hint text | shows the `search_open` chords in help (`crates/mbv-render/src/components/help.rs`) | literal `Ctrl K` in the field (`LibraryTopBar.svelte:197`) | the frame carries the configured `search_open` chord; the hint binds it | ALIGN |
| Search result play / shuffle / add | `Ctrl+P` / `Ctrl+S` / `Ctrl+A` on the result (`music_content/owner.rs:465-475`, `emby_library_content/input.rs:12-29`) | none; arrows and `Enter` only | the same chords on the GUI result rows: play, shuffle, and add through the search-activate command (`emby_library_content/input.rs:25` names `Ctrl+A` enqueue for the search results) | ALIGN |
| Search type-filter cycle | `Tab`/`BackTab` cycle the Search sidebar's type filter (`search_sidebar.rs:136-143`) | `Tab` falls to the browser default while the field is focused (`keys.ts:346-361`) | while the field is focused, `Tab`/`BackTab` cycle the shared type filter through a library command; the frame reflects the filter | ALIGN |
| Item context menu | `.` on the cursor row (`music_content/owner.rs:409`, `home_content/keyboard.rs:76`) | right-click only; `.` falls to core (`keys.ts:331`) | `.` opens the item menu for the cursor row | ALIGN |
| Confirm modal chords | `Enter`, `Esc`, and `d`/`D` when the action is the dirty-playlist prompt (`crates/mbv-components/src/confirm.rs:195`) | `Enter`, `Esc`, and bare `d` only — `D` dropped (`src/app/gui_client_state/overlay.rs:203-233`) | one shared set in `mbv-ui-model::confirm`, used by the TUI component, the GUI overlay branch, and the shared dispatcher | ALIGN |
| Confirm button hint | `button_keys` splits `/`-separated tokens (`crates/mbv-components/src/confirm.rs:164-176`) | `confirm_button_key` reads one character only (`src/app/gui_client_state/confirm.rs:46-57`) | one shared parser in `mbv-ui-model::confirm`; the GUI's press path uses it | ALIGN |
| Queue row move | Queue surface `Shift+Up`/`Shift+Down` | `Alt+J`/`Alt+K`, `KeyGate::GuiClient`, registry-declared (`registry.rs:301`, `:311`) | none — declared divergence with its own gate | MATCH-BY-DEFAULT |
| Right-column toggle, Queue filter | no TUI counterpart | core literals `t`, `/` (`key_policy.rs:243`, `:247`) | none — neither shadows a TUI chord | MATCH-BY-DEFAULT |

### D3. Authorised chords for the approval rows

Each candidate avoids every TUI chord in `KEYBIND_ACTIONS`, every surviving GUI core literal, the `alt_swallow` layer (which swallows every `Alt` chord, so no `Alt` candidate), and the Queue surface's `Shift+Up`/`Shift+Down` row moves. The user approved all eight proposals as written on 2026-11-10, answering "yes" to "Do you approve all eight as proposed?".

| Row | Current chord | Why it must move | Candidate | Authorisation |
| --- | --- | --- | --- | --- |
| A. Pane focus cycle | `Tab` / `Shift+Tab` | the TUI's tab-selection chords | `Shift+Right` / `Shift+Left` | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (`Shift+Right` / `Shift+Left`) |
| B. Select the focused Collection pill | `Space`, `Alt+Space`, `Enter` | `Space` is play/pause, `Enter` is activate/play, `Alt` chords are swallowed | `l`; `;` as the fallback | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (`l`) |
| C. Open the playback-target chooser | `r` | the TUI's leaf refresh | `y` | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (`y`) |
| D. Fold the Queue panel | `q` | the TUI's quit | `f` | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (`f`) |
| E. Collapse the Queue panel | `z` | the TUI's cycle-subtitle | `w` | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (`w`) |
| F. Deep seek back / forward | `[` / `]` | the TUI's Selector-row cycle | `Shift+[` / `Shift+]` | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (`Shift+[` / `Shift+]`) |
| G. Play next (Library) | `n` | no TUI key — free of every registry chord and core literal | keep `n` | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (keep `n`) |
| H. Add / play next where no TUI key exists (Home, Audiobooks, Podcasts, Feeds) | `a` / `n` | no TUI key on those destinations; `a` collides with the `open_audio_popover` GuiClient literal for the Queue playback's audio chip, which has no Library surface | keep `a` and `n`; `Shift+A` is the fallback if the user rejects bare `a`. A kept `n` also needs the play-next wire for the destinations whose `n` sends no command today | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (keep `a` and `n`) |
| J. Enter plays without picking | `Enter` | the TUI's Enter activates first; the same chord, a different effect (pending design D7) | keep the declared divergence, or adopt Activate — the user decides | AUTHORISED: user, 2026-11-10, "yes" to the eight proposals (keep the GUI's D7 divergence: `Enter` plays without picking) |

Rows A–F become registry actions with `KeyGate::GuiClient` through the *Add a keybind action* recipe (`docs/recipes.md`), so `[keys]` can rebind them and their defaults can never shadow a TUI surface again. Row A's policy entry carries no App command: its dispatch is a no-op that reports handled, and the front end reads its chord from the carried `chords` list (D1 route 2). Rows B and G–H stay front-end-owned; the frame carries their chords the same way once they are registry actions.

### D4. The GUI-literal set shrinks to two entries

After D2's rows, the canvas-fixed literal set drops `gui_collapse_panel` (`z` → registry action, row E), `gui_fold_queue` (`q` → registry action, row D), `gui_cycle_subtitle` (`c` — deleted; `z` already covers subtitle cycling and `c` keeps the clear-queue prompt through the registry action's gate), and `gui_open_target_chooser` (`r` → registry action, row C). `gui_right_column_toggle` (`t`) and `gui_queue_filter_open` (`/`) stay as literals because no TUI chord claims them. A later user rebinding of a TUI chord onto `t` or `/` still loses the literal, because these entries are canvas-fixed rather than registry-backed; that is today's documented GUI-only key context and not this change's scope.

### D5. One confirm chord module

`mbv-ui-model::confirm` gains the two shared facts:

1. `ChordSet`: for a `ConfirmAction`, the `Key` chords that answer it — `Enter`, `Esc`, and `d` with and without shift for the dirty-playlist prompt. The TUI's `confirm_intent_for_key` becomes a thin call over it, and the GUI overlay's `GuiOverlay::Confirm` branch replaces its inline match with it, which restores Shift+D in the GUI.
2. The hint parser: `button_keys`' `/`-split and single-character read move there as one function the GUI's press path calls in place of `confirm_button_key`.

*Alternative:* keep the GUI set private and add the missing `D` arm. Rejected: the copy is the defect; three call sites need the same contract.

### D6. Esc ownership under Library focus

`routeEscapeKey` answers only when a panel-owned surface is open: the row menu, the picked item's menu, the Hero overlay, the filter field, the Search field. When it finds nothing to close it returns `'core'`, so the leaf's back, the Inline-Search close in a leaf that owns one, and the router's deferred stop run. This retires the current rule that an `Esc` with nothing open holds the keyboard; the field-close and step-back orders stay as the TUI's (close first, back on the next `Esc`).

### D7. New TUI-chord answers inside the GUI frame

- The refresh command: the App arm runs the refresh path the TUI leaf's `EmbyLibraryRefresh` request runs, minus the Model participant, and lands the same `App` writes; the frame's revision re-carries the content.
- The search result family: the GUI result row carries the shared result identity; `Ctrl+P`/`Ctrl+S`/`Ctrl+A` send the existing search-activate command per action the Music leaf's search-result request names.
- The type-filter cycle: a library command moves the shared sidebar's `type_filter` by one step; the frame's search section reflects it, and the results head shows the active filter.
- The context menu: `.` sends the row-menu command the right-click sends.

### D8. Digit semantics

The digits stop naming the carried tabs' places and resolve as tab positions through the shared `library_tab_jump` command (Home is position 0, so the first presented destination after Home takes `1`). A position the GUI does not present opens nothing. The pending change's "second destination tab after Home" scenario stays true, because the position order and the presented-tab order agree from `1` after Home.

## Risks / Trade-offs

- [Approval latency gates part of the change] → The ALIGN and MATCH rows ship without any approval row; the approval rows are staged last in `tasks.md`. A rejected candidate costs one key move, not one work item.
- [Two open changes touch the same gui-client requirement] → `add-gui-library-panel` modifies "GUI keyboard interaction matches the TUI Client" too. This change's delta supersedes its body at sync; the design note in D2's Esc row records both orders.
- [The literal-leaf table can drift from the Rust leaves] → Each entry's comment names the Rust anchor; a vitest asserts one command per chord against the anchor's request. The recipe gains the GUI aside so future key work repeats the check.
- [Dropping `Ctrl+K` removes a key users may have learned] → The chord was shipped in a change that is still open, so its user base is the live-test round; the user's approval covers the drop.
- [The `Search-activate` result row and the Music leaf's `Ctrl+A` do not agree everywhere in the TUI] → The GUI result rows follow the Music leaf's request shapes, which the pending change's Search section already resolves; the risk runs only where a TUI leaf hides enqueue behind the context menu, and the note in the D2 table names it.

## Migration Plan

1. Shared chord modules and the throw-away fixes that need no approval: the confirm module (D5), the Search hint (D2 search rows), the alignment rows with fixed literals (D1 route 3, D2 rows marked ALIGN), the routing table and the literal shrink (D4).
2. Gate: the user's authorisation pass over D3 — done 2026-11-10; all eight proposals approved as written.
3. The approval rows: the registry actions with their authorised defaults, the carried-chord list, and the front-end routes for local actions.

Rollback is a per-row revert; no stored state changes in any row.
