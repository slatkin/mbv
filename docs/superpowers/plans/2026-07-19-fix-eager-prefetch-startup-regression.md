# Fix Eager All-Items Prefetch Startup Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `spawn_all_items_prefetch` from firing during app-startup library-position restore, where it competes with every other library's concurrent restore fetch and stalls the default library's first paint by roughly a second.

**Architecture:** `spawn_all_items_prefetch(lib_idx)` eagerly fetches and JSON-parses every item in a library (full `Fields` set including `People`/`MediaStreams`) to warm an optional cache (`BrowseLevel.all_items`) that only exists to make in-library search (`/`) open instantly. It has two call sites: `handle_lib_loaded` (user navigates into a library — safe, first paint already happened) and `handle_restored_library_position` (fires for every library restored at app startup — unsafe, races with N other concurrent startup fetches). The fix removes the second call site only. Search's existing lazy-fetch fallback (`spawn_search_items_load`, unchanged) already handles the case where `all_items` isn't warm, so no functionality is lost — restoring a library position simply goes back to how it behaved before commit `9440fbf` (2026-07-14, "feat: restore sticky library position across views"), which introduced this call site.

**Tech Stack:** Rust, no new dependencies. Existing `std::sync::mpsc` channels, `std::thread`, `serde_json`, the repo's existing raw-`TcpListener` test-mock convention (see `crates/mbv-core/src/api.rs`'s `local_listener_url()`/`report_stopped_for_shutdown_stalls_with_one_attempt_and_no_retry` test and `src/app/input.rs`'s `RecursiveFetchServer` test helper for precedent — this plan's Task 2 writes a smaller, single-purpose version of the same pattern, not a shared abstraction).

## Global Constraints

- All work happens in the existing worktree at `/home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing`, on the existing branch `issue-260-load-timing`. Do **not** create a new worktree or branch — one already exists with a prior diagnostics-only commit (`diag: time get_items_sorted HTTP/parse and spawn_browse thread total (#260)`) that this work continues on top of. Run every command from that directory (`cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing && ...` or an absolute-path equivalent) — do not touch the main checkout at `/home/slatkin/Dev/mbv`, which has a pre-commit hook that blocks direct commits on `main`.
- This is issue **#260** on `github.com/slatkin/mbv`. Reference it in commit messages as `(#260)`.
- No `Co-Authored-By:` trailers in commit messages (repo-wide convention already reflected in prior commits on this branch).
- `cargo build --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets` must all be clean (no new warnings, no failures) before each commit in this plan.
- Do not touch the `handle_lib_loaded` call site of `spawn_all_items_prefetch` (`src/app/actions.rs`) — it is correct as-is and out of scope. Only `handle_restored_library_position`'s call site changes.

---

### Task 1: Remove the eager prefetch call from the startup restore path

**Files:**
- Modify: `src/app/actions.rs` — the `handle_restored_library_position` method (find it with Serena's `find_symbol` on name path `impl App[1]/handle_restored_library_position` in `src/app/actions.rs`, or `grep -n "fn handle_restored_library_position" src/app/actions.rs` — do not hardcode a line number, this file has ~4500+ lines and shifts easily).

**Interfaces:**
- Consumes: nothing new.
- Produces: nothing new. This task is a pure deletion plus a comment; no signatures change.

- [ ] **Step 1: Confirm current state**

Run:
```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
grep -n "spawn_all_items_prefetch" src/app/actions.rs
```

Expected output — exactly **four** matches: the `fn spawn_all_items_prefetch` definition itself, one call inside `handle_lib_loaded`, one call inside `handle_lib_refreshed`, and one call inside `handle_restored_library_position`. `handle_lib_refreshed` fires from `refresh_after_stop` (post-playback library refresh, not app startup) — it is **not** part of this bug and must stay untouched; only `handle_restored_library_position`'s call is in scope. The current body of `handle_restored_library_position` looks like this (confirm your file matches before editing — if it doesn't, stop and re-read the function before proceeding, the surrounding guard logic may have changed):

