# TuiRealm Migration — linear execution ledger

This file is the single authoritative execution ledger for the
`migrate-tui-to-tuirealm` change. It is a **docs-only reorganization** of the
prior task list into a linear ledger. It preserves every existing task ID
exactly, adds stable IDs only to previously unnamed/un-parsed units (approved:
`5.3c.1–.7` aliases, `5.3d.P1/P2`, `5.3d.M0`, `5.3d.M1/M1a–M1g`, and the parsed
`5.3d.11-U0…U6` leaves), and never renumbers an existing ID. The original
89-checkbox manifest semantics are preserved (all 77 originally-checked rows
remain checked; all 12 originally-unchecked rows remain unchecked); new checked
leaf aliases represent accepted completed `U0–U5` and previously-completed
unnamed units, and are reported as **additions distinct from** the original
89-checkbox count, not as a claim that the rewritten total "remains 89".

Sections:

1. Current campaign state
2. Execution contract
3. Active and scheduled execution
4. Remaining surface teardown
5. Global framework/campaign gates
6. Completed execution ledger
7. Historical decisions/completion evidence

---

## 1. Current campaign state

- **Accepted baseline HEAD:** `96a11eee62ad40371de083786bcc252f1ae05cd3` (the
  `5.3d.11-U5` landing commit). Doc changes land directly atop it in one atomic
  docs-only commit.
- **Next task (first open executable implementation checkbox):** `5.3d.18e`.
- **Accepted campaign order:** `U6 → 5.3d.18a → 5.3d.19a → 5.3d.20a`. This is an
  accepted *campaign predecessor* order, not an invented technical dependency;
  no technical dependency between TV, Music, and Inline surfaces is asserted beyond
  what the rows below record.
- **Open aggregate gates:** `5.3d.11` remains open as a pending aggregate (its `U0–U6` leaves have landed); `5.3d` stays open
  through all remaining surface teardown and `5.3d.21–24`; `5.5` and `5.6` stay
  open.
- **Checkbox accounting (this rewrite):** original 89 (77 checked / 12 unchecked)
  preserved exactly; additions *beyond* that count are the parsed open leaves
  `5.3d.18a–f`, `5.3d.19a–e`, `5.3d.20a–f`, the parsed `5.3d.11-U0…U6` leaves, and
  stable IDs on previously-completed unnamed units. The rewritten total therefore
  is not "89".

---

## 2. Execution contract

**Read this section before starting any task.** Three consecutive solution
sessions stalled without writing code because the previous version of this
preamble demanded a per-task bar (`delete, don't mirror`) that neither
`design.md` nor the capabilities spec actually requires. It has been corrected.

A surface conversion in groups 2–4 is **not** an ownership transfer. It is a
render/local-state extraction behind a shell-owned mirror. `App` keeps its fields and
its legacy input handlers until group 5. See design D14 for the bridge contract and
the reasoning.

### Standard bundle (groups 2–4)

Each surface-conversion task below bundles the following unless noted:

1. Create the component under `src/app/components/`.
2. The component owns its **rendering** and its **own local interaction
   state** (cursor/scroll/query/mode — whatever it needs to paint and to answer its
   own keys), reproducing the surface's current cursors, pills, panes, hero
   behaviour, focus targets, and keys exactly as the source defines them today
   (design §Governing Principle — there is no target design to invent).
3. The shell mirrors `App` into the component every tick via a `sync_<surface>()`
   method in `shell_overlays.rs` / `shell_<surface>.rs`, following the existing
   `get_component_mut` + downcast pattern (design D14). Async results arrive
   through an `apply_drain`-style push, as `SearchSidebarComponent` already does.
4. **Do NOT delete `App` state or `App` input handlers.** Leaving the legacy field
   and its `handle_key_*` in place, still forwarding, is the *correct* outcome for
   these tasks — not a shortcut. Deletion is group 5's job and is scheduled there
   per cluster. A task that tries to delete `App.<field>` will pull in every
   unrelated authority that reads it and will not land.
5. Emit a typed `Msg` for work crossing the component's authority boundary
   (Service calls, Player effects, nav_stack mutation, persistence). The component
   never owns an `mpsc`, a Service client, or a `PlayerProxy`.
6. Tests: local update/output tests, an `App`-free `TestBackend` render test, and
   one shell-routing test.
7. Flip the surface's `docs/architecture/interactive-surface-ledger.md` row to
   **`component`** (not `migrated` — see 1.10) with its verification record.
8. Verify with the named narrow `rtk cargo nextest` selector plus a clean
   `rtk ast-grep scan`.

