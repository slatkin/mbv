# Power View Becomes The Only View (issue #361)

**Issue:** [#361](https://github.com/slatkin/mbv/issues/361) — "Make the 'power view' the standard view and remove 'standard view'"
**ADR:** 0013, reserved 2026-07-24
**Recommended agent model:** Sonnet for commits 1–2. Commit 3 (docs) is pre-written verbatim in this plan — apply it, do not re-derive it.

**Goal:** Delete the Standard (tab-based) view entirely. Power View becomes the one and only view, `ViewMode` ceases to exist, and the `power` vocabulary is renamed to layout-independent domain names in the same PR.

**Architecture:** `render()` (src/app/render/mod.rs:175) currently branches on `self.view_mode` between `render_power_view` and an inline Standard block. That branch collapses to the Power arm unconditionally. Power's render subtree (`src/app/render/power/**`, ~11k lines) is fully self-contained — verified: all 10 public helpers in `render/library/**` have **zero** callers outside that subtree, and no file under `render/power/` references `render_home_*`, `render_combined`, `render_queue_panel`, `render_library*`, or the `library_*` meta helpers. So `render/home.rs`, `render/playlist.rs`, and `render/library/**` delete wholesale rather than needing to be untangled.

**Tech stack:** Rust, ratatui (TUI), existing `mbv` app crate conventions. Use Serena for all exploration and edits (`rename_symbol` for the rename commit — it is reference-aware and atomic).

---

## Decisions already made — do not relitigate

These were settled in a grilling session on 2026-07-24. If implementation reveals one of them is *impossible*, stop and report; do not silently substitute a different answer.

1. **No fallback for terminals without image support.** Power already guards every image call behind `images_enabled()` (`power/card.rs:140,168`, `power/detail.rs:236,269,452,506`, `power/home.rs:926`). Delete the force-to-Standard branch at `render/overlays/settings.rs:64`. Images-off is an explicit acceptance criterion, not an afterthought.
2. **The left column is the queue surface, full stop.** No queue-maximize mode, no `g` grouping toggle ported over. Power's queue is always grouped.
3. **`start_on_queue` is deleted outright** — F2 row, `Config` field, TOML read/write, `startup_tab_idx()`. It is redundant with restart-persistent panel focus.
4. **Rename lands in this PR**, as its own isolated commit (commit 2).
5. **Vocabulary is queue-side / library-side**, not left/right. See the rename table below.
6. **prefs.json migrates**: read the old key as a fallback, write the new one.
7. **library_position_state.json discards both scopes** and resets to root.
8. **The tab-scroll bug is fixed in scope.**
9. **`v` and `g` are left unbound.** Do NOT implement the audio visualizer — that is issue #98.
10. **Image cache constants AND values are renamed**, accepting a one-time re-download.

---

## Scope boundaries

**Does NOT cover:**
- The audio visualizer (#98). `v` becomes unbound and stays unbound.
- Any change to Power View's own layout, behaviour, or visual design. This is a deletion plus a rename; Power looks and behaves exactly as it does today, minus the bugs listed in Task 5.
- The `[` / `]` handlers at `input.rs:1302,1452,1484,1492,1504,1651` — those are Power's letter-pill / music-group / feed-group bindings and must survive untouched. Only the Standard queue-scope handler at `input.rs:1718,1726` is deleted.
- `render/overlays/**` beyond the specific edits named in Task 4. Overlays are shared by both views today and stay.

---

## Commit structure

Three ordered commits in one PR. **Commit 1 must build clean and pass the full suite on its own before commit 2 begins.** Do not squash them — bisect resolution across this change is the entire point.

**One agent per commit, not one agent for all three.** Each commit is handed to a fresh agent with clean context, and hands off through the worktree, not through conversation. This was corrected mid-flight: commit 1 generates a large compile-error cascade as a byproduct (`tab_idx` alone has 333 references), and an agent that grinds through that arrives at the rename and the verbatim ADR application with heavily degraded context — exactly backwards, since those are the precision-critical steps. The state the next commit needs is on disk; the instructions are in this plan. Neither requires continuity of context.

| Commit | Contents | Character | Agent |
|---|---|---|---|
| 1 | Delete Standard, rewire tab scrolling, migrate persisted state | Behavioural — needs semantic review | Sonnet, fresh |
| 2 | Mechanical rename, `power` → domain vocabulary | Zero behaviour change — reviewable by skimming | Sonnet, fresh |
| 3 | `CONTEXT.md`, ADR-0013, `README.md`, help panel copy | Docs | Author of the appendices |

Each agent's entry condition is the previous commit **committed, building clean, clippy-clean, and green**. Verify that before starting, and stop if it does not hold.

---

## Task 1: Delete the Standard render subtree

**Files:**
- Delete: `src/app/render/home.rs`, `src/app/render/playlist.rs`
- Delete: `src/app/render/library/` (entire directory — `mod.rs`, `album.rs`, `season_grid.rs`, `table/{mod,row,meta,context}.rs`)
- Modify: `src/app/render/mod.rs` — collapse the `match self.view_mode` at line 187 to the Power arm unconditionally; drop the `mod home; mod playlist; mod library;` declarations
- Modify: `src/app/layout.rs` — delete `LayoutHome`, `LayoutQueue`, `LayoutLibrary` and their `AppLayout` fields

**Watch for:** `LayoutHome` is referenced by *Power's* home code path only through `AppLayout::home`, which is the **Standard** home layout — `LayoutPowerHome` is the Power one (`layout.rs:70-75` documents the distinction explicitly). Do not delete the wrong one.

`render_tabs` (`render/mod.rs:608`) and `render_player_panel` (`render/mod.rs:775`) are called from **both** arms — keep both, delete only the Standard call sites.

- [ ] Delete the three render trees and their `mod` declarations
- [ ] Collapse the `render()` view_mode match to the Power arm
- [ ] Delete the now-orphaned layout structs and `AppLayout` fields
- [ ] `cargo build` clean — expect a large cascade of unused-import and dead-code errors; fix them by deleting, never by `#[allow(unused)]` (repo convention)

---

## Task 2: Delete `ViewMode` and the Standard-only state

**Files:** `src/app/mod.rs`, `src/app/actions.rs`, `src/app/input.rs`, `src/app/settings.rs`, `src/app/render/overlays/settings.rs`, `crates/mbv-core/src/config.rs`, `src/login.rs`

Fields to delete from `App` (line numbers are pre-change):

| Field | Line | Note |
|---|---|---|
| `view_mode: ViewMode` | mod.rs:1099 | plus the `ViewMode` enum at mod.rs:67 |
| `tab_idx: usize` | mod.rs:1043 | **333 references** — the bulk of this task |
| `pre_power_tab: usize` | mod.rs:1102 | only exists to restore `tab_idx` on Standard re-entry |
| `queue_group: bool` | mod.rs:1103 | Power is always grouped |
| `home_card_view: bool` | mod.rs:1111 | Standard Home's card/list toggle |
| `start_on_queue: bool` | mod.rs:1290 | + `Config.start_on_queue`, core config.rs:25/103/795/1131 |
| `view_mode: String` | mod.rs:1293 | + `Config.view_mode`, core config.rs:77/123/820/1082 |

Functions to delete: `set_view_mode` (actions.rs:3541), `save_config_view_mode` (actions.rs:3568), `startup_tab_idx` (mod.rs:402), `set_tab` (actions.rs:3512), `lib_tab_offset` (input.rs:109), `SettingKey::ViewMode` + `SettingKey::StartOnQueue` and their `setting_label`/`setting_value`/handler arms.

**`set_tab` is subtle.** It currently does three things (actions.rs:3512-3532): sets `tab_idx`, calls `ensure_tab_visible`, and activates a library position scope. The third behaviour must survive — it moves into the `library_tab` setter path (see Task 5). Read it carefully before deleting.

- [ ] Delete the fields, enum, and functions above
- [ ] Rewire every `tab_idx` reference — most become unconditional, some become `library_tab`
- [ ] Delete the two `SettingKey` variants and every match arm across settings.rs / overlays/settings.rs
- [ ] Delete the `Config` fields and their TOML read/write in `crates/mbv-core/src/config.rs`
- [ ] `cargo build` clean

---

## Task 3: Collapse `LibraryPositionScope`

**Files:** `src/app/mod.rs` (enum at :617), `crates/mbv-core/src/config.rs` (`LibraryViewPositions` at :542)

`LibraryViewPositions { default: Option<LibraryPosition>, power: Option<LibraryPosition> }` collapses so that `LibraryPositionState.libraries` becomes `HashMap<String, LibraryPosition>` directly. Delete the `LibraryPositionScope` enum and drop the scope argument from `activate_library_position_scope` and every caller.

**Critical:** an existing user's on-disk `library_position_state.json` holds the nested shape. Deserialising `{"default":{...},"power":{...}}` into a bare `LibraryPosition` will **fail**, and that failure must not poison the whole load. Ensure the loader degrades to an empty map (log at `warn`, return `Default::default()`) rather than propagating `Err` or panicking. Per decision 7, no attempt is made to salvage the old positions — the user starts at library roots once, then positions repopulate normally.

- [ ] Flatten `LibraryViewPositions` out of the state struct
- [ ] Delete `LibraryPositionScope` and de-parameterise its callers
- [ ] Add a test: a legacy nested JSON file loads as empty without error
- [ ] Also update `CONTEXT.md`'s Library-position entry in commit 3 — the "completely isolated scopes" clause dies here

---

## Task 4: Migrate prefs, and clean up the settings/help surfaces

**Files:** `src/app/mod.rs` (:1891-1899 load), `src/app/input.rs` (:2394-2397 save), `src/app/render/overlays/settings.rs`, `src/app/render/overlays/help.rs`

**prefs.json** — read new key, fall back to old, write new only:

| Old key | New key |
|---|---|
| `power_left_width` | `queue_column_width` |
| `power_left_tab` | `library_tab` |
| `power_focus` | `panel_focus` (values `queue_side` / `library_side` unchanged) |
| `tab_idx` | *deleted, not migrated* |

Add a comment at each fallback noting it can be deleted a release later.

**Settings panel:** delete the `ViewMode` and `StartOnQueue` rows. At `overlays/settings.rs:60-67`, the `ImageProtocol` handler currently forces `home_card_view = false` and `set_view_mode(Standard)` when the protocol is switched off — both die; the handler keeps only the `image_protocol_enabled` update.

**Help panel** (`overlays/help.rs`): delete `mk("v", "Toggle view")` and `mk("g", "Toggle grouping")` (:87-88). The section-ordering logic at :114-116 keys off `tab_idx` (`is_lib` / `is_queue` / `is_home`) — rewire it to key off `panel_focus` and `library_tab` instead: queue side focused → queue section first; library side on Home → home section first; library side on a library → library section first.

- [ ] prefs read-with-fallback + write-new
- [ ] Delete the two settings rows and the ImageProtocol side effects
- [ ] Delete the two help rows, rewire help section ordering
- [ ] Test: a prefs.json containing only old keys yields the correct in-memory values, and the next save emits only new keys

---

## Task 5: Fix the tab bar scroll (in scope — this is a real bug)

**Files:** `src/app/input.rs` (:106 `tab_count`, :1982 `visible_tab_range`, :2001 `ensure_tab_visible`, :3042 mouse scroll), `src/app/render/mod.rs` (:665-760 `render_tabs`), `src/app/actions.rs` (:5380 `power_left_tab_next`, :5395 `..._prev`)

Today `tab_scroll` / `ensure_tab_visible` / `visible_tab_range` are driven **only** by `tab_idx`, and `tab_count()` returns `2 + libs.len()` (the Standard count). Meanwhile `render_tabs`'s Power branch builds its name list from **all** libraries with no `vis_start..vis_end` slicing, while still drawing `«` / `»` indicators computed from that mismatched state. With enough libraries to overflow, Power's tab bar clips at the right edge, shows arrows that lie, and can strand the selected tab permanently off-screen. Standard's branch is the only correct implementation and it is being deleted.

- [ ] `tab_count()` → `1 + libs.len()` (Home + libraries; no Queue tab)
- [ ] Repoint `tab_scroll` / `ensure_tab_visible` / `visible_tab_range` at `library_tab`
- [ ] Slice the surviving `render_tabs` name list by `vis_start..vis_end` (port the Standard branch's slicing into the single remaining branch)
- [ ] Call `ensure_tab_visible()` from `library_tab_next` / `library_tab_prev`
- [ ] Test: with more libraries than fit, selecting the last one scrolls it into view and the arrows reflect actual scroll state

---

## Task 6: Deal with the 22 `ViewMode::Standard` tests

Each needs an individual judgment call — do not batch-delete and do not batch-rewrite:

- **Testing Standard behaviour** → delete the test.
- **Asserting something does *not* happen outside Power** → the premise evaporates. `input_resolver.rs:714 h_does_nothing_outside_power_view_via_handle_key` is the clearest example: "outside Power View" no longer exists as a state. Delete it, but check whether it was the only coverage of the guard it exercised — if so, replace it with a test of whatever guard survives (e.g. that `h` still does nothing while a context menu is open, which `:722` already covers).
- **Using Standard incidentally as setup** (e.g. `mod.rs:9664,9680,9766,…` setting `view_mode` just to reach some other state) → drop the line, keep the test.

Sites: `input_resolver.rs:716`; `mod.rs:6616, 7012, 9664, 9680, 9766, 9783, 9831, 9932, 9949, 9977, 10991, 11028`; plus the assertions in `mod.rs:4283-4295` (`startup_tab_idx`) and `:6937-7010` (`pre_power_tab` seeding), which test deleted functions outright.

- [ ] Classify and handle all 22
- [ ] `cargo test` fully green — 921 tests today, expect a modest net reduction

---

## Task 7 — COMMIT 2: the mechanical rename

**Do not start until commit 1 is committed, building clean, and green.**

Use Serena's `rename_symbol` throughout — it is reference-aware and atomic. Trust it; do not re-read files or re-run the suite to confirm a rename propagated.

| Today | Rename to |
|---|---|
| `src/app/render/power/*.rs` (9 files) | move up to `src/app/render/*.rs`, filling the slots the deleted Standard files vacated |
| `render_power_view` | fold into `render()` |
| `PowerFocus::{Queue, Left}` | `PanelFocus::{Queue, Library}` |
| `power_focus` | `panel_focus` |
| `power_left_width` | `queue_column_width` |
| `power_left_collapsed` | `queue_column_collapsed` |
| `power_left_tab` (+ `_pending`) | `library_tab` (+ `_pending`) |
| `LayoutPower` | `LayoutMain` |
| `LayoutPower.left_*`, `PowerLeftRowTarget` | `library_*`, `LibraryRowTarget` |
| `LayoutPowerHome` | `LayoutHome` (the name is free again) |
| `power_home_cursor` / `power_home_scroll` | `home_cursor` / `home_scroll` |
| `power_queue_scroll` | `queue_scroll` |
| `POWER_LEFT_FOCUSED_BG` / `POWER_RIGHT_BG` | resolve per call-site — see below |
| `IMAGE_CACHE_SUFFIX_POWER_PRIMARY` (`"P"`) | `IMAGE_CACHE_SUFFIX_CARD_PRIMARY` (`"card"`) |
| `IMAGE_CACHE_SUFFIX_POWER_ALBUM` (`"pwr_al"`) | `IMAGE_CACHE_SUFFIX_ALBUM_CARD` (`"album_card"`) |
| `IMAGE_CACHE_SUFFIX_LIBRARY` (`"lib"`) | **delete** — nothing writes it after Task 1; drop its two reads at `mpris.rs:99,104` |
| `POWER_CARD_PLACEHOLDER_KEY` / `_BYTES` | `QUEUE_CARD_PLACEHOLDER_*` |
| input-resolver context names (`power_left_width`, `power_lib_search`, `power_sidebar_toggle_h`, `power_album_track_mode`, …) | drop the `power_` prefix |

**Why queue-side / library-side and not left/right:** `left` currently means two opposite things. `power_left_width` / `power_left_collapsed` mean the **queue** column, while `LayoutPower.left_area` / `left_row_map` / `left_sorted_indices` / `PowerFocus::Left` mean the **library** side — `mod.rs:1394`'s own comment reads `Left, // right panel (library browser)`. `CONTEXT.md:262` pre-authorised the fix: *"visually, Power View has a queue side and a library side, and those are the user-facing concepts."* Layout-independent names also survive a future layout flip.

**`POWER_LEFT_FOCUSED_BG` / `POWER_RIGHT_BG` need per-call-site judgment**, not a blind rename — the existing names are unreliable given the `left` collision. Read each usage and name for what it actually paints.

**Image cache values change**, so every cached image is orphaned and re-downloads once on first launch. This is accepted (decision 10). Note it in the PR description so it isn't mistaken for a regression.

**Highest-risk file: `src/app/render/power/mod.rs`** — 3427 lines, 288 `power` mentions, the densest collision surface in the codebase. Do this file deliberately.

- [ ] Rename symbols via `rename_symbol`
- [ ] Move the 9 files up a directory, update `mod` declarations
- [ ] Rename the prefs keys' *writers* to match Task 4's new names (readers already handle both)
- [ ] `cargo fmt`, `cargo build`, `cargo test` — all green, **zero behaviour change**

---

## Task 8 — COMMIT 3: docs

Apply the pre-written content in the appendices below verbatim. It encodes rationale from the design session that cannot be reconstructed from the diff.

- [ ] Create `docs/adr/0013-power-view-is-the-only-view.md` (Appendix A)
- [ ] Apply the `CONTEXT.md` edits (Appendix B)
- [ ] `README.md`: the screenshot alt text at :11 says "mbv power view" — reword to drop the contrastive framing. Leave the `assets/screenshot-power.png` *filename* alone; renaming assets is churn for no reader benefit.
- [ ] Add a note to `docs/adr/0009-v-key-controls-audio-visualizer.md` (Appendix C)
- [ ] Comment on issue #98 that #361 narrowed its scope (Appendix D)

**Note:** the "Verification gate — BLOCKING, before push" section in the original version of this plan was replaced mid-flight. Visual verification is the user's, done manually; the agent runs automated checks only. See "Verification" below.

---

## Verification

**The implementing agent does automated checks only.** It must not launch the app, use the `/run` skill, or attempt any interactive TUI verification, and must never claim or imply a visual check.

- [ ] `cargo fmt`
- [ ] `cargo clippy` — zero warnings; fix by deleting dead code, never `#[allow(unused)]`
- [ ] `cargo test` — green

**The visual sweep is the user's, done manually.** This is deliberate: 921 tests will stay green through a rename that quietly drops a render call, so the suite is a strong net for the rename and a weak net for the visual result. A background agent driving an interactive TUI is not a credible substitute for a human looking at it.

The agent's final report must include a **ranked, specific list of what to look at hardest**, derived from what it actually changed — surfaces where its diff could plausibly break rendering without breaking a test. That list is the handoff, not the generic sweep below.

Reference surfaces for the manual sweep:

- Home (continue-watching + latest sections)
- A movies library — list, letter pills, inline detail banner
- A TV library — series inline detail, season/episode selection
- A music library — group levels, album drill-down, inline album track selection
- **Podcasts** — flagged fragile in prior work; it rides on the sidebar shell
- Queue column — card, scope pills, grouping, `h` collapse/expand, `Shift+←/→` resize
- Tab bar with more libraries than fit — arrows correct, selected tab always reachable (Task 5)
- **Image protocol off** — set `image_protocol = none` in F2 and sweep the above again. As far as anyone can tell this has never been visually verified in Power View, and after this change it is the only thing a no-image terminal can render. Highest-risk item.
- Overlays over the new layout: F1 help, F2 settings, F3 sessions, F4 playlists, context menu
- `v` and `g` do nothing, silently, everywhere

---

## Ground rules

- Isolated worktree branch from `origin/main`. Never commit to `main`, never merge, never rebase `main`.
- **Do not open a PR** unless explicitly asked. Do not `git push` without asking. Do not merge or self-merge under any circumstances.
- No `Co-Authored-By` trailer in commit messages.
- Stop when this plan's tasks are done. Do not self-assign follow-on work.
- If a decision in this plan turns out to be impossible, **stop and report** — do not substitute your own answer.

---

## Appendix A — `docs/adr/0013-power-view-is-the-only-view.md`

```md
# Power View Is The Only View

mbv had two full UIs: a tab-based Standard view and Power View, a two-column layout with a persistent queue column beside a library browser. Power View had become the one that was actually developed and used, while Standard survived mainly as the automatic fallback for terminals with no image protocol. We deleted Standard entirely (~2600 lines of render code, the `ViewMode` enum, `tab_idx`, and the `start_on_queue` setting) and made Power View unconditional.

## Considered Options

- Keep Standard as an automatic no-images fallback. Rejected because Power View already guards every image call behind `images_enabled()` and degrades to text and placeholders on its own, so the fallback bought a second full UI to maintain in exchange for a graceful degradation we already had. The cost was ongoing: every feature had to be built, tested, and visually verified twice, and in practice the Standard path was the one that silently rotted.
- Build a dedicated text-only layout that reclaims the card area when images are off. Rejected for now as speculative — nobody has reported the placeholder layout as a problem. Worth revisiting if images-off turns out to be a real usage mode rather than a theoretical one.
- Make Power the default but leave Standard reachable. Rejected because it is not a decision, it is a deferral: the maintenance cost stays and the ambiguity about which view is canonical stays with it.

## Consequences

There is no fallback UI. If Power View breaks on some terminal, mbv is broken on that terminal — there is no second path to fall back to. Images-off is therefore a first-class supported configuration that must be visually verified on changes to the render tree, not an edge case.

Because there is no longer a second view to contrast against, "Power" stopped being meaningful as a name and the vocabulary was renamed in the same change: queue side and library side, per `CONTEXT.md`'s existing guidance. The old names had also drifted into contradiction — `power_left_*` meant the queue column while `PowerFocus::Left` meant the library side.

Library positions were previously kept in two isolated scopes, one per view. That collapsed to one. The saved positions from both scopes were discarded rather than merged: any merge rule would have been arbitrary, and the cost of being wrong is one browsing session that starts at a library root.

`v` and `g` became unbound. `v` stays reserved for the audio visualizer (ADR 0009) rather than being reused.
```

## Appendix B — `CONTEXT.md` edits

These entries make claims that this change falsifies. Rewrite, do not delete — the concepts survive, their contrastive framing does not.

- **`:233` Library position** — delete "Default library view and Power View are completely isolated scopes with independent saved positions" and the `_Avoid_` clause's "one shared position across default and Power View, or bootstrapping one view's position from the other". One library, one position. `:234`'s "active view only" / "other view's saved position remains isolated" clause goes with it.
- **`:242` music album index** — "Standard Library and Power View search the same local album-only corpus" → one view searches it. Same for `:243`'s `_Avoid_` clause "sharing Standard and Power navigation state during activation".
- **`:221-222` album art** — "Standard Library album rows/detail, Home/Power album cards, and Power inline album detail must all preserve this rule" → drop the Standard surfaces. `:222`'s "as in Standard Library folder rendering" fallback sentence describes deleted code; remove it.
- **`:251-258` Power View left column** → retitle **Queue column**. Drop "while Power View is active" qualifiers throughout — there is no other state. `:257`'s "Power View is a formal view setting, not a transient keyboard toggle… The ordinary non-Power View app state is the default view" is entirely obsolete: replace with a note that there is one view and no view setting. Keep the `v`-is-not-a-view-toggle guidance, which is now trivially true.
- **`:260-262` Power View panel focus** → **Panel focus**. The `_Avoid_` note about `PowerFocus::Left` can go: the rename fixed it, so keep only the positive statement that queue side and library side are the domain concepts.
- **`:269` Audio visualizer** — **this one matters most.** It currently specifies two surfaces: embedded at the bottom of the Power library list, and a transient fullscreen surface reached "outside Power View". The second is now unreachable — there is no outside. Rewrite the entry down to the single embedded surface and drop the `_Avoid_` clause about "persisting the non-Power-View fullscreen visualizer". This is real scope reduction handed to #98, not bookkeeping.
- **`:81` Power View queue card** → **Queue card**. **`:225` display order**, **`:252`** and others use "power-list view" → "library list".

## Appendix C — note for `docs/adr/0009-v-key-controls-audio-visualizer.md`

Append:

```md
**Amended by ADR 0013 (2026-07-24):** this ADR's stated premise — "Power View remains a persisted view setting, not the default view" — no longer holds; Power View is now the only view. The conclusion is unaffected and in fact simplified: `v` remains reserved for the audio visualizer, now unconditionally rather than context-dependently, and the "transient fullscreen visualizer outside Power View" surface described here is no longer reachable. Only the embedded surface remains.
```

## Appendix D — comment for issue #98

```
#361 (Power View is now the only view) has narrowed this issue's scope.

CONTEXT.md previously specified two visualizer surfaces: an embedded one at the
bottom of the Power View library list, and a transient fullscreen surface reached
by pressing `v` "outside Power View". There is no longer an outside — the second
surface is unreachable and has been removed from the spec.

This issue is now one surface, not two: embedded at the bottom of the library
list, toggled with `v`, preference persists. The `v` key is unbound as of #361
and reserved for this work.
```