```rust
    fn handle_restored_library_position(
        &mut self,
        lib_idx: usize,
        scope: LibraryPositionScope,
        requested_position: crate::config::LibraryPosition,
        position: crate::config::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    ) {
        if self.saved_library_position(lib_idx, scope).as_ref() != Some(&requested_position) {
            return;
        }
        if self.active_library_position_scope_for(lib_idx) != Some(scope) {
            return;
        }
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            lib.apply_library_position(position.clone(), nav_stack);
        }
        self.maybe_refresh_feed_groups_after_refresh(lib_idx);
        let restored = self
            .libs
            .get(lib_idx)
            .map(|lib| lib.library_position_snapshot());
        if restored.as_ref() != self.saved_library_position(lib_idx, scope).as_ref() {
            if let Some(restored) = restored {
                self.replace_saved_library_position(lib_idx, scope, restored);
            }
        }
        self.spawn_all_items_prefetch(lib_idx);
    }
```

- [ ] **Step 2: Remove the eager prefetch call, document why**

Replace the method body's final line. Delete this line entirely:

```rust
        self.spawn_all_items_prefetch(lib_idx);
```

Replace it with this comment (no code — the method now ends with the `if restored.as_ref() != ...` block above it):

```rust
        // Deliberately no `spawn_all_items_prefetch` call here (unlike
        // `handle_lib_loaded`'s sibling call, which is safe): this method
        // fires for every library restored at app *startup*, all
        // concurrently. Eagerly fetching+parsing a whole library's worth of
        // full-field items (People, MediaStreams, ...) here piles CPU-bound
        // JSON parsing on top of N other libraries' simultaneous restore
        // fetches and visibly stalls first paint of the default library
        // (#260). `all_items` is a pure cache for instant `/`-search open
        // (see `spawn_search_items_load`'s lazy fallback in
        // `input.rs`/`handle_lib_event`'s `SearchItemsLoaded` handling) --
        // nothing here requires it to be warm. If you're tempted to add
        // this back, don't: benchmark against a library with 500+ items
        // first and check `~/.local/state/mbv/mbv.log` for `parent=<id>`
        // `http=`/`parse=` timings from `get_items_sorted`.
```

The method's closing brace stays where it was; you're replacing one statement with a comment, not restructuring the function.

- [ ] **Step 3: Use Serena to apply the edit**

