# Scout handoff — ABS podcast Phase B teardown (5.3d), scoped at HEAD 2c6bcce5

Scope mandate (read before using): **Audiobookshelf podcast ownership teardown only**.
Do NOT touch ABS books (`audiobookshelf_book_browse`, `AudiobookshelfBookComponent`,
`render/components/audiobookshelf_books*`, book arms in `input_browse_dispatch.rs`, book
branches in `run_loop_drains.rs`/`widgets.rs`) or any other surface. All evidence below is
podcast-scoped unless marked shared.

Phase A (HEAD `2c6cce5`) already cut the per-frame mirror: `sync_audiobookshelf_podcast`
is now mount-lifecycle-only and the per-frame `set_content` was replaced by the event-scoped
`push_audiobookshelf_podcast_content` projection at the writers of its projected inputs
(`shell_audiobookshelf_podcast.rs:33-70`, `77-95`). Phase B is the "3.5-style ownership move"
the mirror scoping doc names as still pending
(`scoping-5.3d-mirrors.md:158-159`: "needs the full 5.3a-style ownership move (component keys
+ typed Msg + App-field deletion + legacy render branch)").

---

## 1. App-owned podcast interaction fields + every production reader/writer

The single App container is
`App.audiobookshelf_browse: Vec<AudiobookshelfBrowseState>` (`app_struct.rs:60-62`),
entry per ABS library index. The podcast-shaped struct
(`types_audiobookshelf_browse.rs:50-67`) mixes interaction + content/cache:

| Field | Class | Production readers / writers (podcast scope) |
|---|---|---|
| `selected_id` | interaction (show cursor) | R: legacy renderer `render/components/audiobookshelf.rs` (via `selected_show`/`cursor`), `select_audiobookshelf_show` (`browse_actions.rs:164`), `activate_audiobookshelf_position` (`library_position_state.rs:281-286`), `save_audiobookshelf_position` (`library_position_state.rs:231-233`), `selected_audiobookshelf_queue_item` (`browse_actions.rs:271-276`), component `set_content` preservation (`audiobookshelf_podcast.rs:51-60`). W: `select()` (`types_audiobookshelf_browse.rs:87-99`), `append_page` (auto-select), `run_loop_drains.rs:61` init, `activate_audiobookshelf_position` restore. |
| `episode_selection` | interaction | component `move_episode`/`cycle_filter`/`Esc`; legacy `handle_key_audiobookshelf_library` (`input_browse_dispatch.rs:177-207`) and its called actions (`browse_actions.rs` enter/leave/move); **read by effect** `selected_audiobookshelf_queue_item` (`browse_actions.rs:275`); read by legacy renderer (`render_audiobookshelf_episode_rows`). |
| `episode_filter` | interaction | component `cycle_filter`; legacy `cycle_audiobookshelf_filter` (`browse_actions.rs:241+`); read in `audiobookshelf_podcast_modal_actions.rs:22,54` (modal filter); legacy `render_audiobookshelf_episode_rows`. |
| `scroll` | interaction | component (`set_content` preserves; `render_show_rows` writes); legacy `render_audiobookshelf_show_rows` (`audiobookshelf.rs:651+`). |

Content/cache/effect fields that **must remain** App-owned for Phase B and beyond:
`library`, `shows`, `total`, `next_page`, `loading_pages`, `error`, `episodes`,
`detail_cache`, `detail_loading`, `progress`. These are written by the service-completion
path (`audiobookshelf_service_actions.rs`, `run_loop_drains.rs:81`, `lib_event_actions.rs`
socket progress reconcile at 176/191/310/329/365/702/715) and read by `push_selection`
(`shell_audiobookshelf_podcast.rs:85`), the narrow modal (`audiobookshelf_podcast_modal_actions.rs`),
render, and playback-queue resolution. Do NOT move these into the component in this unit.

### Production read/write map of the whole Vec (podcast only)
- shell projection read: `shell_audiobookshelf_podcast.rs:85` (`push_audiobookshelf_podcast_content`).
- component receives snapshot via `set_content` (`shell_audiobookshelf_podcast.rs:94`).
- legacy renderer: `render/components/audiobookshelf.rs:114,184,227,259,314,496,647` (whole file is podcast-only).
- narrow selection-modal builder: `audiobookshelf_podcast_modal_actions.rs:14,54`.
- position save/restore: `library_position_state.rs:231,287` (this is effect, keep on App).
- effect methods + cursor moves: `audiobookshelf_browse_actions.rs:33-245` (see §3/§5 for split).
- async service completions / drains: `audiobookshelf_service_actions.rs:461,484`, `run_loop_drains.rs:57-106`, `lib_event_actions.rs:176-748`, `app_audiobookshelf_service_completion.rs:22`.
- key front-door: `input_browse_dispatch.rs:176-230` (legacy handler to delete).

