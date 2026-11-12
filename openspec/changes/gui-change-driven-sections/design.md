# Design

## Context

See `proposal.md` (Why) and the `gui-client` delta spec for behavior. This section gives only the current state that shapes the approach. The state is `main` after `add-gui-settings-screen` (PR 998) merges.

- **The step.** `CoreThread::step` (`crates/mbv-gui/src/core_thread.rs`) waits on its channel with the timeout `CLIENT_IDLE_CADENCE` (50 ms), runs `ClientCore::tick`, and calls `emit_frame` when a `report_layout` arrived, when the tick saw input, or when `App::frame_cadence_due` says the TUI render interval (150 ms while playback is active, 1 s otherwise) passed. The tick host sets `input_received` for every `CoreInput::Wake`, so each Owner relay wake also emits.
- **The frame.** `emit_frame` syncs the Queue Surface state, runs `App::fetch_playback_metadata`, reads `queue_playback_projection` one time, resolves the trickplay tile, the card artwork, and the Library section, and builds `GuiFrame` with every field. The sink sends it as `GuiWindowFrame::Slice` on `mbv://frame`. `frames.ts` keeps only the last frame.
- **The carry-once memos.** `theme_pending` (the theme), `ArtworkFrames` (`artwork.rs`), `TrickplayFrames` (`trickplay.rs`), and `LibraryFrames` (`library_frames.rs`) each remember what the window holds and re-arm on `report_layout`. `LibraryFrames` builds the whole Library content on every frame and compares it with the last build to find its revision. Its builders also run the frame-path ensures (`ensure_gui_*_selection`, `ensure_gui_shows_workspace`, the art `fetch_card_image` calls, and `warm_gui_hero_neighbours`).
- **Artwork.** `resolve_gui_art` clones the cached source bytes. `GuiLibraryArt::from` and `artwork::encode` base64-encode them into a `data:` URL on the core thread. On `main` no pixel re-encode happens on this path: the #997 item 6 "encode" is the base64 step and the byte clone. The CSP is `img-src 'self' data:`.
- **The playhead.** `GuiQueuePlayback` carries `playback` (position, runtime, paused), `elapsed_clock`, `remaining_clock`, and `finish_clock`, all from Rust. The rail and the compact strip derive their progress from `positionTicks`. mbv has no playback-rate fact: no projection carries a speed.
- **Settings (PR 998).** `GuiSettingsFrame::build(app, playback)` runs on every frame. It calls `App::gui_settings()` (a `GuiSettingsState` clone) several times, `config_snapshot()` (a full `Config` clone under the lock), and `gui_settings_row_choices` (a `to_vec` per row). The Sessions section reads `playback.target_choices`.
- **Tests.** `crates/mbv-gui/tests/core_thread.rs` drives `CoreThread::step` over a mocked channel and sink with `mbv` `test`-feature stubs (`stub_app_with_queue`, `stub_app_with_playing_queue`, the `MockHttp` Emby client). Criterion 0.8.2 is a workspace dev-dependency with benches in `mbv-render` and `mbv-components` (`docs/architecture/performance.md`). The UI uses vitest 5.1.0, whose `bench` mode is built in.
- **File sizes.** `core_thread.rs` has 796 lines, `ui/src/lib/library/state.ts` 790, `bridge/frame/library.rs` 700. New code goes into new modules.

## Goals / Non-Goals

**Goals:**

- The core thread does work in proportion to change, not to time.
- The bridge carries only changed data, and never image bytes.
- The front end follows web conventions: one store for each kind of data, a local clock for time-based values, images by URL.
- Every optimization is measured against a baseline taken before it.

**Non-Goals:**

- No change to `ClientCore::tick`, `ClientTickHost`, `CLIENT_IDLE_CADENCE`, or the TUI loop (`wants_terminal_render`, `render_interval`).
- No change to the Owner, the ctrl protocol, or the Owner's progress broadcast.
- No change to the Library panel's interaction rules (ADR 0036) or to any key route.
- No playback-rate support. The playhead has no rate field because mbv has none.
- No CLI argument, no environment switch, no new script.

## Decisions

### D1. One `mbv://sections` event per emitting tick, with per-section revisions