Every checkpoint commit must be behaviour-preserving. None except group 5 is a
completion; a mixed framework is never a mergeable endpoint (spec: "Complete
conversion with no mixed-framework endpoint").

### Deferred by construction

These are **not** open questions blocking any group 2–4 task. They are all
answered in one place, at 5.2, when the precedence table moves as a unit:

- `key_policy.rs`'s per-key gates that cannot be a static `SubClause`
  (`playback`, `lib_search`, `album_track_mode`).
- How a per-instance `SubClause` guard is built at mount time for surfaces with
  one component per tab (`InlineSearch(BrowserKey)`, `Browser(BrowserKey)`).

Under the mirror-first bar these surfaces keep forwarding to legacy input, so no
subscription guard is needed for them until 5.2. Do not attempt to design one
earlier.

### Remaining teardown verification policy (decided 2026-08-25)

For the framework-removal and discovery-led units only. The compiler is the
primary gate; delete a field and every stale reader becomes a build error. The
per-unit gate is therefore `rtk cargo check -p mbv`, `rtk cargo clippy
--workspace --all-targets`, `rtk cargo nextest run -p mbv` (existing coverage
only), `rtk ast-grep scan`, and `rtk cargo fmt --all -- --check`, with the
maintainer's manual pass as acceptance. `rtk make check-code-file-lines` is
deferred to the final gate so bounded units do not churn unrelated over-limit
files. Do **not** write behaviour-preservation tests. Differential tests — two
paths agree, no expected values encoded — are permitted only while both paths
coexist, and are deleted together with the second path. Mouse units must never
assert against a hand-set `layout.main.*` rect; render into a `TestBackend` at a
known size and hit-test the geometry render produced. Regression tests for
defects introduced *by* the migration are the one exception and are kept.

### D17 — discovery-led teardown, durable scout handoffs

Before a writer is assigned to a remaining surface, a read-only scout records a
durable symbol-level handoff under `openspec/handoffs/` covering: (1) every input
to the mirror and every production writer of those inputs; (2) component-local
interaction state vs shell-owned content/cache/effect state; (3) raw input
forwarding and exact existing effect entry points; (4) legacy underpaint,
cover/image work, and layout values produced only by that renderer; (5) unrelated
readers preventing immediate `App` field deletion; (6) the smallest
compile-complete implementation units and their dependency order. Discovery and
implementation are separate assignments. A normal writer receives one closed
behaviour/ownership family touching roughly three to six production files; if
implementation exposes a missing authority or exceeds that bound, it stops and
returns the coupling instead of absorbing design discovery. Larger mechanical
fan-out requires a named preparation unit first.

A surface normally advances through these teardown stages, omitting a stage only
when the scout proves it does not exist: (1) separate mount reconciliation from
per-frame content projection; (2) replace projection with targeted pushes at
validated writer choke points; (3) replace raw input forwarding one coherent
behaviour family at a time with typed intents, keeping shell-owned effects at
existing boundaries — **the authority/writer transition (typed intent) lands
before the raw removal in the same surface**; (4) remove interaction-state pins
and obsolete `App` readers only after all remaining consumers are re-homed; (5)
detach component geometry/content from legacy underpaint, then delete that
surface's legacy renderer; (6) remove the now-empty mirror/mount adapter and
legacy handler endpoint.

Direct pushes of validated shell-owned content/cache/effect presentation are not
forbidden mirrors. The forbidden completion state is per-frame or two-way
synchronisation of component-local interaction state. Mount reconciliation may
remain temporarily after projection is removed, but must be renamed or deleted at
the surface completion stage so `sync_*` no longer hides multiple authorities.

Surface teardown precedes global framework deletion. Only after every remaining
raw-key endpoint and interaction mirror has gone may the campaign re-inventory
and delete `CONTEXT_STACK`, `LegacyInput`, and terminal reconstruction adapters.
Repository-wide line-cap verification runs at the final 5.6 gate; bounded units
run only named, focused/full existing-test, lint, architecture, and format checks.

---

## 3. Active and scheduled execution

Executable open rows, in accepted campaign order. Each uses literal checkbox
syntax and states its scope, dependency/campaign predecessor as applicable, and
verification.

### 5.3d.11 — audiobookshelf podcast interaction re-home (open aggregate)

- [ ] **5.3d.11** **Teardown — ABS podcast re-home.** Re-home remaining podcast
  App-level interaction readers to the mounted component; delete obsolete App
  episode/filter handlers and the empty mount/sync/push adapters, keeping the
  shared `AudiobookshelfBrowseState` type. Scout corrections (2026-08-26):
  type members `selected_id`/`episode_filter`/`episode_selection`/`scroll` are
  SHARED by `App.audiobookshelf_browse` and the component's state (B1); the
  deletable artifact is the App-level handlers, not the type members.
  `push_audiobookshelf_podcast_content` is NOT empty — it is the sole `set_content`
  + cover-fetch bridge (5.3d.9), so relocate it before deleting (B2). Position
  persistence crosses the App/Model boundary (B3). Child leaves:
  - [x] **U0** — component accessors (`selected_id()`/`episode_selection()`/
    `episode_filter()` getters, `set_episode_filter()`/`set_episode_selection()`
    mutators, `Model::abs_podcast_component_mut(index)`). 2 files
    (`components/audiobookshelf_podcast.rs`, `shell_audiobookshelf_podcast.rs`).
    Commits: `420ddcc9`.
  - [x] **U1** — relocate the `push_audiobookshelf_podcast_content` projection +
    cover-fetch (B2) into the post-mount path of `sync_audiobookshelf_podcast`;
    `shell.rs` drops the 9 `push_audiobookshelf_podcast_content()` calls
    (272/284/338/423/544/738/761/798/825). 2 files. Commits: `73850fda` +
    `3507183f`.
  - [x] **U2** — delete episode handlers + dispatch in the same commit:
    `audiobookshelf_browse_actions.rs` (delete
    `enter/leave_audiobookshelf_episode_selection`,
    `move_audiobookshelf_episode_cursor`, `cycle_audiobookshelf_filter`);
    `input_browse_dispatch.rs` (delete `handle_key_audiobookshelf_library` +
    dispatch arm); `shell.rs` (remove the ~810–825 component-routing callers). 3
    files. Commits: `bb54ad92` + `3c6d7ce8` + `0227d748`.
  - [x] **U3** — re-home modal filter (`audiobookshelf_podcast_modal_actions.rs`
    open/select uses the U0 accessors). ≤2 files. Depends on U0. Commits:
    `bbec2657` + `b3cf5d04`.
  - [x] **U4** — re-home position persistence (B3, parent-scope first):
    `library_position_state.rs` (save:237/238/290/291, activate:290/291/296/298/300)
    + a Model-side seam (`run_loop_drains.rs`/`cw_library_tab_actions.rs`/
    `select_audiobookshelf_show`). App has no `application` handle; likely exceeds
    3 files — split by parent before delegation. Commit: `877e28a6`.
  - [x] **U5** — playback target from the component (`selected_audiobookshelf_queue_item`
    + `handle_audiobookshelf_podcast_episode_intent`), resolved via U0 accessor +
    component method. 2 files; depends on U0/U2/U4 ordering. Commit:
    `96a11eee` (baseline).
  - [x] **U6** — Book-style split, not deletion: retain
    `sync_audiobookshelf_podcast` as a mount-only lifecycle owner (tab/kind
    guard + mount/unmount/active + `abs_podcast_id` write); extract the
    per-sync projection to a new `push_audiobookshelf_podcast_content` called
    at the writer sites (fresh mount, ABS drain, lib_rx drain,
    audiobookshelf_socket_rx drain, key/effect seams, PodcastShowMove /
    PodcastEpisodeIntent arms, modal filter select path); relocate the
    cover-fetch bridge (B2) into the push fn keeping the image-disabled gate;
    delete the dead `AudiobookshelfBrowseState::enter_episode_selection`
    method and the `abs_podcast_component_id` helper (sole caller is the
    retained sync; inline the `BrowserKey` construction at the call site,
    matching Book's shape). 3 files (`shell.rs`,
    `shell_audiobookshelf_podcast.rs`, `types_audiobookshelf_browse.rs`);
    green only after U0–U5. Commits: 5ca1b099.

Depends: 5.3a, 5.3b, 5.3c, 4.1, 4.10 (via the `5.3d` aggregate). Verification:
the `5.3d` policy gates; test sets `shell_audiobookshelf_podcast.rs` (re)
tests repainted as part of the production rows.

### Campaign schedule — TV / Music / Inline

- [x] **5.3d.18a — TV workspace typed keyboard.** Convert the series-list cursor
  keys (Up/k, Down/j, Left/h, Right/l, PageUp/Down, Home/End, Enter, Esc/Backspace)
  to typed requests carrying the resolved pane/episode/season cursor; keep episode
  play/enqueue raw for 18f. 3 files; Emby template commits `8929248`/`24e645b9`/`6fa217fb`.
  Campaign predecessor: `U6`.
- [x] **5.3d.19a — Music mount/idempotent mirror.** Idempotent mount + content
  mirror (album cursor via `move/jump/page_grouped_album_cursor`, focused track via
  `focused_music_track`); no behavioural change. ≤3 files. Campaign predecessor:
  `5.3d.18a`.
- [x] **5.3d.20a — Inline library-Search drop mount-id field.** `shell.rs`
  `inline_search_id` field + `shell_inline_search.rs`
  `inline_search_component_id` + `shell_library.rs:41` mount-id precedence branch.
  ≤3 files. Campaign predecessor: `5.3d.19a`.

---

## 4. Remaining surface teardown

The remaining open surface-scoped rows. Each `5.3d.18/.19/.20` aggregate stays
open until its child leaves land; each row lists its scopes in the exact original
wording. Dependency vs campaign predecessor is distinguished: the "BD" (hard
dependency) tags are technical; the remainder are accepted campaign predecessors.

### 5.3d.18 — TV workspace teardown (open)

- [ ] **5.3d.18** Refine the TV workspace contract into an exact typed
  keyboard and writer-seam contract; bounded rows below, no production edit in this
  aggregate.
  - **5.3d.18a — typed keyboard (3 files)** has been promoted to an open active
    leaf (see §3, campaign schedule). Convert the series-list cursor keys to typed
    requests; keep episode play/enqueue raw for 18f.
  - [x] **5.3d.18b — drop mirror-pin (≤3 files):** remove the
    `components/tv_workspace.rs` `last_mirrored_*` pins and its per-frame App
    cursor/selection writes. Retain `shell_tv_workspace.rs`'s `sync_tv_workspace`
    and temporary `set_content` refresh for non-cursor content (season/episode/
    pane/context) until 18c; make the component cursor authoritative and remove
    the TV `TvMoveRows`/`TvJumpCursor` App-side dual writes (B1).
  - [x] **5.3d.18c — writer pushes (≤3 files):** add `push_tv_workspace_content`
    at the nav-track/panel-focus/letter/resize writers (`shell_tv_workspace.rs`,
    `shell.rs`, `lib_cursor_actions.rs`, `input_browse_dispatch.rs`); keep the
    `series_detail_cache` reader at `shell_tv_workspace.rs:51` (B2).
  - [x] **5.3d.18d — geometry/underpaint (≤3 files):** `render/components/tv_wide.rs`
    publishes the `tv_wide_*` rects the component hit-tests; delete the legacy
    `render_list` wide-TV branch only after the component owns geometry (B4).
  - [x] **5.3d.18e — teardown (≤3 files):** remove the empty `sync_tv_workspace`
    adapter + `CONTEXT_STACK` TV arms + obsolete mount/sync names.
    Verified no-op/re-scope: sync_tv_workspace remains the live mount-only lifecycle owner; no CONTEXT_STACK TV arms or obsolete mount/sync names exist at this HEAD; full TV lifecycle teardown is deferred to aggregate 5.3d.21–5.3d.24.
  - [x] **5.3d.18f — episode play-only re-scope:** focused episode-pane Enter
    emits `TvEpisodeActivate`, resolved from
    `series_detail.episodes[season_id][episode_cursor]` and played through the
    existing playback path. Continuation: the episode enqueue trigger/request
    is intentionally deferred by product decision; no enqueue key or request
    was added.
  - Safe dependency order (honest): TV authority/writer transition (18c) lands
    before the raw removal (18e); no technical dependency on Music/Inline.

### 5.3d.19 — Music workspace teardown (open)

- [ ] **5.3d.19** Complete the Music contract with exact raw-key, projection,
  geometry, and underpaint rows; bounded rows (Music).
  - **5.3d.19a — mount/idempotent mirror** is promoted to an open active leaf (see
    §3): `components/music_workspace.rs` + `shell_music_workspace.rs`; idempotent
    mount + content mirror.
  - [x] **5.3d.19b — geometry pre-pass (≤3 files, BLOCKER for 19c):**
    `render/components/music_wide.rs` + `shell_music_workspace.rs`; compute
    `wide_music_area`/`wide_music_right_area`/`left`/`hero`/
    `wide_music_art_area` before the component `view` (today only the legacy
    `render_list` wide-music branch sets them — chicken-and-egg, R1).
  - [x] **5.3d.19c — delete legacy underpaint (≤3 files):** `render/components/list.rs`
    wide-music branch after 19b sets geometry. **Order:** 19d (fetch relocation)
    lands before 19c, because the fetch trigger currently lives in that branch.
  - [x] **5.3d.19d — relocate album-track fetch (≤3 files):** `images.rs`
    `fetch_album_tracks` has a legacy wide-Music trigger in `list.rs` that is being
    moved to the component/writer path (R2); narrow grouped-album-plan and
    selection-modal callers remain unchanged.
  - [x] **5.3d.19e — framework teardown (≤3 files):** remove the empty
    `sync_music_workspace` adapter + delete the differential legacy test.
    Re-scope: the adapter was verified live/non-empty and retained as mount-only
    lifecycle ownership; the differential test was removed, with full lifecycle
    teardown deferred to the aggregate teardown.
  - Risks (preserved): R1 geometry chicken-and-egg (blocker); R2 fetch
    relocation; R3 Page requires Library panel focus; R4 one-frame mouse warm-up;
    R5 h/l single-step vs arrow row-strides.

### 5.3d.20 — inline-library-Search residual scaffolding (open)

- [ ] **5.3d.20** Scout the remaining inline-library-Search mirror and raw endpoint,
  then bounded rows (surface already migrated; residual shell scaffolding).
  - **5.3d.20a — drop mount-id field** is promoted to an open active leaf (see §3):
    `shell.rs` `inline_search_id` + `shell_inline_search.rs`
    `inline_search_component_id` + `shell_library.rs:41` branch.
  - [x] **5.3d.20b — re-verify claimed redundant re-pushes (NO-OP at `ee28c78c`):**
    `shell_inline_search.rs` projections at lines 153, 206, and 241 are distinct
    required open/activation/LibEvent transitions; no source change made.
  - [x] **5.3d.20c — drop `apply_inline_search_items` (≤2 files):**
    `shell_inline_search.rs` + `parent_id` guard.
  - [x] **5.3d.20d — drop recursive pool branch (≤2 files):** `shell_inline_search.rs`
    recursive `Albums` pool branch.
  - [x] **5.3d.20e — re-host `/` trigger (≤2 files):** `components/browser.rs:90-92`
    `ShellRequest::OpenInlineSearch`.
  - [x] **5.3d.20f — mouse `left_area` quirk (≤2 files):** `components/inline_search.rs`
    Verified no-op; residual pre-first-view click before the first view() may see zero left_area.
    Default-layout mismatch.
- Risks (preserved): `scroll` written inside `view()` (render side-effect) —
  preserve on teardown; render seam `list.rs` + `list_rows.rs` `with_search`.

### 5.3d.13 — ABS Book Phase-B report gate (open, report-only)

- [x] **5.3d.13** Scout the ABS book typed-input, interaction-reader, legacy-render,
  image, and layout teardown at symbol level; add the resulting bounded rows here
  before any Phase-B book writer starts. **Open report-only gate** before any ABS
  Book Phase-B writer; no Phase-B production tasks are invented below it. (The
  Phase-A push helper at 5.3d.12 is checked.)
  - Report outcome: `audiobookshelf_book_area`, component geometry, image path, and
    mount-only sync remain; no teardown rows are created for those surfaces.
  - [x] **5.3d.13-R1 — typed ABS Book input:** Convert raw
    `AudiobookshelfBookKey` handling to typed component requests, mirroring podcast
    U6.
  - [x] **5.3d.13-R2 — ABS Book legacy reader teardown:** After R1's zero-ref gate,
    delete `handle_key_audiobookshelf_book_library` and its shell bridge.
  - [x] **5.3d.13-R3 — ABS Book App-state cleanup:** After R2, remove the remaining
    obsolete App reader/state pieces.

---

## 5. Global framework/campaign gates

These remain **open** until the whole change completes. The mixed framework is
never a mergeable endpoint; the final end-of-change requirements below are the
definition of done.

### 5.3d aggregate + final rows (open)

- [ ] **5.3d** Teardown — framework removal. Requires 5.3a, 5.3b, 5.3c, 4.1, 4.10.
  Remove `LegacyInput`, `CONTEXT_STACK` interaction dispatch, the global mouse
  router/hit map and duplicated mouse-coordinate paths, every interaction-state
  `sync_<surface>()` mirror, and all remaining temporary interaction adapters.
  Render-only layout state may remain under D16. **Stays open through every
  remaining row below and `5.3d.21–24`.**

### Final gates

- [x] **5.3d.21** After every surface row above lands, re-inventory remaining
  `CONTEXT_STACK`, `Msg::Legacy`, `LegacyTerminalEvent`, `LegacyInput` terminals,
  and `sync_*` interaction endpoints; classify retained shell-owned content
  projections and add exact deletion rows before editing.
  - **Inventory outcome:** The live `CONTEXT_STACK` has 11 global precedence
    entries: `global_overlay_open`, `queue_column_width`, `panel_mode_cycle_x`,
    `confirm_skip_intro`, `confirm_next_up`, `clear_queue_prompt_c`, `visualizer`,
    `playback`, `ctrl_l_force_clear`, `f5_refresh`, and `view_dispatch`.
    `playback` and shell/runtime precedence remain retained. `UiRoot`,
    `LegacyInput`, `Msg::Legacy`, `LegacyTerminalEvent`, and terminal
    reconstruction remain live until per-surface raw forwarding is gone.
    `sync_*` content/mount projections and render-derived hitmaps remain
    retained. The mouse framework is D16 report-only/accepted-broken.
  - **Execution order:** Remove per-surface raw forwarding adapters first;
    then complete the 5.3d.22 precedence cleanup; then the 5.3d.23 bridge
    teardown; finally perform 5.3d.24 verification.
  - [x] **5.3d.22-A — precedence stack family:** After per-surface raw
    forwarding removal, delete the bounded precedence-stack family, allowlisted
    to `src/app/input_resolver.rs`, `src/app/input.rs`,
    `src/app/input_queue_keys.rs`, and `src/app/input_lib_keys.rs`.
    - **Verified no-op at `fae36907`:** every candidate remains reachable from
      the live `CONTEXT_STACK`/legacy bridge or has a direct shell/test caller;
      no bounded entry or handler met the zero-reference gate without changing
      keyboard precedence or widening into 5.3d.22-B/23. No production symbols
      were deleted.
  - [x] **5.3d.22-B — confirm/visualizer precedence family:** After 5.3d.22-A,
    delete the bounded confirm/visualizer precedence family, allowlisted to
    `src/app/input_confirm_keys.rs`, `src/app/input.rs`, and
    `src/app/input_resolver.rs`. Run serially after 5.3d.22-A because the two
    families share input files.
    - **Verified no-op at `572ffe5b`:** none of the bounded entries or handlers
      met the zero-reference gate. `confirm_skip_intro` and `confirm_next_up`
      remain direct shell playback-prompt callers as well as live stack entries;
      `clear_queue_prompt_c` remains a live stack entry and is covered by the
      end-to-end input tests; and `visualizer` remains a live stack entry with
      direct visualizer tests. Removing any candidate would change keyboard
      precedence or playback/shell behavior. No source symbols were deleted.
  - [ ] **5.3d.23-A — global bridge teardown:** After all per-surface forwarding
    adapters and the 5.3d.22 precedence families are complete, remove the
    global bridge pieces allowlisted to `src/app/root.rs`,
    `src/app/legacy_input.rs`, `src/app/msg.rs`, and `src/app/shell.rs`.
  - [x] **5.3d.23-B — terminal conversion-adapter consumer fanout:** Report-only
    inventory of the terminal conversion-adapter fanout (>6 production files);
    do not delete consumers in this row. Live production conversion/Legacy
    consumers span >6 files: global `LegacyInput`/root/shell plus Home, Browser,
    TV workspace, Music workspace, ABS podcast/book, `daemon_lost`,
    `remote_reanchor`, confirm, queue, playback prompt/playback, playlists,
    feeds/feeds_manage, settings, inline search, context menu, selection modal,
    sessions, multiselect, search sidebar, library routes, save playlist, and
    help. No consumer is safely dead; the global bridge must remain until all
    families reach zero-reference gates. ABS Book R1/R2/R3 leaves are complete,
    but its component still has legitimate Legacy fallback/mouse endpoints for
    this phase.
    - [x] **5.3d.23-B1 — typed request/key surfaces:** Serial child; allowlisted
      to `src/app/components/queue.rs`, `save_playlist.rs`, `settings.rs`,
      `feeds_manage.rs`, and `feeds.rs`. Convert local/typed authority before
      the zero-reference gate for both crossterm adapters and `Msg::Legacy`;
      run focused queue/settings/feeds tests.
    - [x] **5.3d.23-B2 — transient overlays:** Serial child; allowlisted to
      `confirm.rs`, `daemon_lost.rs`, `remote_reanchor.rs`,
      `playback_prompt.rs`, and `context_menu.rs`. Pass the zero adapter/
      `Msg::Legacy` gate, preserve shell modal effects, and add/run focused
      tests (playback_prompt coverage may need adding).
    - [x] **5.3d.23-B3 — media/workspace surfaces:** Serial child; allowlisted
      to `music_workspace.rs`, `tv_workspace.rs`, `audiobookshelf_podcast.rs`,
      `audiobookshelf_book.rs`, `playback.rs`, and `playlists.rs`. Pass the zero
      adapter/`Msg::Legacy` gate, preserve media effects, and run focused
      surface tests, noting playback/playlists test gaps.
    - [x] **5.3d.23-B4 — navigation/search/content surfaces:** Serial child;
      allowlisted to `home.rs`, `browser.rs`, `inline_search.rs`,
      `search_sidebar.rs`, `library_routes.rs`, and `help.rs`. Pass the zero
      adapter/`Msg::Legacy` gate, preserve hit geometry and shell effects, and
      run focused home/browser/inline-search tests plus module checks for the
      others.
    - [ ] **5.3d.23-B5 — residual NoOp-only components:** Serial child;
      allowlisted to `selection_modal.rs`, `sessions.rs`, and `multiselect.rs`.
      Pass the zero `Msg::Legacy` gate; this is the last component family before
      5.3d.23-A. Run the corresponding component tests.
    - B1-B5 are serial because the global bridge remains until every
      zero-reference gate passes.
  - [ ] **5.3d.24-A — mouse/framework residual:** Report-only verification of
    `mouse_gestures`, layout, and render-derived hitmaps at the D16 boundary;
    accepted-broken mouse framework residuals are not deletion work here.
- [ ] **5.3d.22** Delete now-unreferenced per-surface `CONTEXT_STACK` handlers in
  bounded families (the static keyboard-precedence proofs from 5.4).
- [ ] **5.3d.23** Delete `LegacyInput`, `Msg::Legacy`, `LegacyTerminalEvent`,
  terminal reconstruction adapters, and obsolete mount/sync names after the
  inventory is empty (no raw legacy terminal endpoint remains).
- [ ] **5.3d.24** Verify no component-local interaction state is mirrored through
  `App`, no legacy renderer paints beneath a migrated component surface, and no
  global mouse router/hit map remains.
- [ ] **5.5** Flip all `docs/architecture/interactive-surface-ledger.md` rows to
  `migrated` with verification records; verify no `legacy` **and** no `component`
  row remains (see 1.10).
- [ ] **5.6 Final gate:** `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
  `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan` pass; plus the
  **final-only** `rtk make check-code-file-lines`; confirm no parallel legacy
  interaction framework remains and the shell Model holds only shell/runtime
  authority plus the TuiRealm `Application`.

Final commands preserved in full. The line-cap gate (`rtk make check-code-file-lines`)
runs only here.

---

## 6. Completed execution ledger

All originally-checked rows, preserved. This is the "past" of the campaign — do
not re-execute. Original 77 checked rows are listed below; the new checked leaf
aliases (`U0`–`U5`, and stable IDs on previously-completed unnamed units) are
additions shown as checked without implying a "remain 89" total.

### Foundation (group 1)

- [x] **1.1** Add `tuirealm = "4.1"` (default features already include
  `crossterm` and `derive`); verify `rtk cargo check -p mbv` succeeds and
  `Cargo.lock` resolves tuirealm 4.1 on the existing ratatui 0.30/crossterm 0.29.
- [x] **1.2** Declare `rust-version = "1.88"` in `[workspace.package]` **and** add
  `rust-version.workspace = true` to each member (`mbv`, `mbv-core`, `mbvd`);
  verify `rtk cargo check --workspace` passes and CI uses a ≥1.88 toolchain.
- [x] **1.3** Add `src/app/components/` with the `ComponentId`, `Msg`, and
  `UserEvent` enums from D3–D5 (surface variants may start empty); verify
  `rtk check -p mbv`.
- [x] **1.4** Introduce the shell `Model` holding `App` and the TuiRealm
  `Application<ComponentId, Msg, UserEvent>`; verify it builds.
- [x] **1.5** Convert `App::run` to drive `application.tick(PollStrategy::Once(..))`
  and mark the frame dirty when `tick` reports a processed event (reuse the
  existing `had_events` → `wants_terminal_render` path). † Note — "the binary still
  launches", "temporary message-only `LegacyInput`", verify the first frame still
  precedes Remote Service startup (ADR 0018); test unchanged.
- [x] **1.6** Map each run-loop receiver (startup, player, library, Search, session,
  cast, shared-data, feed, image, websocket, ABS socket) to a shell-owned adapter
  (default) or a TuiRealm `Port`, each injecting a `UserEvent` token; validated by
  the existing generation/revision/session/image-key guards then written via
  `get_component_mut`+downcast; prefer shell-owned adapters for runtime-replaced
  listeners (player, websocket, ABS socket) since `restart_listener` replaces the
  whole listener. Verify async-completion behaviour and stale-completion discards
  unchanged.
- [x] **1.7** Add the `key_policy` ordered precedence table mirroring the current
  `CONTEXT_STACK` order and wire global/parent bindings as TuiRealm subscriptions
  with mutually-exclusive `SubClause` guards derived; verify against ADR 0002
  input-precedence tests.
- [x] **1.8** Route mouse via `EventClause::Any` subscriptions on visible top-level
  regions, filtering `Event::Mouse(column,row)` against its own painted geometry
  guided `Not(IsMounted(overlay))` under blocking overlays (no shell hit-router —
  `Application` has no per-component event delivery); CP1 `LegacyInput` route. Verify
  unchanged mouse behaviour.
- [x] **1.9** Add enforcement scaffolding: `rules/interactive-component-boundary/*.yml`
  (reject `impl App`, `App` as type, Service-client/`PlayerProxy` deps, `mpsc`
  ownership) each with one accepted + one rejected fixture, register the dir in
  `sgconfig.yml`, add `.github/workflows/architecture-boundaries.yml` job
  `interactive-component-boundary` pinning ast-grep 0.44.1; verify scan passes and
  fixtures demonstrate accept/reject.
- [x] **1.10 (doc-only, do first)** — reconcile the ledger vocabulary: add the
  intermediate state `component` to `docs/architecture/interactive-surface-ledger.md`
  legend; demote the seven already-flipped rows (Help, Confirm, Daemon-lost,
  Remote-reanchor, Context menu, Global Search sidebar, Sessions) from `migrated`
  to `component`; add the term to `CONTEXT.md` per the AGENTS.md new-domain-term
  rule behind 5.5's gate.

### Group 2 — low-risk leaf surfaces

- [x] **2.1** Convert Help sidebar (local scroll, destination-derived content);
  verify `rtk cargo nextest run -p mbv help` + `rtk ast-grep scan`.
- [x] **2.2** Convert Confirm modal (shared yes/no).
- [x] **2.3** Convert Daemon-lost modal (process-lifecycle effects stay shell-owned).
- [x] **2.4** Convert Remote-reanchor popup (reconciliation stays shell-owned).
- [x] **2.5** Convert Context menu (exclusive top-priority overlay with anchor
  geometry).

### Group 3 — medium-risk surfaces

- [x] **3.1** Extract the Search render seam (`render_panel_shell*`,
  `render_sidebar_scrollbar`, `panel_row_text_width`, `render_panel_row`),
  output-preserving, no `impl App`.
- [x] **3.2** Convert the global Search sidebar (component-owned 300 ms debounce
  via `UserEvent::Clock`, preserve the `global-search-sidebar` contract, do **NOT**
  fix its known bugs).
- [x] **3.3** Convert inline library Search (`LibSearch`, child of one Emby
  browser, distinct from global Search). Component owns query string + results
  cursor/scroll + the two candidate-pool shapes; shell keeps
  `spawn_search_items_load`, `App.album_indexes`, `activate_recursive_album`;
  `LibraryTab.search` stays on `App` until 5.3a.
- [x] **3.4** Convert Home (cross-Service rows + hero presentation). partly landed:
  `src/app/shell_home.rs` mounts it and mirrors `App.home` per tick; remaining work
  component-owned local cursor/scroll, test bundle, ledger row. `key_policy`
  precedence deferred to 5.2.
- [x] **3.5** Convert the Emby generic/Movies/home-video browser (gated on 3.11);
  music-group/series/album branches stay behind the 3.11 seam until 4.2/4.3/4.4.
- [x] **3.6** Convert Feeds (grouping/selector/list/inline hero); shell keeps the
  refresh `mpsc`, `feed_tab_actions.rs`, Home-Feeds section build, `feeds_manage`
  reset, `sync_feeds()`. Teardown of all above is 5.3b.
- [x] **3.7** Convert the Sessions sidebar (merged Emby/Cast targets, fixed-stride
  geometry); verify `rtk cargo nextest run -p mbv sessions` + scan.
- [x] **3.8** Convert Selection modal (filters, source-specific behaviour).
- [x] **3.9** Convert Playback prompts (skip-intro/next-up; Player effects stay
  shell-owned).
- [x] **3.10** Convert Settings nested popups (Multiselect, Library-routes,
  Feed-management) as `Popup` children.
- [x] **3.11** **Shared render seam — gates 3.3/3.5/4.2/4.3/4.4.** Behaviour-
  preserving extraction; parameterizes item-source/cursor/scroll/column/hero-sizing
  decisions currently made by `impl App` reading `lib.search` and `lib.nav_stack`
  directly into a typed context (`ListRenderCtx`), across `list.rs` (`render_list`,
  `render_wide_library_rows`), `tv_wide.rs` (`render_wide_tv`), `movies_wide.rs`
  (`render_wide_movies`, `selected_movie`), `music_wide.rs` (`render_music_group`,
  `render_left_tracks`), `detail.rs` (`selected_movie_item`, `selected_series_item`).
  ~1,500 lines — largest single diff; sequential per-file commits encouraged.

### Group 4 — high-risk surfaces

- [x] **4.1** Convert Queue (cursor/scroll/scope to the component; canonical queue
  stays in the Player owner, referenced by opaque `QueueSlotId`).
- [x] **4.2** Convert the TV workspace (two focusable panes, season/episode child
  targets). Gated on 3.11, independent of 3.5.
- [x] **4.3** Convert the grouped Music workspace (album/track focus coupling).
- [x] **4.4** Convert inline album-track interaction (child state machine of Music).
- [x] **4.5** Convert the Audiobookshelf podcast browser (show/episode workspace).
- [x] **4.6** Convert the Audiobookshelf book browser (browser/chapter workspace).
- [x] **4.7** Convert Playlists sidebar (variable-row `hit_test`). The duplicated
  mouse-path geometry in `input_mouse_panels.rs` is 5.3c.
- [x] **4.8** Convert the Save-playlist dialog (child of the Playlists workflow).
- [x] **4.9** Convert the Settings sidebar and setup forms (Service effects stay
  typed `Msg::Service`).
- [x] **4.10** Convert Playback chrome and global controls (Playback authority
  stays outside; reduced-panel projection).

### Group 5 — teardown core (completed; the `5.3d` aggregate stays open)

- [x] **5.1** Convert the Library parent (active destination, Panel focus/mode,
  child routing); verify `library_parent` + scan.
- [x] **5.2** Convert Root UI + overlay-stack routing using TuiRealm's native LIFO
  focus stack (yes). Resolve here the precedence questions deferred from groups
  2–4 (the per-key gates) with the 5.2 `SubClause`/mount-time one-component-per-tab
  answer; verify `root_ui` + scan.
- [x] **5.3-pre** — LibraryTab constructor (`LibraryTab::new(library)`), rewrite
  each literal to `..LibraryTab::new(item)`, delete no field. Verify unchanged test
  count + clippy full.
- [x] **5.3a** — Teardown Library/browse cluster. Requires 3.3/3.5/3.11/4.2/4.3/4.4/
  5.1. Delete `LibraryTab`'s component-owned fields (`search`,
  `series_selection`/`series_season_cursor`) + the handlers: `input_browse_dispatch.rs`,
  `input_lib_keys.rs`, the eight `search.is_some()` branches in
  `lib_cursor_actions.rs`, `select`/`go_back` arms in `actions_navigation.rs`,
  `lib_event_actions.rs` `lib.search` handlers, `library_search_actions.rs`; extract
  `select(lib_idx)` → `select_item(lib_idx, item)`. Rewrite the tests. **Landed**
  in three passes (search `008be6c5`..`9ac69d81`, series `9e4bd7c`/`153c9b9`/
  `758d0a84`; `5.3-pre` `5d9e77ec`).
- [x] **5.3b** — Teardown Feeds cluster. Requires 3.6/3.4/3.10/5.1. Delete
  `FeedTabState` interaction fields; move readers to the shell boundary
  (`feed_tab_actions.rs`, Home-Feeds section build, `feeds_manage` reset, Feeds
  key/mouse branches). `App.feed_tab` itself survives (shell-owned fetch state;
  not component-owned; 5.6 does not require removal).
- [x] **5.3c** — Teardown overlay/modal cluster. Requires 2.1–2.5/3.2/3.7/3.8/3.9/
  4.7/4.8/4.9/5.2. Delete the `App` open-flags, overlay state, and forwarded
  handlers + duplicated `input_mouse_panels.rs` geometry. Dispatched as named
  units, sized by files forced open (~45 ceiling / ~25 target). **Completed units
  (5.3c.1–.7):**
  - [x] **5.3c.1** Overlay prep — `shell_overlays.rs` split by family,
    `App::ask_confirm`.
  - [x] **5.3c.2** Modals — `confirm_modal`, `daemon_lost_modal`,
    `remote_reanchor_popup`, `save_playlist_dialog` replaced by
    `pending_overlay: Option<OverlayRequest>` + `blocking_overlay_active`. 48 files.
  - [x] **5.3c.3** Sidebar state prep — the four open-flags collapsed to
    `open_sidebar: Option<SidebarId>`.
  - [x] **5.3c.4** Sidebars — delete `open_sidebar` + sidebar `handle_key_*`.
  - [x] **5.3c.5** Selection modal.
  - [x] **5.3c.6** Context menu.
  - [x] **5.3c.7** Settings popups.

### 5.3d completed units

- [x] **5.3d.P1** Album cursor prep — settle the narrow-mode question (or prove
  unreachable), move `render/screens/album_cursor.rs` three entry points into
  `MusicWorkspaceComponent`. Behaviour-neutral, deletes no field.
- [x] **5.3d.P2** Album track focus — delete `LibraryTab.album_track_focus`, re-home
  its four `= None` resets; `MusicWorkspaceComponent::track_cursor` is the sole
  owner (narrow stays explicitly unfocused). `#app`-free. Landing reported `6b2977d4`.
- [x] **5.3d.M0** Mouse gesture prep — extract the remaining three
  `match self.tab` dispatch points into one named method per surface (mirroring
  `handle_mouse_scroll_browse`).
- [x] **5.3d.M1** Mouse geometry and router — alpha scope disposition; for the
  deferrals the D16 series; for the landed units the mouse-path ownership:
  - [x] **5.3d.M1a** `browser` `hit_test` — real row/hit geometry.
  - [x] **5.3d.M1b** `home` `hit_test`.
  - [x] **5.3d.M1c** `queue` `hit_test`.
  - [x] **5.3d.M1d** `tv_workspace` `hit_test` — two focusable panes.
  - [x] **5.3d.M1e** `music_workspace` `hit_test` — explicitly deferred beyond
    alpha by D16.
  - [x] **5.3d.M1f** Blocking modals/prompt (`confirm`, `daemon_lost`,
    `remote_reanchor`, `playback_prompt`) — explicitly deferred beyond alpha by D16.
  - [x] **5.3d.M1g** Framework deletion — delete the three legacy
    `input_mouse*.rs` entry points and their global coordinate routing.
- [x] **5.3d.1** Recount and classify remaining `sync_*` methods; record retained
  shell-owned projections vs seven interaction mirrors in `scoping-5.3d-mirrors.md`.
- [x] **5.3d.2** Re-home Home content and section-preference ownership, replace the
  per-frame content mirror with writer pushes. `b3fbf5b0`–`b21653dc`.
- [x] **5.3d.3** Replace ABS podcast per-frame content projection with event-scoped
  writer pushes. `b414a1ec`, `2c6bcce5`.
- [x] **5.3d.4** Resolve the podcast show-movement parity discrepancy; the
  component's restored `selected_id` (local one-item/page vs legacy multi-column
  row). Exact typed contract: `ShellRequest::AudiobookshelfPodcastShowMove`.
- [x] **5.3d.5** Convert only podcast show-list movement to
  `AudiobookshelfPodcastShowMove`. `4eeee915`.
- [x] **5.3d.6** Convert podcast episode movement/filter/exit to
  `AudiobookshelfPodcastEpisodeTransition`. `0d8a4ef0`.
- [x] **5.3d.7** Convert podcast enter/play/enqueue/modal to typed intents. `d6f67656` +
  `e7abcb13`.
- [x] **5.3d.8** Complete the podcast downstream-reader/cover-fetch handoff:
  enumerate persistence/queue/legacy-render/image-fetch readers. No production edit.
- [x] **5.3d.9** Move podcast cover fetch to the smallest shell/Model bridge;
  preserve the image-disabled gate. `bbe2fda4`.
- [x] **5.3d.10** Delete podcast legacy underpaint after its cover/layout handlers
  are detached; keep narrow/default render characterization. (Cover-fetch bridge
  landed in 5.3d.9; legacy underpaint removal, plus the row's `10d`/`10e` slices,
  commits `4d2f6de1`, `af2bf9c8`, `ad403485`+`2e9090e5`.)
- [x] **5.3d.12** Implement the two-file Audiobookshelf book Phase-A push helper
  (mount-only reconciliation + writer-seam pushes; App writers/component/renderer
  unchanged). `4f5df745` + key-seam/kind-guard correction `354fc5c0`.
- [x] **5.3d.14** Resolve Emby `wide_movies` ownership (**D18**): adopt the
  temporary per-draw adapter now (move `set_wide_movies` into the draw closure,
  mirroring `dim_backdrop_active`); component-local derivation deferred to
  5.3d.17/R1.
- [x] **5.3d.15** Split Emby mount reconciliation from content projection, replace
  content/focus pushes at proven writers. `8929248` (mount) + `push`/M2; Emby template.
- [x] **5.3d.16** Claim remaining Emby raw-key fallthrough, remove cursor/scroll
  mirror-pin, make the component authoritative.
- [x] **5.3d.17** Remove Emby generic/Movies/home-video legacy underpaint after the
  wide dependency is detached; remove the empty sync/mount adapter.
- [x] **5.4** Confirm every alpha-supported mouse path reads component-owned
  geometry; verify keyboard precedence, deterministic focus restoration, and D16's
  structural checks (the absence of the three `input_mouse*.rs` entry points and of
  any global mouse hit map). **Completed static-table proof path; D15 declined
  (explicit).** Six proofs + deterministic focus restoration recorded in section 7
  below (`key_policy.rs` keeps `#![allow(dead_code)]`).
  - [x] **Orphan cleanup** (folded into this unit) — `ccc75e30` deleted the dead
    `GroupedAlbumGroup.start`/`.end` and `GroupedAlbumCatalog.groups` fields and
    their builders.

---

## 7. Historical decisions/completion evidence

Keep the completed evidence readable but out of the active path.

### 5.4 proof narrative (D15 declined explicitly)

`KEY_POLICY` and `KeyPolicyGate::sub_clause()` are referenced nowhere outside
`key_policy.rs`'s own ordering test; the file carries `#![allow(dead_code)]`. 5.3a
analyzed and table runs through legacy CONTEXT_STACK dispatch, so the clauses do
not execute until legacy input is removed at 5.3c/5.3d. 5.4 **declined** D15's
`Component::perform(Cmd)` adoption because that is not incrementally valid while
`LegacyInput` and `CONTEXT_STACK` still route keys. Proofs established:

1. blocking-overlay swallow — `blocking_contexts_swallow_before_*` test: every
   `blocking` entry precedes every `Sub` entry; `global_overlay_open` gate is
   `NotHasAttrValue(Playback, ATTR_BLOCKING_OVERLAY_ACTIVE)`.
2. parent/global precedence + owners — `parent_and_global_bindings...` test.
3. table consistent with CONTEXT_STACK — `key_policy_order_matches_context_stack`
   test.
4. simultaneous Queue+Library mouse — D16 structural: `input_mouse*.rs` absent and
   no global mouse map/router remains; components self-filter.
5. global overlay blocks underlying mutation — D16 structural; the
   `global_overlay_open` gate + `ATTR_BLOCKING_OVERLAY_ACTIVE`.
6. geometry cannot drift — D16 structural: each component hit-tests its own
   view()-painted `Rect`/rows (`home.rs`, `tv_workspace`, `browser`, `queue`,
   `playlists`, `settings`, …); `AppLayout` survives only load-bearing.

Deterministic focus restoration: `root_ui_uses_native_lifo_focus_restoration`
(covers Help, stacked Confirm, restoration)).

### 5.3c / 5.3d sizing and deferral record

- Modals were 48 files / 958 changed lines (ceiling ~45 files / target ~25).
- `music_workspace` and blocking-modal/prompt mouse **deferred beyond alpha by
  D16**; framework deletion supersedes the remaining mouse work.
- `App.feed_tab` survives holding shell-owned fetch state (not component-owned).

### D16 restatement (accept for this ledger)

Global interaction/hit-routing authority and interaction-only layout data are
removed; render-only, load-bearing `AppLayout` state may remain. `AppLayout`
survives only for load-bearing rendering, not as an interaction hit-map.

### Scout-handoff notice

The five scout handoff files referenced by earlier rows
(`scout-abs-podcast-b1-first-slice.md`, `scout-music-workspace-preliminary.md`,
`scout-abs-book-phase-a.md`, `scout-emby-browser.md`, `scout-tv-workspace.md`) do
not exist under `openspec/handoffs/`. Their inline contracts/evidence have
been retained in the rows above (5.3d.4, 5.3d.18–20, 5.3d.12, 5.3d.14–17) so no
task loses its contract; no replacement handoff files are created.

### D14–D18 remain

- **D14** two-stage mirror→delete.
- **D15** `Cmd in, Msg out`, static-proof path (declined at 5.4).
- **D16** accepted-broken; framework deleted rather than ported.
- **D17** discovery-led staged teardown; ~3–6 production-file writer units.
- **D18** Emby `wide_movies` per-draw adapter; component-owned V0 at underpaint.

No Phase-B production tasks are invented below 5.3d.13.