## 2. Content/cache vs interaction — what stays App-owned

Content/cache/effect (`shows`, `episodes`, `detail_cache`, `detail_loading`, `loading_pages`,
`total`, `next_page`, `error`, `progress`, and the `library` identity) stay on
`App.audiobookshelf_browse` for Phase B. They are produced by async Service work and consumed
by render, playback-queue resolution, position persistence, and the selection modal. The
component already reads them only through the pushed snapshot.

The four interaction fields (`selected_id`, `episode_filter`, `episode_selection`, `scroll`)
are ALREADY component-local too (`AudiobookshelfPodcastComponent{state: AudiobookshelfBrowseState}`,
`audiobookshelf_podcast.rs:24-26`). The Phase B task is to make the component the **authority**
and stop the legacy handler from re-driving them — not to delete them outright, because
`selected_id`/`episode_selection` are still read by: position persistence
(`library_position_state.rs:231,281`), queue-item resolution (`browse_actions.rs:271-276`),
and the legacy renderer. Their removal is the follow-on render/layout unit (see §8). Keep them
as data fields on `App` for this unit; stop mutating them on the interaction path.

## 3. Key forwarding — component event → App → back

Current trace (all keyboard):
1. `LegacyInput` terminal bridge → `Application` → `AudiobookshelfPodcastComponent::on` (`audiobookshelf_podcast.rs:247-255`).
2. `Component.handle_key` (`audiobookshelf_podcast.rs:84-135`) mutates the component-local cursor/episode/filter/scroll, then **always** returns `Msg::Shell(ShellRequest::AudiobookshelfPodcastKey(tocrossterm_key))` (`:131`) — it forwards every key raw.
3. `shell.rs:723-731` arms `ShellRequest::AudiobookshelfPodcastKey` → `Model::handle_audiobookshelf_podcast_key` → `App::handle_key` → `input.rs:198 PanelFocus::Library → handle_key_browse_dispatch → input_browse_dispatch.rs:56-64 Podcast arm → handle_key_audiobookshelf_library` (`:176-230`).
4. That legacy handler re-implements the same cursor/episode/filter dispatch AND the effects (wide/narrow Enter→selection modal or episode-focus; Space Enter play; piece Ctrl+A enqueue; `[`/`]` filter; PageUp/Down/Home/End) mutating App state via `audiobookshelf_browse_actions.rs`.
5. The shell then calls `push_audiobookshelf_podcast_content()` (`shell.rs:730`) re-snapshotting the (now-mutated) App state into the component (`set_content` preserving `selected_id` etc. only if still present).

So today App is still the authority for effects AND for the cursor (it re-mutates the same fields the component already set). The component mutate is effectively overwritten by the push-back; the effect branch is what drives `save/save_audiobookshelf_position`, queue and modal.

Phase B goal: delete the legacy handler, and have the component emit **typed intents** instead of a raw key, with the shell calling the existing App effect methods on the selected show/episode.

## 3. Traces: cover fetch + selection-aware effects