The bridge has one event, `mbv://sections`, with the payload `GuiSectionBatch`. The batch has one optional field for each section. Each present field is a `GuiSection<T> { revision: u64, value: T }`. An absent field means "unchanged". A section whose content can be absent (the overlay, the Library list) has `T = Option<…>`, so `value: null` clears it.

The sections, with the tier that decides when they are built (D2):

| Section | Content | Tier |
| --- | --- | --- |
| `attach` | the pre-attach state, `null` after the attach | attach campaign |
| `theme` | `GuiTheme` | gated |
| `focus` | `GuiFocus` | compared |
| `queue` | `GuiQueue` (viewport rows, cursor, marks, hover, drag) | compared |
| `summary` | `GuiQueueSummary` | compared |
| `panel` | `GuiPanelToggles` | compared |
| `overlay` | `Option<GuiOverlayFrame>` | compared |
| `playback` | `GuiQueuePlayback` without the playhead fields and the hover; its artwork is a `GuiArtRef` (D7) | compared |
| `playhead` | `GuiPlayhead` (D5) | anchor rule |
| `hover` | `Option<GuiScrubberHover>` with the tile's `GuiArtRef` | compared |
| `footer` | `GuiQueueFooter` | compared |
| `status` | `GuiStatusBar` | compared |
| `libraryList` | `Option<GuiLibraryList>`: tabs, current tab, the destination's list part (D8) | gated |
| `libraryDetail` | `Option<GuiLibraryDetail>`: the selection, the Hero, the Workspace, the item menu, the playing marks (D8) | gated |
| `rowMenu` | `Option<GuiLibraryRowMenuFrame>` | gated |
| `search` | `Option<GuiSearchFrame>` | gated |
| `settings` | `Option<GuiSettingsFrame>` | gated |

**Why one event and not one event per section.** The user's direction named per-section events. One event that carries several sections keeps the per-section stores and revisions, and it adds atomicity: a track change moves the active slot in `queue`, `playback`, and `playhead` in one update, so the window never paints two active slots (spec scenario "Two sections change together"). Tauri delivers events in order, so a batch needs no sequence number. Separate events would need a commit marker to give the same guarantee.

**Why a revision.** The revision lets the front end skip a store write when a re-sent section (after `sync`) has the revision it holds. It also gives the logs and tests one number to name a section state. Revisions start at 1 and grow by one for each built value that differs from the last built value. A wrap is not reachable in a Client's life.

### D2. Two build tiers and a section ledger

A new module `crates/mbv-gui/src/sections.rs` holds `SectionLedger`. For each section it keeps the last built value, its revision, and whether the window holds that revision. After each tick that did not ask for a restart, the step calls `SectionLedger::collect`, which returns a batch or `None`. The step sends a batch only when it is `Some`. The step keeps `fetch_playback_metadata` and the Queue Surface sync in its per-tick prologue before `collect`: today `emit_frame` is the only GUI caller of `App::fetch_playback_metadata`, and no test fails when its trigger is lost.

- **Compared sections** are built on every tick and compared with the last built value (`PartialEq`). They are small: the queue section holds only the viewport rows, and the other compared sections hold a few fields each. Building them on each 50 ms tick replaces today's full build at up to 6.7 Hz, which also builds the whole Library content, serializes everything, and updates the webview. The idle-step bench (D12) records the cost before and after the ledger lands.
- **Gated sections** are built only while they carry a dirty mark (D3). A gated build still compares with the last built value, so a mark that changes nothing sends nothing.
- **The playhead** follows its own rule (D5).

`frame_requested`, `last_frame`, `theme_pending`, `ArtworkFrames`, `TrickplayFrames`, `LibraryFrames`, and `App::frame_cadence_due` are deleted. `App::frame_cadence_due` has no other caller. `render_interval` stays for the TUI.

The step keeps the `CLIENT_IDLE_CADENCE` wait. The shared tick has time-based phases (the delayed settings save, session polls, the websocket keepalive, toast expiry), and the Non-Goals keep `ClientCore::tick` as it is. The wait is a tick timer, not a repaint timer. A timeout tick emits only what changed. This amends point 1 of the direction: "no repaint timer" holds, "no timer" does not.

