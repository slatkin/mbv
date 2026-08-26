# Scout — ABS Book Phase A push-helper unit (5.3d framework removal)

Scope: only the Audiobookshelf **book** per-frame content mirror. No podcast, no
typed-teardown, no legacy-renderer deletion, no cover relocation, no adapters,
no docs, no code edits performed here. Read-only from HEAD `2c6bcce5`.

## 1. Current mirror inputs (exact)

`sync_audiobookshelf_book` — `src/app/shell_audiobookshelf_book.rs:24-97`, called
per-frame at `src/app/shell.rs:954`. It:

1. Computes the active-tab **mount id** from `app.tab`
   (`TabSelection::AudiobookshelfLibrary(index)` where
   `audiobookshelf_kind_at(index)==Book`) + `app.audiobookshelf_libraries`; mounts/
   unmounts `AudiobookshelfBookComponent` on id change (`abs_book_component_id`, lines 32-36).
2. Reads the **snapshot** = `app.audiobookshelf_book_browse.get(index)` (line 61) — the
   entire `AudiobookshelfBookBrowseState` (books, total, next_page, loading_pages,
   selected_id, error, detail_cache, detail_loading_ids, detail_loading, progress,
   chapter_selection, scroll, buckets, selected_bucket).
3. Computes **focused** = `app.effective_panel_focus() == PanelFocus::Library` (line 63).
4. Computes **images_enabled** = `app.images_enabled()` (line 65).
5. Calls `book.set_content(snapshot, focused, images_enabled)` (lines 69-72).

`set_content` — `src/app/components/audiobookshelf_book.rs:42-70`: clones the
snapshot, then **preserves the component's local interaction state**
(`selected_id`, `chapter_selection`, `scroll`, `selected_bucket`) when the selected
id still exists in `books`; sets `focused`/`images_enabled`. The component already
owns its cursor/focus/scroll/bucket; the mirror only pushes async-fetched
**content** + focus + images.

`render_audiobookshelf_book_component` (lines 100-118) additionally drains
`take_image_paint` after `view()` — **render-only, unchanged by Phase A**.

## 2. Writer choke points (App-side canonical `audiobookshelf_book_browse`)

Phase A does **not** edit App-side writers; it pushes at the shell seams where those
writers are already invoked (mirrors the landed podcast Phase A, see §4). Writers:

- **Async books fetch** — `lib_event_actions.rs:215-230`
  (`LibEvent::AudiobookshelfBooksFetched` → `state.append_page_books(page, total, items)`,
  `state.error`). Books + buckets + loading_pages + total + next_page.
- **Async detail fetch** — `lib_event_actions.rs:253-273`
  (`LibEvent::AudiobookshelfBookDetailFetched` → `detail_cache`, `detail_loading`,
  `detail_loading_ids`).
- **Catalog rebuild** — `run_loop_drains.rs:63-64` rebuilds the whole
  `audiobookshelf_book_browse` vec from scratch at catalog completion; `run_loop_drains.rs:84-85`
  assigns the per-index `book_progress` map.
- **Refresh/reset** — `audiobookshelf_browse_actions.rs:352-389`
  (`audiobookshelf_book_refresh`: clears books/total/next_page/error/detail_cache/
  detail_loading_ids/chapter_selection/scroll/loading_pages, marks page 0, reissues fetch).
- **Saved-position restore** — `library_position_state.rs:313-338`
  (`activate_audiobookshelf_book_position`: sets `selected_id` from slot /
  `select(0)`); save at `library_position_state.rs:256` `save_audiobookshelf_book_position`.
- **Setup clear** — `app_audiobookshelf_service_completion.rs:23` clears the vec.
- **Key/cursor writers** — `audiobookshelf_browse_actions.rs` ~396-560
  (`select_audiobookshelf_book`, `move/jump`, `cycle/select_bucket`,
  `focus_audiobookshelf_book_chapters/browser`, `move_audiobookshelf_book_row`).
  The **component already applies its own cursor locally** and `set_content` preserves it,
  so these need no immediate push — content changes they trigger land via the async drains.

## 3. Smallest Phase A push-helper unit

**Exactly 2 production files** (well under the 6-file bound), a line-for-line copy
of the landed podcast Phase A (`shell_audiobookshelf_podcast.rs:24-108`):

1. **`src/app/shell_audiobookshelf_book.rs`** — add
   `pub(super) fn push_audiobookshelf_book_content(&mut self)` (clone of
   `push_audiobookshelf_podcast_content`, lines 77-108): early-return unless
   `abs_book_id` is set and `tab` is the active Book library index; read snapshot,
   `focused = matches!(effective_panel_focus(), PanelFocus::Library)`,
   `images_enabled`; `downcast_mut::<AudiobookshelfBookComponent>()` →
   `set_content(snapshot, focused, images_enabled)`. Then slim `sync_audiobookshelf_book`
   to **mount lifecycle only** (keep mount/unmount + a `push_audiobookshelf_book_content()`
   right after a fresh mount, lines 44-51) — mirror `sync_audiobookshelf_podcast`.
2. **`src/app/shell.rs`** — add `self.push_audiobookshelf_book_content();` at the same
   shared audiobookshelf seams where podcast pushes sit: after `drained_abs_events`
   (line 271), after player events (line 280), after lib_rx drain (line 332), after
   socket drain (line 408), and after the every-key seam (line 525).
   The per-frame `sync_audiobookshelf_book()` call stays at line 954 (mount-lifecycle
   only), exactly as `sync_audiobookshelf_podcast` remains.