Cover fetch today lives **only** in the legacy renderer, inside hero painting:
- `render/components/audiobookshell_podcast` is podcast-only is this file. `render_audiobookshelf_hero` (`audiobookshelf.rs:304`), guarded by `self.images_enabled()` (`:335`), computes `server_url` from `self.config(...)` and calls `self.fetch_audiobookshelf_cover(server, show.library_item_id)` at line 337. The fetch itself is idempotent (`images.rs:334-344`: guarded by `image_protocol_enabled`, `card_image_loading`, `card_image_states`).
- This runs on the base `app.render(f)` frame, which paints under the component; the component paints `image: None` in its own `render_podcast_hero` (`render/components/audiobookshelf_podcast.rs`), so in current state the cover image region is a redraw-over by the component (pre-existing; not part of this unit — flagged risk #6).

Slice: for the remove-renderer unit (B2, §8), the cover-fetch call must relocate to the smallest existing bridge — `push_audiobookshelf_podcast_content` already replay runs at the exact writers of `shows`/`episodes`. Because the image state is deduplicated, moving the fetch there (called on mount + when the selected show changes) preserves behavior with no new seam. For B1 this is NOT required — renderer stays.

## 4. Which legacy renderer reads to remove first

The legacy podcast renderer is 100% contained in `render/components/audiobookshelf.rs`
(`render_audiobookshelf_podcasts` :103, `..._bucket_pills` :220, `render_audiobookshelf_hero` :304,
`render_audiobookshelf_episode_rows` :492, `render_audiobookshelf_show_rows` :637). This file
has **zero book coupling** — it is the podcast renderer alone (book render is in
`render/components/audiobookshelf_books.rs`). Dispatch is `widgets.rs:551-566` →
`tab.audiobookshelf_index()` → if Book kind `render_audiobookshelf_books`, else
`render_audiobookshelf_podcasts`. No other production caller reads this file's functions.

So the legacy renderer can be deleted once:
(a) the component's render covers both wide and narrow (it does — `render_audiobookshelf_content`
handles both), and
(b) two pieces of state the legacy renderer currently ALSO produces are re-homed:
   - `layout.main.audiobookshelf_podcast_area` / `.audiobookshelf_podcast_right_area` /
     `.hero_area` / `.left_area` geometry (`render/components/audiobookshelf.rs:104-146`) is read
     by `render_audiobookshelf_podcast_component` (`shell_audiobookshelf_podcast.rs:82`) and by
     `layout.rs:105,152-164` (mouse geometry). This slice must recompute that area in the shell
     (a small legal-only move, ~3 lines: reuse `hero_left::shared_hero_presentation(pod_area)`).
   - the cover-fetch relocation (see §9).
Below. Do not delete the renderer in unit B1; leave it as a redundant base under-lay while
interaction is removed first.

## 5. Recommended smallest coherent next unit

**Unit B1 — ABS podcast interaction authority to component + shell; legacy podcast stub handler
removed.** This is the bounded, one-family slice of the 5.3d podcast teardown. It does *not*
delete the content/cache struct in `AudiobookshelfBrowseState` (selected fields stay on App for
position + queue + render reads) and does **not** delete the legacy renderer (stays, keeps the
cover-fetch). That containment is local, safe and reviews cleanly.

- In-scope: only the podcast keyboard path + podcast effect dispatch.
- Out-of-scope (leave for B2/B3): legacy renderer deletion, cover-fetch relocation, deletion of
  `selected_id`/`episode_filter`/`episode_selection`/`scroll` from `AudiobookshelfBrowseState`
  (all still read by App authorities), book surface, mouse path.
- Production files (this unit) — **6 files**:
  1. `components/audiobookshelf_podcast.rs` — teach component to handle every podcast key and
     emit typed intents (currently it forwards every key). e.g. add `ShellRequest` variants with
     the selected-target ids.
  2. `components/msg.rs` — add podcast intent variants (e.g. `PodcastShowSelected(showId)`,
     `PodcastEpisodePlay(showId, epId, index)`, `PodcastEpisodeEnqueue(...)`, `PodcastOpenModal`,
     `PodcastCycleFilter`, `PodcastEnterEpisode`).
  3. `shell_audiobookshelf_podcast.rs` — new `Model` effect handlers calling the existing App
     methods (select/play/enqueue/modal) + re-push.
  4. `shell.rs` — match the new variants in the `Msg` loop, delete the raw-key +
     `handle_audiobookshelf_podcast_key` branch.
  5. `input_browse_dispatch.rs` — delete `handle_key_audiobookshelf_library` (L176-230) and the
     `AudiobookshelfBrowseKind::Library => …` podcast arm in `handle_key_browse_dispatch`
     (L59-64). Keep the book arm.
  6. `audiobookshelf_browse_actions.rs` — delete the now-dead cursor-move methods
     (`move_audiobookshelf_show_cursor`, `move_audiobookshelf_show_rows`,
     `jump_audiobookshelf_show_cursor`, `enter_/leave_/move_ episode_selection`,
     `cycle_audiobookshelf_filter`) and their sole callers; KEEP the effect methods
     (`select_audiobookshelf_show`, `start_audiobookshelf_detail`, `play/enqueue_..._episode`,
     `selected_audiobookshelf_queue_item`, `audiobookshelf_refresh`) — the shell continues to
     call them.
- Test boundary: adapt `components/audiobookshelf_podcast_component_tests.rs` to the typed
  intents (currently asserts `msg.is_some()`, `component.cursor()==0`); keep the existing test
  that renders to `TestBackend` (it stays App-free). Tests that drive `handle_key_audiobookshelf_library`
  directly (the `tests_podcast.rs` interaction/persistence blocks at `tests_podcast.rs:165-430`,
  `tests_podcast_playback.rs`, `tests_podcast_context_menu.rs`) need rehoming around the
  shell boundary — the same App-rewrite the 5.3a seek task did. **Note the 5.3d policy:**
  do NOT write differential tests that pin drift; existing tests that assert current interaction
  are adapted to the new shell boundary (end-of-test to component).