**Alternative rejected: diff the serialized JSON.** It still builds and serializes every section on every tick, which is the cost this change removes.

### D3. Dirty marks come from inputs, drain hooks, deadlines, and playhead marks

Direction point 4 asked for "each App projection rebuilds only when its source revision changes". Revisions inside `App` would need a bump at every mutation site of every Service store. The TUI shares those sites, and a missed bump leaves the GUI stale with no error. So the source revision is kept by the core: `SectionLedger` keeps a dirty mark for each gated section, and four sources set marks.

1. **Inputs.** A key, a Library command, a Settings command, and a press, click, double-click, or right-click gesture that resolved to a region mark every gated section. These come at human rate. A pointer move, a wheel step, drag motion, a cancel, a layout report, a `sync`, and an art request mark none: they change only compared sections, and an effect that reaches the Owner comes back through a drain hook.
2. **Drain hooks.** The GUI's `ClientTickHost` methods set marks when they report work. The table lives in one function, `marks_for_hook`, with an exhaustive `match` on a `DrainHook` enum, so a new hook fails the build until it names its sections:
   - `drain_startup_workers`, `drain_suspended_home_events`: every gated section.
   - `drain_player_events` (handled), `drain_lib_events`, `reproject_audiobookshelf_content`, `reproject_feed_content`, `drain_feed_add_results`, `drain_ws_events`, `drain_audiobookshelf_socket_events`, `drain_idle_feed`: `libraryList`, `libraryDetail`, `rowMenu`, `settings`.
   - `drain_search_results`: `search`.
   - `drain_card_image_completions`: `libraryList`, `libraryDetail`.
   - `drain_resize_responses`, `drain_transport_events`: none (documented no-ops).
3. **Deadlines.** Two Library facts change with time alone: the navigation-idle gate of `warm_gui_hero_neighbours`, and the retry time of an interrupted art fetch (invariant 27). `App` gets two read-only queries. `App::gui_navigation_idle() -> bool` wraps the crate-private `navigation_idle` (`last_nav_at` plus `NAV_IDLE_DELAY`), so every path that calls `stamp_navigation` feeds it and `mbv-gui` copies no constant. The core compares it on each tick like rule 4: a change from not idle to idle is the passed deadline. The core needs only that change, not the deadline's `Instant`. `App::gui_art_retry_due(now) -> bool` is true when an interrupted key's retry time passed since the last call. A passed deadline marks `libraryList` and `libraryDetail`.
4. **Playhead marks.** `App` gets one read-only query, `App::gui_playhead_marks() -> GuiPlayheadMarks`, with the playing Music track id and the current Audiobooks chapter, computed by the derivations the projections already call. The step compares it on each tick. A change marks `libraryDetail` and `libraryList`. A change of the `playback` section's target choices marks `settings`, because the Sessions section reads them.

`App`-side drains with no host hook (`drain_session_events`, `drain_cast_events`, `drain_playback_metadata_events`, `drain_trickplay_sheet_events`, `drain_notif_actions`, `drain_pinned_requests`, `settle_pending_queue_transfer`) feed compared sections, or reach a gated section through rule 4.

**The audit.** A missed mark is a stale section with no error. `CoreThread::set_section_audit_for_test(true)` makes the ledger build every clean gated section on every tick and panic when the build differs from the last built value. `test_core_with` turns it on, so every core-thread test audits. Production never turns it on. The audit cannot catch a missed navigation-idle mark: the warm-up is a side effect (a fetch start), not a section value, and the audit's own builds run `warm_gui_hero_neighbours`. Thus the test of that mark turns the audit off. A new invariant (`docs/invariants/32-section-marks-cover-their-sources.md`) records the rule, the audit, and the places it can still fail.

**Frame-path ensures.** The ensures and fetch triggers stay inside the gated builders. They run when the section is dirty, which is when their inputs can have changed. The deadline marks cover the two time-based cases.

### D4. `sync` replaces the `report_layout` re-arm

A new bridge command `sync` pushes `CoreInput::Sync`. The ledger marks every section "not held by the window". The next batch carries every present section at its current revision (a full snapshot). `ui/src/lib/bridge/sections.ts` calls `invoke('sync')` after its `listen` promise resolves, so no batch can fall between the subscribe and the snapshot.