Use `replace_symbol_body` on `impl App[1]/handle_restored_library_position` in `src/app/actions.rs` with the full new body (identical to Step 1's block, but with the final line replaced per Step 2). Do not use a raw text editor — this file is large and a symbol-scoped edit avoids any risk of matching the wrong occurrence of a similar-looking guard clause elsewhere in the file.

- [ ] **Step 4: Verify the call site is gone, sibling call site untouched**

Run:
```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
grep -n "spawn_all_items_prefetch" src/app/actions.rs
```

Expected — exactly **three** matches now: the `fn spawn_all_items_prefetch` definition, the surviving call inside `handle_lib_loaded`, and the surviving (untouched, out-of-scope) call inside `handle_lib_refreshed`. If you still see a call inside `handle_restored_library_position`, the edit didn't take — re-check Step 3.

- [ ] **Step 5: Build**

Run:
```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
cargo build --workspace 2>&1 | tail -30
```

Expected: `Finished` with no errors, no new warnings.

- [ ] **Step 6: Commit**

```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
git add src/app/actions.rs
git commit -m "fix: stop eager all-items prefetch on startup library-position restore (#260)

handle_restored_library_position fires for every library restored at
app startup, concurrently. Its spawn_all_items_prefetch call (added by
9440fbf, 2026-07-14, unrelated to that commit's actual sticky-position
feature) eagerly fetches and parses an entire library's full-field item
set on top of every other library's simultaneous restore fetch, which
visibly stalls first paint of the default library. all_items is a pure
cache for instant in-library search-open (/) and has an existing lazy
fallback (spawn_search_items_load) for when it's not warm, so nothing
breaks -- this just stops paying the cost eagerly at startup. The
sibling call in handle_lib_loaded (user navigates into a library,
after first paint) is untouched and still warms the cache there."
```

---

### Task 2: Regression test proving startup restore no longer fetches eagerly

**Files:**
- Modify: `src/app/mod.rs` — add a new `#[test]` inside the existing `#[cfg(test)] mod tests { ... }` block, next to the other `RestoreLibraryPosition`-related tests (search `grep -n "fn ensure_lib_loaded_for_visible_power_library_accepts_restore_from_queue_focus" src/app/mod.rs` to find them — put the new test immediately after that one).

**Interfaces:**
- Consumes: `App::handle_lib_event`, `LibEvent::RestoreLibraryPosition`, `App::replace_saved_library_position`, `LibraryPositionScope::Power`, `BrowseLevel`, `make_app_stub()`, `make_item(name, item_type)` — all pre-existing test helpers already used by the neighboring test in this same file (see Step 1's reference code below for exact call shapes).
- Produces: nothing consumed by later tasks — this is the last task in the plan.

**Why this shape:** `spawn_all_items_prefetch` only does network I/O when the restored level is *not* fully loaded (`items.len() < total_count` — see `is_fully_loaded()` at `src/app/mod.rs:316-318`). The existing `RestoreLibraryPosition` test in this file restores a level with `items: make_items(2), total_count: 2` (fully loaded), which never exercised the prefetch path even before this fix — so it can't regress-test this bug. This new test deliberately restores a level with `total_count` greater than `items.len()`, points `App`'s `EmbyClient` at a local mock TCP listener (same idiom as `crates/mbv-core/src/api.rs`'s `local_listener_url()` test helper), and asserts the listener receives **zero** connections shortly after the restore event — proving no background fetch was spawned. Before Task 1's fix, this same test would have seen exactly one connection (the prefetch's `GET /Users/.../Items?...Limit=50` request) and failed.

- [ ] **Step 1: Find the insertion point and copy the neighboring test's setup shape**

Run:
```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
grep -n "fn ensure_lib_loaded_for_visible_power_library_accepts_restore_from_queue_focus" src/app/mod.rs
```

This is the existing test whose `App`/library/`LibraryPosition` setup you're modeling. Read it in full (`find_symbol` on that test name in `src/app/mod.rs`, `include_body=true`) before writing the new one — reuse its exact field names (`LibraryPositionLevel { parent_id, title, focused_item_id, cursor_index, item_types, unplayed_only, sort_by, sort_order }`, `LibraryTab { library, nav_stack, search, feed_home_video, power_detail_scroll, album_track_focus, artist_header_focus }`) since `LibraryPosition`/`LibraryTab`/`BrowseLevel` field lists are easy to get subtly wrong by guessing.

- [ ] **Step 2: Write the failing test**

Add this test immediately after `ensure_lib_loaded_for_visible_power_library_accepts_restore_from_queue_focus` in `src/app/mod.rs`'s test module:

```rust
    #[test]
    fn restoring_library_position_does_not_eagerly_prefetch_all_items() {
        use std::io::Read;
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        // Minimal local server: we're proving *absence* of a request, so we
        // only need to count accepted connections, not answer them. See
        // crates/mbv-core/src/api.rs's `local_listener_url()` test helper
        // for the same non-blocking-accept-with-deadline idiom.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_for_thread = connection_count.clone();
        let server_handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(400);
            while std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        connection_count_for_thread.fetch_add(1, Ordering::SeqCst);
                        let mut buf = [0u8; 1024];
                        let _ = stream.read(&mut buf);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        let mut app = make_app_stub();
        {
            let mut client = app.client.lock().unwrap();
            client.config.server_url = base_url;
            client.user_id = "user-1".into();
            client.token = "token-1".into();
        }
        app.queue_view = QUEUE_VIEW_POWER;
        app.tab_idx = 1;
        app.power_focus = PowerFocus::Queue;
        app.power_left_tab = 1;
        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.collection_type = "movies".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: 0,
            album_track_focus: None,
            artist_header_focus: None,
        });
        let power_position = crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Power".into(),
                focused_item_id: Some("id1".into()),
                cursor_index: 1,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
            }],
            ..Default::default()
        };
        app.replace_saved_library_position(0, LibraryPositionScope::Power, power_position.clone());
        app.ensure_lib_loaded_for(0);

        // Restore a level that is NOT fully loaded (2 items out of a
        // reported 50) -- this is the condition under which
        // spawn_all_items_prefetch actually does network I/O
        // (is_fully_loaded() is items.len() >= total_count).
        app.handle_lib_event(LibEvent::RestoreLibraryPosition {
            lib_idx: 0,
            scope: LibraryPositionScope::Power,
            requested_position: power_position.clone(),
            position: power_position,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Power restored".into(),
                items: make_items(2),
                total_count: 50,
                cursor: 1,
                scroll: 0,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
        });

        // Give a background thread every reasonable chance to have
        // connected by now if the eager prefetch were still wired up.
        std::thread::sleep(std::time::Duration::from_millis(300));
        server_handle.join().unwrap();

        assert_eq!(
            connection_count.load(Ordering::SeqCst),
            0,
            "restoring a library position must not eagerly fetch all items \
             (spawn_all_items_prefetch should not be called from \
             handle_restored_library_position -- see #260)"
        );
        assert_eq!(app.libs[0].nav_stack[0].title, "Power restored");
        assert!(app.libs[0].nav_stack[0].all_items.is_none());
    }
```

- [ ] **Step 3: Run the test against the pre-fix code to confirm it would have caught the regression**

This step is retroactive verification, not a strict TDD red step (Task 1's fix is already committed). You're temporarily reintroducing the bug on top of your commits, confirming the new test catches it, then discarding that temporary change.

First, view Task 1's commit to see the exact line it removed:
```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
git show HEAD~1 -- src/app/actions.rs | grep -A2 -B2 "spawn_all_items_prefetch"
```

Then use Serena's `find_symbol` on `impl App[1]/handle_restored_library_position` in `src/app/actions.rs` and temporarily add `self.spawn_all_items_prefetch(lib_idx);` back as the method's last statement (undoing Task 1 locally, without committing — do not use `git` for this part, a direct symbol edit is simpler than reconstructing history). Then run:
```bash
cargo test --workspace restoring_library_position_does_not_eagerly_prefetch_all_items 2>&1 | tail -30
```
Expected: **FAIL** — `connection_count` is 1 (or more), assertion panics with the message from Step 2. This confirms the test is actually exercising the bug.

Then discard the temporary reintroduction so the file matches what Task 1 committed:
```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
git checkout -- src/app/actions.rs
```

- [ ] **Step 4: Run the test against the real (fixed) code**

```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
cargo test --workspace restoring_library_position_does_not_eagerly_prefetch_all_items 2>&1 | tail -20
```

Expected: **PASS**.

- [ ] **Step 5: Run the full workspace test suite and clippy**

```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
cargo test --workspace 2>&1 | tail -40
cargo clippy --workspace --all-targets 2>&1 | tail -40
```

Expected: all tests pass (the pre-existing suite plus this new one), clippy shows zero new warnings (there may be pre-existing warnings unrelated to this change — do not fix unrelated warnings as part of this task, note them instead if present).

- [ ] **Step 6: Commit**

```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
git add src/app/mod.rs
git commit -m "test: pin startup library-position restore against eager prefetch (#260)

Regression test for the fix in the previous commit: restores a
not-fully-loaded library level via RestoreLibraryPosition and asserts
a local mock server sees zero connections, proving
spawn_all_items_prefetch is not reachable from
handle_restored_library_position. Confirmed this test fails against
the pre-fix code (spawn_all_items_prefetch call temporarily
reinstated) before confirming it passes against the fix."
```

---

## Final Verification

- [ ] **Full workspace check, one more time, from a clean state**

```bash
cd /home/slatkin/Dev/mbv/.claude/worktrees/issue-260-load-timing
cargo build --workspace 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -50
cargo clippy --workspace --all-targets 2>&1 | tail -40
git log --oneline -3
git status --short
```

Expected: clean build, all tests pass (including `restoring_library_position_does_not_eagerly_prefetch_all_items`), no new clippy warnings, two new commits on top of the branch's existing diagnostics commit, clean working tree (nothing uncommitted).

Do not push, open a PR, or merge — leave the branch as-is for review.