- Required checks: `cargo check -p mbv`, `cargo clippy --workspace --all-targets`,
  `cargo nextest run -p mbv abs_podcast podcast`, full `cargo nextest run -p mbv`,
  `cargo fmt --all`, `ast-grep scan` (the 5.3d units use the compiler gate + existing coverage;
  do not add behavior-preservation tests).

---

## 6. Blockers and smallest design decision

- **No hard blocker for B1.** Cover-fetch is NOT triggered by the podcast key path and stays in
  the legacy renderer during B1, so it does not block the interaction move. The shared Vec is
  not a blocker either: B1 only stops mutating interaction fields on the podcast input path;
  the content/cache stores remain App-owned in the same struct, so the existing 15+
  content authorities are untouched.
- **A real coupling to call out for the FOLLOW-ON unit (B2/B):** you cannot delete
  `selected_id`/`episode_selection`/`episode_filter`/`scroll` from
  `AudiobookshelfBrowseState` until you relocate the three unrelated readers that just read
  them: (i) position persistence `library_position_state.rs:231-233` (+ `activate` restore),
  (ii) playback-queue resolution `browse_actions.rs:271-276` (`selected_audiobookshelf_queue_item`),
  (iii) the legacy renderer `render/components/audiobookshelf.rs` (cursor/`cursor()`). That is a
  later unit (B2) in the same family; B1 must NOT try to delete the fields.
- A3 — the smallest design decision for B1: choose the intent vocabulary (one `ShellRequest`
  variant carries the resolved show/episode id, vs. the current "forward the raw key"; the
  former is the established pattern in 5.3a modules that passed items at the boundary, mirror the
  Home/Feeds `Play(guid)`/`Enqueue(guid)` intents in `msg.rs`). No new design doc needed; mirror
  `FeedsPlay`/`FeedsEnqueue` (`shell.rs:578,582`) and the `BrowserComponent` typed-target
  pattern.

## 7. Entry point for the implementer

Open `src/app/components/audiobookshelf_podcast.rs` first (`on`/`handle_key`), then
`shell_audiobookshelf_podcast.rs` (routing + mount), then `input_browse_dispatch.rs` (what to
delete). These three define the entire boundary.

## 8. Ready-to-send implementer prompt (start from `2c6cce5`)