**Not touched**: App-side writer files (`lib_event_actions.rs`, `run_loop_drains.rs`,
`audiobookshelf_browse_actions.rs`, `library_position_state.rs`), `set_content`, the
component, typed `Msg`/`ShellRequest` conversion, App-field deletion, renderer.

## 4. Parity constraints

- `set_content` receives the identical 3 inputs (snapshot, focused, images_enabled) —
  component cursor preservation semantics unchanged.
- Fresh mount must push immediately so a newly mounted component paints the current
  snapshot (podcast precedent).
- Duplicate pushes are idempotent (read snapshot, early-return on wrong tab) — safe at
  every shared seam.
- Pushes are event-scoped, deterministic in `App` state; the per-frame sync is *not
  deleted* in Phase A (deletion is the follow-up ownership move) — it keeps only mount
  lifecycle, so the tree stays green (`abs_book_shell_mounts...` still valid).

## 5. Existing test to adapt / add

- **Adapt**: `src/app/shell_audiobookshelf_book.rs` test
  `abs_book_shell_mounts_and_routes_component` (lines 121-165) still calls
  `model.sync_audiobookshelf_book()` directly → still compiles/passes (mount retained).
- **Add** (mirror podcast): one focused Model-boundary test driving
  `sync_audiobookshelf_book()` then `push_audiobookshelf_book_content()` and asserting
  the component received the snapshot (downcast + field probe), matching
  `abs_podcast_shell` pattern. No behaviour-preservation tests (5.3d policy).
- Untouched: `audiobookshelf_book_component_tests.rs` (calls `set_content` directly);
  render tests (`tests_audiobookshelf_books*.rs`) drive the render path, unaffected.

## 6. Required checks (5.3d verification policy)

`rtk cargo check -p mbv` · `rtk cargo clippy --workspace --all-targets` · `rtk cargo
nextest run -p mbv` · `rtk ast-grep scan` · `rtk make check-code-file-lines` · `rtk cargo
fmt --all -- --check`.

## 7. Stop condition if coupling exceeds the bound

If Phase A requires editing any App-side writer file (`lib_event_actions.rs`,
`run_loop_drains.rs`, `audiobookshelf_browse_actions.rs`, `library_position_state.rs`),
or converting the component's key-forward to typed `Msg`, or deleting `App`
fields/`sync_audiobookshelf_book`, **stop** — that is the full 5.3a-style ownership
move (the scheduled follow-up), not the Phase A push helper. Phase A must be pure
two-file seam insertion.

## 8. Landing result

Commit `4f5df745` implemented the two-file helper and five planned async/global
writer seams. Review found two missing safety requirements: the
`AudiobookshelfBookKey` request arm also needed a push, and the helper needed to
repeat the active Book-kind guard before projecting to a possibly stale mount.
Correction `354fc5c0` added both. Focused/full nextest (1,156 passed), cargo
check, workspace clippy, unchanged 69-finding ast-grep baseline with none in
touched files, and fmt passed. Fresh Luna review returned `ACCEPT` for the
corrected cumulative unit. No test hook was added: the existing mount test
executes the fresh-mount push, while component content remains private.

## 9. Original ready-to-send implementer prompt

> Starting from `2c6bcce5` on feat/migrate-tui-to-tuirealm, implement **ABS Book
> Phase A** — the push-helper unit for `sync_audiobookshelf_book` (task 5.3d framework
> removal), copying the already-landed podcast Phase A exactly
> (`src/app/shell_audiobookshelf_podcast.rs:24-108`).
>
> Do **not** touch podcast, typed-interaction teardown, legacy renderer deletion,
> cover relocation, framework adapters, or docs.
>
> Changes (2 production files):
> 1. `src/app/shell_audiobookshelf_book.rs`: add `push_audiobookshelf_book_content()`
>    (read `app.audiobookshelf_book_browse[index]` snapshot when `tab` is the active
>    Book library; `focused = matches!(effective_panel_focus(), PanelFocus::Library)`;
>    `images_enabled`; `downcast_mut::<AudiobookshelfBookComponent>()` →
>    `set_content(snapshot, focused, images_enabled)`; early-return on no mount/right
>    tab). Slim `sync_audiobookshelf_book` to mount lifecycle only + a push right after
>    a fresh mount. Keep the component's cursor-preservation semantics identical.
> 2. `src/app/shell.rs`: add `self.push_audiobookshelf_book_content();` at the same
>    seams as podcast: after `drained_abs_events` (~271), player events (~280), lib_rx
>    drain (~332), socket drain (~408), and the every-key seam (~525). Keep the
>    per-frame `sync_audiobookshelf_book()` (mount-only).
>
> Do not edit App-side writers, `set_content`, the component, or any App field.
> Do not delete `sync_audiobookshelf_book` or the per-frame mount call.
>
> Add one focused Model-boundary test (sync then push, downcast-probe content),
> mirroring `abs_podcast_shell`. Verify: `rtk cargo check -p mbv`, `rtk cargo clippy
> --workspace --all-targets`, `rtk cargo nextest run -p mbv`, `rtk ast-grep scan`,
> `rtk make check-code-file-lines`, `rtk cargo fmt --all -- --check`. Stop and report
> instead of editing App-side writers.
