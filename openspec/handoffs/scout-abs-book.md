# Scout — ABS Book typed-input / interaction-reader / legacy-render (5.3d.13)

Read-only from HEAD. Scope: the Audiobookshelf **book** browser surface. No podcast,
no cover relocation, no doc edits performed here. Companion to the already-landed
Phase-A push-helper handoff (`openspec/handoffs/scout-abs-book-phase-a.md`).

## State summary (important framing)

The book browser is **currently half-converted**: a TuiRealm `AudiobookshelfBookComponent`
already exists, is mounted, and renders; AND the legacy `App::render` library path
still renders the same surface. The component's interaction `on()` **locally mutates
its own copy of state and then forwards the key to `app.handle_key`**, whose
`handle_key_audiobookshelf_book_library` is the **authoritative effect producer**.
`push_audiobookshelf_book_content` then mirrors the authoritative App state back into
the component, overwriting the component's local mutation. So the component's local
`move_book`/`cycle_bucket`/`move_chapter` are *vestigial* (overwritten next push);
the App path is canonical. This matches AGENTS.md "component row is half-converted by
design".

---

## 1. Typed-input surface

### 1a. Component entry (`AppComponent::on`) — TuiRealm event layer
`src/app/components/audiobookshelf_book.rs`
- `on(&mut self, &Event<UserEvent>)` (impl `AppComponent`, lines ~250-262): routes
  `Event::Keyboard(key)` → `handle_key`, `Event::Mouse(mouse)` → `handle_mouse`, else `None`.
- `handle_key(&mut self, &KeyEvent)` (lines 103-150): **local** moves on its own
  `self.state`, then **always returns** `Some(Msg::Shell(ShellRequest::AudiobookshelfBookKey(
  to_crossterm_key_event(key))))`. Keys handled locally:
  - `[` / `]` (no modifiers) → `cycle_bucket(-1/+1)`
  - `Up`/`k` or `Down`/`j` (if `chapter_selection.is_some()`) → `move_chapter(±1)`
  - `Right` (if chapters_focused) → `chapter_selection = None` (focus browser)
  - `Left` (if !chapters_focused) → `chapter_selection = Some(0)` (focus chapters)
  - `Up`/`k` or `Down`/`j` → `move_book(±1)`
  - `PageUp`/`PageDown` (if !chapters_focused) → `move_book(∓page_size)`
  - `Home`/`End` (if !chapters_focused) → `select_bucket_edge(false/true)`
  - `Esc`/`Backspace` (if chapters_focused) → `chapter_selection = None`
- `handle_mouse(&mut self, &MouseEvent)` (lines 152-183): Left-press hit-tests
  `geometry.book_rows` → `select(idx)`; `geometry.chapter_rows` → `chapter_selection = Some(idx)`;
  `geometry.selector_tabs` → set `selected_bucket` + `select(range.start)`. Returns
  `Some(Msg::Legacy(LegacyTerminalEvent::Mouse(crossterm_mouse)))`. (Mouse is routed to
  the legacy terminal handler, NOT to `app.handle_key`.)

### 1b. Authoritative App effect path (legacy keyboard dispatch)
`src/app/shell_audiobookshelf_book.rs:18-20` `handle_audiobookshelf_book_key` →
`self.app.handle_key(key)`. `App::handle_key` (`src/app/input.rs:76`) runs
`CONTEXT_STACK`; one resolver is `handle_key_browse_dispatch`
(`src/app/input_browse_dispatch.rs:40`) → `handle_key_audiobookshelf_book_library`
(`src/app/input_browse_dispatch.rs:247-319`).

`handle_key_audiobookshelf_book_library(index, key)` keys + effects (authoritative):
- `chapters_focused = layout.main.is_wide_book_active()
   && state.chapter_selection.is_some()` (read via `audiobookshelf_book_browse.get(index)`).
- `[` / `]` (plain, no CTRL/ALT) → `cycle_audiobookshelf_book_bucket(±1)`; **returns** `Some(false)`.
- `Up`/`k`, `Down`/`j`: if `chapters_focused` → `move_audiobookshelf_book_row(±1)`;
  else → `move_audiobookshelf_book_cursor(±1)`.
- `Enter`/`Space` if `chapters_focused` → `activate_audiobookshelf_book_row()` (absolute seek).
- `Right` if `chapters_focused` → `focus_audiobookshelf_book_browser()` (`chapter_selection = None`).
- `Left` if `!chapters_focused && layout.main.is_wide_book_active()` → `focus_audiobookshelf_book_chapters()`
  (`chapter_selection = Some(0)`).