`report_layout` applies the layout (capacity, hit map, container size, the hover re-resolve) and nothing else. A layout change that moves the queue viewport changes the `queue` section, and the normal comparison sends it. The rule "every `report_layout` emits, changed or not" is removed.

The attach campaign (`attach.rs`) keeps its own loop. It sends `GuiSectionBatch { attach, theme }` for each state change and answers `CoreInput::Sync` with both. A dead-end state serves `Sync` and `Close`, where it served `Layout` and `Close`. The attached core's first `collect` sends `attach: null` with every section.

The theme is a gated section with one mark at core start. It is built one time and re-sent on each `sync`. A later change that edits the palette marks it.

### D5. The playhead anchor

`GuiPlayhead { active: bool, activeSlot: Option<u64>, positionTicks: i64, runtimeTicks: i64, paused: bool }`. The ledger keeps the last sent anchor and the `Instant` of its send.

On each tick the step reads the position from the one `queue_playback_projection` read of that tick and sends a new anchor when one of these is true:

- `active`, `activeSlot`, `paused`, or `runtimeTicks` changed;
- the reported position is a new sample (it differs from the last observed value) and differs from the prediction `anchor.position + elapsed since send` (no advance while paused) by more than `PLAYHEAD_TOLERANCE` = 1 s.

A seek moves the position far from the prediction, so it re-anchors at once. Only a new sample is compared. An output that reports its position from time to time (a Cast receiver, an Emby Session) does not re-anchor between reports, and the window does not jump back (spec scenario "The output reports no new position"). A stall while buffering re-anchors when the next sample arrives.

**Amendment: no Rust timestamp on the wire.** The direction asked for "position, play state, and a timestamp". The core and the webview share no monotonic clock: an `Instant` cannot cross the process boundary, and a wall-clock timestamp can step under NTP. The anchor's position is the position at the send. The front end stamps the anchor with `performance.now()` when it receives it. The IPC delay (well under one frame) is the only error, and the next anchor corrects it.

**Amendment: no rate.** mbv has no playback-rate fact, so the anchor has no rate and a rate change is not a trigger. If mpv's speed changes through mpv's own keys, the drift rule re-anchors each time the error passes 1 s.

**What the front end derives.** `ui/src/lib/bridge/playhead.ts` keeps the anchor and a `now` store that `requestAnimationFrame` drives while the anchor is active and not paused, and that stops otherwise. Derived stores give the position (clamped to `[0, runtime]`), the elapsed clock, the remaining clock, and the played fraction. The scrubber fill, the scrubber clocks, the rail ring, the rail's remaining time, and the compact strip's progress line read them. `ui/src/lib/bridge/time.ts` gets `formatDurationShort`, a port of `mbv_ui_model::ui_util::fmt_duration_short` (`H:MM:SS` from one hour, else `M:SS`).

**What stays in Rust.** Facts with a minute or percent grain: the transport's finish clock, the queue header's finish clock, the rows' "Ends at" value, the queue rows' progress percent, the Music playing track, and the Audiobooks current chapter. Their sections are compared (or marked through D3 rule 4), so they send when the value changes: about once a minute for a clock, once per percent for a row. A finish clock is the time now plus the remaining time, so it also moves each minute while playback is paused. Thus a paused window gets the `playback`, `summary`, and `queue` sections once each minute. This change keeps that behavior; it does not freeze the finish clocks while paused. This keeps local-time formatting and the queue's runtime chain in one place (Rust), and it costs a few small updates per minute. `GuiQueuePlayback` loses `playback`, `elapsed_clock`, and `remaining_clock`. The hover keeps its Rust `time_label`, because the pointer drives it, not the clock.

**Alternative rejected: Rust sends the clocks at 1 Hz.** It keeps a timer-driven update for every playing second, which is the cost this change removes.

### D6. Front-end section stores