```
ABS podcast Phase B, unit `5.3d / ABS podcast interaction authority`, on
feat/migrate-tui-to-tuirealm at `2c6cce5` (clean). Read-first: AGENTS.md
(run `rtk` prefix on commands), design.md D14, tasks.md 5.3d + its bulletin
scoping, scoping-5.3d-mirrors.md (the "needs full ownership move" line), and
docs/architecture/interactive-surface-ledger.md. Do NOT touch ABS book
browser or any other surface. Do NOT delete `audiobookshelf_browse` content
fields; keep them App-owned for content/cache (library, shows, episodes,
detail_cache, detail_loading, loading_pages, total, next_page, error,
progress). Do NOT delete the legacy renderer in this unit and do NOT move
which cover-fetch seam.

Goal: make `AudiobookshelfPodcastComponent` the sole keyboard authority for
the ABS podcast browse; delete the legacy podcast key handler.

1. In `components/audiobookshelf_podcast.rs` expand `handle_key` to cover all
   podcast keys (Up/Down/Left/Right, PageUp/PageDown, Home/End, `[`/`]`,
   Enter, Space, Esc/Backspace, Ctrl+A), moving the cursor/episode/filter
   locally, and emit typed intents in `components/msg.rs` (new `ShellRequest`
   variants, e.g. PodcastShowSelected{index/show_id} + PodcastPlayEpisode +
   PodcastEnqueueEpisode + PodcastOpenModal, mirroring `FeedsPlay`/`FeedsEnqueue`).
2. In `shell_audiobookshelf_podcast.rs` add handlers that call the existing
   `App` effects on the selected show/episode (not
   `select_audiobookshelf_show` retains position+detail-fetch + cover-fetch,
   `play_selected_audiobookshelf_episode`, `enqueue_...`, `open_podcast_selection_modal`),
   then  re-run `push_audiobookshelf_podcast_content()`.
3. In `shell.rs` replace the `AudiobookshelfPodcastKey` raw-key match with the
   new typed variant matcher.
4. In `input_browse_dispatch.rs` delete `handle_key_audiobookshelf_library`
   and the Podcast arm of `handle_key_browse_dispatch`; keep the Book arm.
5. In `audiobookshelf_browse_actions.rs` delete the now-unreachable cursor-move
   publish methods (single callers were the deleted handler); KEEP effect
   methods and `refresh`.
6. Tests: update `audiobookshelf_podcast_component_tests.rs` for typed intents;
   re-home `tests_podcast*.rs` interaction assertions to the shell boundary
   (the App-owner assert pattern already applied in 5.3a/5.3b). Follow the
   5.3d test policy (no behavior-preservation-only tests; keep only the
   migration-to-component coverage).
7. Verify: `cargo check -p mbv`; `cargo clippy --workspace --all-targets`;
   `cargo nextest run -p mbv abs_podcast podcast`; full `cargo nextest run -p mbv`;
   `cargo fmt --all -- --check`; `cargo ast-grep scan` (only 69 pre-existing
   render/screens diagnostics remain; none in your files). No committed control
   until you check. Do not commit that any project artifact.

## 9. Follow-on (B1.Next) after B1 (scope only, not to run now)

`render/components/audiobookshelf.rs` full delete + `widgets.rs:551-567` podcast branch
removal; relocate LayoutMain `audiobook_podcast_area`/right/hero/left computation into a
shell/`render_audiobook_podcast_component` slice (layout.rs:105-164, widgets.rs audio-arm stays
Book-only); move cover-fetch into `push_audiobookshelf_podcast_content`. Only then can
`AudiobookshelfBrowseState` interaction fields be pruned (with the three readers in
`library_position_state.rs`, `browse_actions.rs`, legacy render moved). All of that is podcast-family,
still excluding book.

## 8. Residual risks (scoped)

- R1 (mitigated for B1): legacy renderer records selected cursor/episode while the component is
  now authoritative. During B1 the renderer is painted *under* the component; if the two diverge
  the offer layer highlight is stale but covered by the component. Verified acceptable as an
  intermediate state; deleted in B2 where it would otherwise show a second selection caret.
- R2: `selected_id`/`episode_selection` fields still read by 3 non-shell authorities after B1
  (position/queue/render). B1 must not try to prune them — regression if it prunes. Documented in §6/§8.
- R3: cover-fetch relocation to the projection seam is only REQUIRED in B2; if any author teams it
  into B1 make sure to keep the `images_enabled` + dedupe guards (it is idempotent today).
- R4: narrow-mode `Enter` opens the podcast selection modal; keep that path App-side intact. B1 must
  only remove the keyboard handler, not the modal-affecting modal actions (`audiobookshelf_podcast_modal_actions.rs`)
  which the modal relies on. The Level 5.3c modals are already shipped. Do not re-wire modal.
- R5: tests referencing `handle_key`, `handle_key_audiobookshelf_library`, or
  `handle_key_browse_dispatch` for the podcast surface may no longer run after deletion — re-home
  at the shell, do not leave behind orphaned assertions.
- R6: possible pre-existing visual mask: the component currently paints its hero (`render_podcast_hero`)
  without the cover image, so the legacy cover display may already be over-painted for podcasts.
  Unverified; out of scope for B1; flag to the maintainer rather than fixing inside B1.

## 9. Verification map (evidence)
- fields, reader/writer map: `app_struct.rs:60-62`, `types_audiobookshelf_browse.rs:50-67`,
  `components/audiobookshelf_podcast.rs:24-26+51-60`, `render/components/audiobookshelf.rs` all,
  `audiobookshelf_browse_actions.rs`, `library_position_state.rs:230/272/281`,
  `input_browse_dispatch.rs:55-64+176-230`, `lib_event_actions.rs`, `service_actions.rs`,
  `run_loop_drains.rs` — same-name calls keep content writes.
- cover fetch: `render/components/audiobookshelf.rs:298-337` + `images.rs:334-344` (dedupe gate).
- layout area read: `shell_audiobookshelf_podcast.rs:82`, `layout.rs:105,152-164`.
- key forward: `components/audiobookshelf_podcast.rs:84-135+247`, `msg.rs:292`, `shell.rs:723-731`,
  `input.rs:176-198`, `input_browse_dispatch.rs:59-64`.
- ledger row: `docs/architecture/interactive-surface-ledger.md` (ABS podcast = `component`,
  unchanged in B1).
</content>
</invoke>