- `PageUp`/`PageDown` if `!chapters_focused` → `move_audiobookshelf_book_cursor(∓lib_page_size())`.
- `Home`/`End` if `!chapters_focused` → `jump_audiobookshelf_book_cursor(false/true)`.
- `Enter` if `!chapters_focused && !layout.main.is_wide_book_active()` → `activate_audiobookshelf_book_parent()`
  (opens narrow chapter `SelectionModal` when inline hero fits, else ordinary play).
- `Space`/`Enter` if `!chapters_focused` → `play_selected_audiobookshelf_book(index)`.
- `Ctrl+'a'` if `!chapters_focused` → `enqueue_selected_audiobookshelf_book(index)`.
- Everything else `_ => {}` (returns `Some(false)` at end).

Effect producers (all in `src/app/audiobookshelf_browse_actions.rs` unless noted):
- `move_audiobookshelf_book_cursor` (412) — clamps to current bucket range, `select_audiobookshelf_book`.
- `jump_audiobookshelf_book_cursor` (431) — bucket start/end.
- `focus_audiobookshelf_book_chapters` (468) / `focus_audiobookshelf_book_browser` (464).
- `cycle_audiobookshelf_book_bucket` (476) / `select_audiobookshelf_book_bucket` (497).
- `move_audiobookshelf_book_row` (526) — `chapter_selection` clamp within `visible_rows`.
- `activate_audiobookshelf_book_row` (549) — absolute seek to `visible_rows()[cursor].start`.
- `play_selected_audiobookshelf_book` (638) / `enqueue_selected_audiobookshelf_book` (690)
  — queue submit/enqueue via `selected_audiobookshelf_book_queue_item`.
- `activate_audiobookshelf_book_parent` (`src/app/audiobookshelf_book_modal_actions.rs:50`) →
  `open_audiobookshelf_book_selection_modal` (book `SelectionModal`, `SelectionModalSource::Book`).

**Key finding (coupling risk):** the component `handle_key` forwards *every* key to
`app.handle_key`, so the App dispatch re-applies the same movement (e.g. bucket cycle).
Both layers start from the same pre-event App state and `push_audiobookshelf_book_content`
re-syncs, so they converge — but the forward means the legacy `handle_key_*` is the
**only** path that may be deleted when the component takes full ownership; the
component's local moves must be promoted to authoritative at that point (or removed if
delegated back to `app`).

---

## 2. Interaction readers (App state read for interaction)

Read by `handle_key_audiobookshelf_book_library` + actions + component mirror:
- `app.audiobookshelf_book_browse: Vec<AudiobookshelfBookBrowseState>`
  (`src/app/app_struct.rs:62`; `AudiobookshelfBookBrowseState` defined
  `src/app/types_audiobookshelf_browse.rs:319`). Per-index fields read for interaction:
  `books`, `selected_id`, `chapter_selection`, `scroll`, `buckets: Vec<SurnameBucket>`,
  `selected_bucket`, `detail_loading`, `detail_cache`, `progress: HashMap<id, …>`.
- `app.tab` + `tab.audiobookshelf_index()` — active book library index.
- `app.audiobookshelf_kind_at(index)` → `AudiobookshelfBrowseKind::Book` guard.
- `app.layout.main.is_wide_book_active()` (`src/app/layout.rs:211`) — wide vs narrow
  presentation; gates `chapters_focused` and the Left/Right pane-focus keys.
- `app.lib_page_size()` — PageUp/PageDown stride.
- `app.effective_panel_focus()` / `PanelFocus::Library` — computes `focused` for mirror.
- `app.images_enabled()` — image paint toggle for mirror.
- `app.layout.main.audiobookshelf_book_area` — component's render target area
  (read in `render_audiobookshelf_book_component`).
- `app.layout.main.inline_hero_area` — read by `activate_audiobookshelf_book_parent` to
  decide modal vs play.
- Component local readers (`AudiobookshelfBookComponent`): its own `state`
  (`AudiobookshelfBookBrowseState`), `focused`, `geometry: AudiobookshelfBookGeometry`
  (`selector_tabs`, `book_rows`, `chapter_rows` — `src/app/render/components/audiobookshelf_book.rs:17`)
  for mouse hit-testing + render. `set_content` (component `audiobookshelf_book.rs:42`)
  preserves `selected_id`/`chapter_selection`/`scroll`/`selected_bucket` when selected id
  still present, clamps `selected_bucket` to `buckets.len()`.

---

## 3. Legacy render geometry

### 3a. Legacy `App` renderer (to be deleted at ownership move)
Entry: `App::render_library` (`src/app/render/components/widgets.rs:516`) →
`render_audiobookshelf_library` (widgets.rs:549) — for `AudiobookshelfBrowseKind::Book`
dispatches to `render_audiobookshelf_books` (`src/app/render/components/audiobookshelf_books.rs:51`).
- Wide: `render_wide_audiobookshelf_books` (audiobookshelf_books.rs:102) →
  `render_audiobookshelf_book_right_pane_wide` (book_browser.rs:24) →
  `render_audiobookshelf_book_bucket_pills` (book_browser.rs:96) +
  `render_audiobookshelf_book_browser_rows` (book_browser.rs:132) + hero
  (`render_audiobookshelf_book_hero` audiobookshelf_books.rs:321,
  `render_audiobookshelf_book_rows` audiobookshelf_books.rs:458).