`ui/src/lib/bridge/sections.ts` replaces `frames.ts`. It exports one Svelte `writable` store for each section (the project's existing store pattern; `frames.ts` and `library/state.ts` already use `svelte/store`), `applyBatch(batch)`, and `connectSections()` (listen, then `sync`). `applyBatch` writes a store only when the section's revision differs from the one it holds. A batch that carries `theme` calls `applyTheme`. `App.svelte` passes each component the section stores it reads, in place of slices of one frame. `carryLibraryContent` and the artwork and trickplay latches in `Panel.svelte` are deleted. A store write in Svelte notifies only that store's subscribers, so a `queue` batch re-renders only the queue.

### D7. Artwork through the `mbv-art` URI scheme

`main.rs` registers `mbv-art` with `tauri::Builder::register_asynchronous_uri_scheme_protocol` (Tauri 2.12.5, the current version; no upgrade). On Linux, WebKitGTK serves the scheme as `mbv-art://localhost/<path>`. The paths:

- `card/<percent-encoded cache key>`: an image-cache entry's source bytes (the Library art, the queue card art);
- `trickplay/<percent-encoded sheet key>`: a cached trickplay sheet;
- `placeholder`: `QUEUE_CARD_PLACEHOLDER_BYTES` (`image/webp`).

The handler never touches `App`. It pushes `CoreInput::Art(ArtRequest { path, reply })` into the core channel. `reply` is a `Box<dyn FnOnce(ArtResponse) + Send>` that `main` wires to the `UriSchemeResponder`, so `core_thread` stays free of Tauri types and the tests mock the reply. The tick host answers each pending art request in `tick_input` before it applies other input. It reads `App` through two new read-only queries only: `App::gui_art_source(&str) -> Option<SourceImage>` (the cached source of an image-cache key) and `App::trickplay_sheet_bytes(&TrickplaySheetKey) -> Option<Arc<[u8]>>` (a cached sheet; the handler parses the wire key in the path back into the key). It never calls `queue_card_artwork`, `gui_library_artwork`, or `trickplay_preview_tile`, because these projections start fetches. `gui_art_source` clones the source bytes one time for each request; the browser cache makes a second request for the same URL rare. An art request sets no dirty mark and does not set `input_received`. The attach campaign answers an art request with `Missing`, so no responder is left open before the attach.

`ArtResponse` is `Found { mime, bytes }` or `Missing`. `Found` becomes a 200 response with `Content-Type` from the cached source's MIME type and `Cache-Control: max-age=31536000, immutable`. The cache entry under a key is final for the life of the process (invariant 27), and the webview lives in the same process, so a URL never names two different images. `Missing` becomes a 404. The front end's `onerror` paints the loading fill (spec scenario "A referenced image is no longer cached").

DTOs carry `GuiArtRef`: `{ kind: 'image', src }`, `{ kind: 'placeholder', src }`, or `{ kind: 'loading' }`. `src` is built in Rust by one function (`art_url`), so the scheme name and the encoding live in one place. `GuiLibraryArt`, `GuiCardArtwork`'s `data:` payloads, `GuiScrubberTile.sheet`, `artwork::encode`, and `data_url` are deleted. The `base64` dependency of `mbv-gui` is deleted with them when the crate has no other use of it. The CSP in `tauri.conf.json` becomes `img-src 'self' mbv-art:`. `data:` leaves the CSP because no `data:` image remains.

This closes #997 item 2 (no data URL to keep) and item 6 (no encode on the core thread; the core clones the bytes one time per request, and the browser cache holds the decoded image). The keyed LRU that #997 proposed is not needed: the image cache is the keyed store, and the browser's cache is the second level.

**Alternative rejected: a keyed `data:` URL section with an LRU** (the other option the user was offered). It keeps base64 (+33 %) and megabyte JSON events, and the core must mirror what the window evicted.

### D8. The Library list and detail are separate sections

Each destination's `GuiXFrame` splits into a list part and a detail part:

- `GuiLibraryList { tabs, currentTab, destination: GuiDestinationList }`. `GuiDestinationList` is a tagged enum (`music`, `home`, `shows`, `emby`, `audiobooks`, `podcasts`, `feeds`) with each destination's pills, rows or tree, `lib_idx`, `loading`, and `failed` facts.
- `GuiLibraryDetail` is a tagged enum with the same tags and each destination's `selected`, `hero`, Workspace, art reference, item menu, and (Music) `playing_track_id`.

Both are built in one gated pass, because one App projection call (for example `gui_music_projection`) gives both. Each is compared on its own, so a pick sends `libraryDetail` only (spec scenario "A pick changes only the detail"). The row menu moves out of the content into `rowMenu`, and Search stays its own section. The tagged enums replace the seven `Option` fields and the "no two fields present" rule, so the type states the exclusivity.

### D9. The Library scroll offset is its own store (#997 item 1)

`ui/src/lib/library/scroll.ts` holds each list's `scrollTop` and `viewportHeight` in their own store. `library/state.ts` keeps the cursor, the open nodes, the filter, and the filter state. Each list (`LibraryList`, `LibraryShowsList`, `LibraryAudiobooksList`, `LibraryFeedsList`, `LibraryHomeList`, `LibraryEmbyList`, `LibraryPodcastsList`) derives its rows from the content, the open nodes, the filter, and the filter state only, and its visible window from the rows and the scroll store. `scrollLibraryList` clamps with the derived row count and does not rebuild rows. This also takes `state.ts` (790 lines) away from the 800-line limit.

### D10. Smaller projection fixes

- **Search (#997 item 5).** `gui_search_projection` returns `None` at once for an empty or blank query that has no rows. `GuiSearchFrame` is built by moving the projection's rows, not by `carried` cloning them again.
- **Settings (PR 998 findings).** `config_snapshot` is replaced by `App::with_config(|config| …)`, which borrows the configuration under the lock. `App::gui_settings()` returns `Option<&GuiSettingsState>`. `gui_settings_row_choices` returns `Option<&[GuiSettingsChoiceProjection]>`, and the DTO copies a choice list one time, when the section is built. With D3 the section builds only when marked.
- **Shows, Home, tabs, row menu (#997 items 3, 4, and the minor item).** D3 gates them. No other change: once they build only on a mark, a per-build clone costs at human rate.

### D11. What does not change

`ClientCore::tick`, `ClientTickHost`, the TUI's `Model` run loop, `CLIENT_IDLE_CADENCE`, the Library commands, the key routes, the Queue Surface state rules, and the layout registry's coalescing. The `CoreInput` variants `Key`, `Pointer`, `Layout`, `Library`, `Settings`, `Wake`, and `Close` keep their meaning. `Sync` and `Art` are added.

### D12. Measure first, and the contract of each test

**Baseline.** A criterion bench `crates/mbv-gui/benches/sections.rs` (dev-dependency `criterion = { workspace = true }`, criterion 0.8.2, the current stable release, already the workspace version; `[[bench]] name = "sections" harness = false`). It drives `CoreThread::step` through the public test seams over `mbv` `test`-feature stubs. The fixtures:

1. an idle step: a 50-slot queue, nothing playing;
2. a playing step: a 50-slot queue, the active item playing;
3. a Music step: a Grouped Music library with 2,000 artists and 6,000 albums, one album picked;
4. a Shows step: the All mode with 300 series;
5. a Home step: 50 Continue rows and 100 recent rows;
6. a Search step: an open query with 200 results;
7. a Settings step: the Settings screen open on its F2 section.

Each fixture reports the step time. A bench helper reads the emitted payload's serialized size and reports it as criterion `Throughput::Bytes`, so the bytes per step are measured too. Fixtures that the `test` feature cannot build get a new stub in `src/app/app_stub.rs` behind the feature. A vitest bench `ui/src/lib/library/rows.bench.ts` (vitest 5.1.0 `bench`, no new dependency) measures the row derivation of `LibraryList` under 100 scroll steps over 5,000 rows.

The baseline runs before any change. The medians go in `openspec/changes/gui-change-driven-sections/measurements.md` (committed with the change), and the PR description quotes them. Each later task re-runs the bench that covers its fix and adds its medians to the same file. A fix with no gain beyond noise is reverted, except D1 to D5, D7, and D8: they are the contract change, and the measure for them is structural (an idle step sends nothing; a pick sends no list).

**Test contracts** (one owner each, `docs/invariants/14-test-ownership.md`). Rust tests live in `crates/mbv-gui/tests/` and front-end tests beside their module:

| Contract | Owner |
| --- | --- |
| An idle tick with nothing changed sends no batch | `tests/core_thread.rs` (replaces `a_cadence_due_timeout_tick_emits_the_frame`) |
| A batch carries only changed sections, with a new revision | `tests/core_thread/sections.rs` |
| `sync` re-sends every present section at its held revision | `tests/core_thread/sections.rs` |
| A layout report that moves nothing sends nothing | `tests/core_thread.rs` (replaces `layout_report_emits_a_frame_without_a_state_change`) |
| A clean gated section matches its fresh build (the audit) | every core-thread test, through `test_core_with` |
| The drain-hook mark table | `tests/core_thread/sections.rs`, one `#[case]` row for each hook whose sections differ |
| A passed navigation-idle gate starts the Hero neighbour fetch with no input | `tests/core_thread/sections.rs`, with the audit off |
| The anchor rule: change of slot, pause, runtime; seek; drift; no re-anchor without a new sample | `tests/core_thread/playhead.rs`, one `#[case]` row for each trigger |
| A pick sends `libraryDetail` and no `libraryList` | `tests/core_thread/library/frame.rs` |
| An art request answers the cached bytes, the placeholder, or missing | `tests/core_thread/art.rs` |
| The attach phase answers `sync` with its state and theme | `tests/attach.rs` |
| `applyBatch` writes only the named stores and skips a held revision | `ui/src/lib/bridge/sections.test.ts` |
| The playhead advances while playing and holds while paused | `ui/src/lib/bridge/playhead.test.ts` (time injected, no real wait) |
| `formatDurationShort` matches the Rust formatter's two forms | `ui/src/lib/bridge/time.test.ts` |
| A scroll does not rebuild the rows | `ui/src/lib/library/scroll.test.ts` |
| The empty Search query projects nothing | `src/app/tests` (the existing Search projection test file) |

Existing tests that assert the frame shape move to the section that now owns the fact. They are not copied. A test whose contract was "the frame carries X" becomes "the section carries X" in the same file.

### D13. Terms

These terms go into `CONTEXT.md` with the code (task 2.11):

- *Window section*: one named part of what the GUI window paints, sent by the Client core with its own revision and kept by the window in its own store. Avoid: frame section, slice.
- *Section batch*: the one update that carries the window sections that changed in one tick. Avoid: frame, snapshot (except for the full batch that answers `sync`).
- *Playhead anchor*: the position, runtime, paused state, and active slot that the Client core sends on a discontinuity, from which the window predicts the position. Avoid: progress tick, position update.

## Risks / Trade-offs

- [A missed dirty mark leaves a section stale until the next input] → The test-only audit (D3) fails every core-thread test that exercises the missed source. The mark table is one exhaustive `match`. Invariant 32 records the rule.
- [The compared tier builds on every 50 ms tick, more often than today's idle frame] → Its sections are small, and the build sends nothing when nothing changed. The idle-step bench records the cost against the baseline idle frame, which also built the whole Library content.
- [A held key repeats at about 30 Hz and marks every gated section on each repeat] → This is today's cost (a full frame per key), not a new one. A key can run any `[keys]` binding, so a narrower mark would need a per-action table, which this change does not add.
- [The window's predicted position differs from mpv's by up to the tolerance] → 1 s is below what the clocks can show without a visible jump, and a new sample past it re-anchors. The finish clocks stay in Rust and are not affected.
- [An art URL can name an entry the image cache evicted] → The core answers 404 and the window paints the loading fill. A later mark rebuilds the section, and the art reference becomes `loading` while the fetch runs again.
- [A custom URI scheme is a new input surface into the core thread] → The handler accepts only the three path forms and answers only from the cache. It runs no fetch, reads no file, and takes no other parameter. The CSP admits only `mbv-art:` beside `'self'`.
- [Every component that reads `$frames` changes] → `App.svelte` is the one place that unpacks the frame today (five non-test files read `frames`), so the change is mostly prop wiring.
- [Today's specs say "the next frame"] → In the `gui-client` and `gui-settings-screen` specs, "frame" reads as "the next update the window receives". The behavior is the same, so no delta rewrites those lines.

## Migration Plan

The bridge contract is internal to `mbv-gui`, and the Rust side and the front end build together (`make dev`). No stored state changes. The GUI launch state, the panel widths in local storage, and the Owner protocol do not change. Rollback is a revert of the change's commits.

## Open Questions

None.