- Narrow: `render_narrow_audiobookshelf_books` (audiobookshelf_books.rs:187) →
  `render_audiobookshelf_book_right_pane_narrow` (book_browser.rs:70) → same rows.
- Layout: wide uses `library_arrangement::wide_library_panes(area, 0, PANE_PAD_Y)` →
  recessed `left_panel`/`right_panel` with `▔`/`▁` border glyphs; pills via
  `hero_left::hero_on_left_right_pane`; browser via `padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y)`.
  Narrow uses `hero_left::pill_bar_areas(area)`.
- `render_audiobookshelf_book_browser_rows` writes `LayoutMain` interaction geometry:
  `layout.hero_area`, `layout.selected_item_rect`, `layout.left_row_targets`
  (`Vec<Option<LibraryRowTarget::Book>>`), `layout.audiobookshelf_book_right_area`,
  and `layout.selector_tabs` (from `render_audiobookshelf_book_bucket_pills`).
- Inline detail flow via `inline_detail_flow`/`selected_detail_shell`
  (HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS) — recomputed in the component's
  `render_book_browser`/`render_book_hero` (`render/components/audiobookshelf_book.rs`).

### 3b. Component renderer (already live)
`render_audiobookshelf_book_content` (`src/app/render/components/audiobookshelf_book.rs:28`)
called from `Component::view` (component `audiobookshelf_book.rs:228`). Paints:
placeholder when empty; `book_hero_plan` (author/overview/cover) → `render_book_hero`
(returning `HomeImagePaint::AudiobookshelfCover` when `plan.image_key` set),
`render_book_rows` (chapter/audio-file table), `render_book_pills` (bucket pills),
`render_book_browser`. Computes `AudiobookshelfBookGeometry` (selector_tabs/book_rows/
chapter_rows) for mouse hit-test. Reuses `hero_left::shared_hero_presentation`,
`wide_library_panes`, `pill_bar_areas` — same arrangement primitives as the legacy path.

**Key finding (double-paint risk):** `app.render` (legacy) and `render_audiobookshelf_book_component`
(shell draw, `src/app/shell.rs:1124`) both run each frame into the library area. The
legacy `render_library` Book arm is NOT gated off, so the surface is currently painted
twice. The legacy renderer must be removed (delete the `AudiobookshelfBrowseKind::Book`
arm in `render_audiobookshelf_library` + `render_audiobookshelf_books` +
`render_audiobookshelf_book_*`/`render_audiobookshelf_book_browser.rs`) when the component
takes full ownership; otherwise the component underpaint is wasted/conflicting.

---

## 4. Mount / sync adapter

`shell_audiobookshelf_book.rs`:
- `Model::abs_book_id: Option<ComponentId>` (`src/app/shell.rs:62`, init `None` at shell.rs:106).
- `sync_audiobookshelf_book()` (32-67): **mount-lifecycle only**. Mounts
  `AudiobookshelfBookComponent::new()` at `ComponentId::Browser(BrowserKey{service:
  Audiobookshelf, library_id: app.audiobookshelf_libraries[index].id, kind: AudiobookshelfBook})`
  when `app.tab == AudiobookshelfLibrary(index)` with kind `Book`; unmounts on id change.
  Called **per-frame** at `src/app/shell.rs:1067`. On fresh mount, immediately calls
  `push_audiobookshelf_book_content()` (line 63).
- `push_audiobookshelf_book_content()` (76-108): event-scoped projection. Early-return
  unless `abs_book_id` set AND active tab is Book kind. Reads
  `app.audiobookshelf_book_browse.get(index)` (snapshot), `focused =
  effective_panel_focus()==Library`, `images_enabled`; `downcast_mut::<AudiobookshelfBookComponent>()`
  → `set_content(snapshot, focused, images_enabled)`. Idempotent.
  **Call sites (writers of projected inputs):** `src/app/shell.rs:273, 286, 343, 425, 548, 836`
  (plus the fresh-mount push at 63). NOT per-frame.
- `render_audiobookshelf_book_component(frame)` (110-118): called at `src/app/shell.rs:1124`
  into `app.layout.main.audiobookshelf_book_area`; drains `take_image_paint` →
  `app.paint_home_image`.

Component ↔ App state mirrored: full `AudiobookshelfBookBrowseState` snapshot +
`focused` + `images_enabled`. Component local `selected_id/chapter_selection/scroll/
selected_bucket` preserved across pushes (see §2). Sync frequency: mount only on tab
change; content push at the 6 event seams above (async drains, setup, every-key).

---

## 5. Smallest safe implementation units

This is the *full ownership move* (the scheduled follow-up to Phase A). Keep ≤6 files/unit.

**Unit A — Delete legacy renderer + gate legacy dispatch (render ownership).**
- `src/app/render/components/widgets.rs`: remove the `AudiobookshelfBrowseKind::Book`
  arm in `render_audiobookshelf_library` (lines 549-560) so Book no longer paints legacy.
- `src/app/render/components/audiobookshelf_books.rs`: delete `render_audiobookshelf_books`
  (51), `render_wide_audiobookshelf_books` (102), `render_narrow_audiobookshelf_books`
  (187), `render_audiobookshelf_book_hero` (321), `render_audiobookshelf_book_rows` (458)
  (Book-only; keep any Podcast-shared helpers if used elsewhere — verify with grep).
- `src/app/render/components/audiobookshelf_book_browser.rs`: delete whole file
  (`render_audiobookshelf_book_right_pane_wide/narrow`, `render_audiobookshelf_book_bucket_pills`,
  `render_audiobookshelf_book_browser_rows`).
- Risks: `LayoutMain` fields (`left_row_targets`, `selected_item_rect`, `hero_area`,
  `audiobookshelf_book_right_area`, `selector_tabs`) are written by the deleted code and
  read elsewhere (e.g. mouse-target resolution, conformance tests). Grep every reader
  before deleting; migrate needed geometry into `AudiobookshelfBookGeometry` (component).
  Tests `render/tests_audiobookshelf_books*.rs` drive the legacy path → must be deleted or
  repointed to the component view.

**Unit B — Component owns interaction (delete legacy `handle_key_*` for book).**
- `src/app/input_browse_dispatch.rs`: remove `handle_key_audiobookshelf_book_library` (247-319)
  + its dispatch arm at `handle_key_browse_dispatch` (around 62, the `Book` match). Route
  book keys entirely via the component `on()` (already forwards). Promote the component's
  local `move_book`/`cycle_bucket`/`move_chapter`/focus toggles to be authoritative OR keep
  delegating to `app` effect methods via typed `Msg`. Decide with D14
  (`openspec/changes/migrate-tui-to-tuirealm/design.md`) before editing.
- `src/app/audiobookshelf_browse_actions.rs`: keep `move_*`/`cycle_*`/`focus_*`/
  `activate_*`/`play_*`/`enqueue_*` as the canonical effect impls; they become the
  component's effect backend (invoked via typed `Msg` instead of `app.handle_key`).
- `src/app/shell_audiobookshelf_book.rs`: change `handle_audiobookshelf_book_key` from
  `app.handle_key(key)` to no-op or a typed `Msg` dispatch, then delete once Unit B lands.

**Unit C — Typed `Msg`/`ShellRequest` (optional, pairs with B).**
- `src/app/components/msg.rs:357` `ShellRequest::AudiobookshelfBookKey(crossterm::event::KeyEvent)`
  and the component's `Msg::Legacy(LegacyTerminalEvent::Mouse)` are the current bridge.
  Convert to a typed `Msg::AudiobookshelfBook{ MoveBook, CycleBucket, FocusChapters, … }`
  so the component stops round-tripping through `app.handle_key`. Per AGENTS.md D14, do
  this only as part of the group-5 ownership move, not opportunistically.

**Keep untouched in 5.3d.13:** `set_content`, component `new`/`view`, Phase-A push seams,
`abs_book_id`/`sync_audiobookshelf_book`, App field `audiobookshelf_book_browse` (deletion
is a later group), cover relocation, podcast paths.

## 6. Verification gates (5.3d policy)
`rtk cargo check -p mbv` · `rtk cargo clippy --workspace --all-targets` · `rtk cargo nextest
run -p mbv` · `rtk ast-grep scan` (keep 69-finding baseline, none new in touched files) ·
`rtk make check-code-file-lines` · `rtk cargo fmt --all -- --check`. Preserve
`abs_book_shell_mounts_and_routes_component` (shell_audiobookshelf_book.rs test) and
`audiobookshelf_book_component_tests.rs`.

## 7. Resolved unknowns to confirm with supervisor before writing
- Whether `LayoutMain` geometry written by the legacy browser (`left_row_targets`,
  `selected_item_rect`, `audiobookshelf_book_right_area`, `selector_tabs`) is still read
  by any live surface (queue drag, scrollbar, conformance) after the component owns
  rendering — gate Unit A on this grep.
- Whether the component should keep forwarding every key to `app.handle_key` (current
  double-apply) or own interaction via typed `Msg` (D14).
