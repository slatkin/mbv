## Context

See `proposal.md` - Why. The four `LayoutMain::is_wide_*_active()` predicates
(`src/app/layout.rs:208-229`) read fields (`wide_music_right_area`,
`tv_wide_right_area`, `audiobookshelf_podcast_right_area`,
`audiobookshelf_book_wide_right_area`) that are only populated by the render
path *for the frame that just painted*. Two of these fields are themselves
mirrors: `render_audiobookshelf_book_component`
(`src/app/shell_audiobookshelf_book.rs:211`) and
`render_audiobookshelf_podcast_component`
(`src/app/shell_audiobookshelf_podcast.rs:259`) copy a mounted component's
own `geometry().wide`/`geometry().right_area` back into `LayoutMain` after
paint, so those two predicates are one hop further removed from the current
frame than the TV/Music fields.

A paint-free precedent already exists: `App::wide_tv_library_area`
(`src/app/render/components/tv_wide.rs:131-152`) computes
`chrome_geometry` → `right_panel_content_area` →
`hero_left::shared_hero_presentation(lib_area).is_some()` entirely from
`self.terminal_width`/`self.terminal_height` and current panel state, with no
dependency on what painted last frame. `shared_hero_presentation`
(`src/app/render/arrangements/hero_left.rs:45`) is the single owner of the
wide/narrow breakpoint (width ≥ `TWO_COLUMN_THRESHOLD` and height above
`HERO_ON_LEFT_MIN_AREA_HEIGHT`) for every hero-on-left destination (TV,
Music, Audiobookshelf book, Audiobookshelf podcast, Home, Feeds, Movies) per
`openspec/specs/right-panel-arrangements/spec.md` ("The right panel has
exactly two hero presentations"). Because the breakpoint is the same for
every destination, a single generic predicate — not four provider-specific
ones — is sufficient for every boolean "is the right panel wide right now"
decision.

`TerminalObserverEvent::Resize` currently carries no payload
(`src/app/components/msg.rs:60`); the terminal size that produced it is
discarded at the mapping site (`src/app/components/root.rs:72`:
`Event::WindowResize(_, _) => TerminalObserverEvent::Resize`). `Model`'s
`terminal_width`/`terminal_height` are only updated later, when the next
frame's `chrome_geometry` call recomputes layout from the real terminal
size read at paint time — so on the resize tick itself, any decision that
runs before that paint (keyboard dispatch, mount-gate sync) still sees the
pre-resize size.

## Goals / Non-Goals

**Goals:**
- One paint-free method that answers "is the right panel currently in the
  wide breakpoint" for any destination, replacing all four
  `is_wide_*_active()` predicates.
- Make `TerminalObserverEvent::Resize` carry the terminal's new
  width/height and apply them to `Model`/`App` before any other resize
  side effect runs on the same tick.
- Migrate every real (non-test) call site of `is_wide_*_active()` to the
  new predicate (directly, or via `wide_tv_library_area`, which already
  has a paint-free implementation and keeps its TV-specific role).
- Detach the Audiobookshelf book/podcast post-paint wide-flag mirrors so
  `audiobookshelf_book_wide_right_area` and `audiobookshelf_podcast_right_area`
  are no longer written from component-reported geometry as a decision
  input (they may still exist as paint output consumed once per frame for
  geometry, per Decision 4).
- Delete the four `is_wide_*_active()` methods outright.

**Non-Goals:**
- Changing the breakpoint value, the minimum-height guard, or any pane
  geometry. This is a correctness/timing fix, not a presentation change.
- Making every rect a consumer reads paint-free. Once a branch is chosen
  using the new predicate, code inside that branch may still read a
  painted rect for its own geometry (e.g. context-menu anchors) — see
  Decision 4 and the spec delta's "narrow-only or wide-only geometry field"
  scenario.
- Changing `is_wide_tv_library`'s TV-specific content gate (Series-only
  nav level) or any other provider-specific "does this library own the
  wide mount" logic.

## Decisions

### 1. One generic predicate, not four

Add `App::is_right_panel_wide(&self) -> bool` in `src/app/layout.rs` (next
to the predicates it replaces, in the `impl App` block colocated with
`wide_tv_library_area`, i.e. `src/app/render/components/tv_wide.rs`, since
that is where the paint-free geometry pipeline already lives). It factors
the pipeline `wide_tv_library_area` already has, minus the TV-specific
gate:

```rust
impl App {
    /// The right panel's content area for the current terminal size and
    /// panel state, paint-free — `None` when the right panel is not
    /// visible (e.g. Queue-only panel mode). Factored out of
    /// `wide_tv_library_area` so every paint-free breakpoint consumer
    /// shares one pipeline.
    fn right_panel_lib_area(&self) -> Option<Rect> {
        let chrome = chrome_geometry(ChromeGeometryInput {
            area: Rect::new(0, 0, self.terminal_width, self.terminal_height),
            panel_mode: self.effective_panel_mode(),
            panel_focus: self.effective_panel_focus(),
            queue_column_width: self.queue_column_width,
            terminal_width: self.terminal_width,
        });
        chrome.right_visible.then(|| {
            right_panel_content_area(chrome.right_area, self.effective_panel_mode() != PanelMode::Both)
        })
    }

    /// Whether the right panel is in the wide hero-on-left breakpoint right
    /// now, derived paint-free from the current terminal size. Replaces the
    /// four `LayoutMain::is_wide_*_active()` paint-inference predicates:
    /// the breakpoint (`shared_hero_presentation`) is the same for every
    /// hero-on-left destination, so one predicate serves all of them.
    pub(in crate::app) fn is_right_panel_wide(&self) -> bool {
        self.right_panel_lib_area()
            .is_some_and(|area| hero_left::shared_hero_presentation(area).is_some())
    }
}
```

`wide_tv_library_area` is reimplemented in terms of `right_panel_lib_area`
(dropping its inlined copy of the same chrome/right-panel-area steps) but
keeps its own signature and its `is_wide_tv_library(lib_idx)` gate, because
its callers need the resolved `Rect` for a *specific* TV library, not just
a bool:

```rust
pub(in crate::app) fn wide_tv_library_area(&self, lib_idx: usize) -> Option<Rect> {
    if !self.is_wide_tv_library(lib_idx) {
        return None;
    }
    let lib_area = self.right_panel_lib_area()?;
    hero_left::shared_hero_presentation(lib_area).map(|_| lib_area)
}
```

**Alternative considered:** four separate paint-free predicates
(`is_wide_tv_active_now`, `is_wide_music_active_now`, ...), one per
provider, mirroring the names being replaced. Rejected: the underlying
geometry pipeline and breakpoint are identical across all four: a provider
distinction only matters when a caller needs the *area itself* (TV) or a
provider-specific content gate (none of the other three currently have
one), not for a plain wide/narrow bool. Four near-identical functions would
re-fragment the single source of truth the proposal asks for.

### 2. `Resize` carries terminal size, applied eagerly

`TerminalObserverEvent::Resize` (`src/app/components/msg.rs:60`) becomes
`Resize { width: u16, height: u16 }`. The mapping site
(`src/app/components/root.rs:72`) changes from discarding the event's
payload to forwarding it:

```rust
Event::WindowResize(width, height) => TerminalObserverEvent::Resize { width, height },
```

In `apply_terminal_observer` (`src/app/shell.rs:400-432`), the `Resize` arm
sets `model.app.terminal_width`/`terminal_height` from the event fields
*before* the existing `force_clear`/`card_image_states.clear()`/
`push_inline_search_content()` side effects run, so every paint-free
predicate call made later in the same tick (including inside
`push_inline_search_content` and any `sync_mounted_surfaces` mount-gate
check that runs before the next paint) sees the post-resize size:

```rust
TerminalObserverEvent::Resize { width, height } => {
    model.app.terminal_width = width;
    model.app.terminal_height = height;
    model.app.force_clear = true;
    model.app.card_image_states.clear();
    model.app.card_image_loading.clear();
    model.push_inline_search_content();
    *music_resize = true;
    *tv_resize = true;
}
```

This removes the one-frame lag: previously `terminal_width`/`terminal_height`
were only refreshed inside the next `compute_chrome_geometry` call during
paint, which runs *after* this tick's keyboard/mount-gate decisions.

**Alternative considered:** keep `Resize` payload-free and instead read the
real terminal size via a `TerminalAdapter`/crossterm query at the top of
`apply_terminal_observer`. Rejected: `crossterm::terminal::size()` is
already what produces `Event::WindowResize`'s columns/rows in the
underlying terminal backend, so re-querying it duplicates work the event
already carries and adds a new fallible I/O call to a pure event-mapping
function; forwarding the event's own fields is simpler and matches how
every other `TerminalObserverEvent` variant already carries its payload
(e.g. `Key`, `MouseClick { column, row }`).

### 3. Consumer migration: direct replacement, no shim

Every real (non-test) call site of `is_wide_music_active()`,
`is_wide_tv_active()`, `is_wide_podcast_active()`, or `is_wide_book_active()`
is replaced in place:

- TV-specific decisions that need to know whether *the current tab's* TV
  library is the one holding the wide mount
  (`src/app/input_browse_dispatch.rs:35`, `src/app/shell_browser.rs:196`,
  `src/app/shell_tv_workspace.rs:86,181,230`, `src/app/shell_library.rs:62`,
  `src/app/shell_overlays_menus.rs:114`) replace
  `self.app.layout.main.is_wide_tv_active()` with
  `self.app.wide_tv_library_area(lib_idx).is_some()` for whichever `lib_idx`
  the call site already has in scope (each of these sites already resolves
  a library index for its current tab; none needs a new lookup). This
  keeps the TV content gate (`is_wide_tv_library`) intact, not just the
  breakpoint.
- Music/Book/Podcast decisions that only need "is the right panel wide"
  with no provider-specific content gate
  (`src/app/actions_navigation.rs:205`, `src/app/shell_music_workspace.rs:143`,
  `src/app/shell_messages.rs:32`, `src/app/audiobookshelf_book_modal_actions.rs:57`,
  `src/app/shell_audiobookshelf_book.rs:41`,
  `src/app/shell_audiobookshelf_podcast.rs:49`) replace their
  `is_wide_*_active()` call with `self.is_right_panel_wide()` (or
  `self.app.is_right_panel_wide()` from `Model` methods).
- `src/app/shell_overlays_menus.rs:114`'s context-menu anchor keeps reading
  `layout.main.tv_wide_left_area`/`tv_wide_right_area` for the actual anchor
  rect once the wide branch is chosen (Decision 4) — only the branch
  condition changes from `is_wide_tv_active()` to
  `self.app.wide_tv_library_area(lib_idx).is_some()`.

No compatibility shim or deprecation period: this is an internal-only
change (proposal's "BREAKING (internal)" bullet), so every call site is
updated in the same migration rather than kept on a dual API.

### 4. Paint-produced geometry fields stay, but stop being decision inputs

`tv_wide_right_area`, `wide_music_right_area`,
`audiobookshelf_podcast_right_area`, and
`audiobookshelf_book_wide_right_area` are not deleted — code that already
consumes them for their *rect value* after a wide frame has painted
(hit-testing, context-menu anchors, the embedded list's own viewport math)
keeps doing so; only their use as a *breakpoint decision input* is removed
with the predicates that read them.

The two mirrors that write one of these fields from a component's own
post-paint geometry report are detached:

- `render_audiobookshelf_book_component`
  (`src/app/shell_audiobookshelf_book.rs:195-221`) stops writing
  `self.app.layout.main.audiobookshelf_book_wide_right_area` from
  `geometry.wide`/`geometry.left_area`. The rest of that projection
  (`left_area`, `hero_area`, `selected_item_rect`, `selector_tabs`) is
  unrelated to the breakpoint and is unchanged.
- `render_audiobookshelf_podcast_component`
  (`src/app/shell_audiobookshelf_podcast.rs:249-263`) stops writing
  `self.app.layout.main.audiobookshelf_podcast_right_area` from
  `geometry.right_area`. The rest of that projection is unchanged.

Once `is_wide_book_active()`/`is_wide_podcast_active()` have no remaining
readers (all migrated to `is_right_panel_wide()` per Decision 3), the two
now-orphaned `LayoutMain` fields these mirrors wrote
(`audiobookshelf_book_wide_right_area`, `audiobookshelf_podcast_right_area`)
have no remaining reader either and are deleted along with the predicates,
per the proposal's "single source of truth, no dual API" bullet — they were
never read anywhere except through the mirror-then-predicate pair being
removed. (`tv_wide_right_area` and `wide_music_right_area` are NOT deleted:
they have real geometry readers beyond the deleted predicates, e.g. the
episode/track hit-testing paths in `src/app/render/components/widgets.rs`.)

## Risks / Trade-offs

- **[Risk]** A migrated call site's `lib_idx` doesn't match the library the
  legacy `is_wide_tv_active()` implicitly meant (it read a paint-produced
  field with no library index at all, so it was always "whatever painted
  last," which is usually but not provably the current tab's library) →
  **Mitigation**: each render-test pair added in the completion gate
  (narrow + wide, post-resize tick) exercises the specific call site's
  behavior at both breakpoints; a mismatch shows up as the wrong branch
  being taken in that test.
- **[Risk]** Forgetting a real (non-test) call site during migration leaves
  a dangling reference to a deleted predicate, which the compiler will
  catch (this is a `pub(crate)`-visible method, not a trait object) →
  **Mitigation**: deleting the four predicates last (tasks.md's completion
  gate) forces `cargo check` to enumerate every remaining caller before the
  change can be considered done.
- **[Risk]** Existing tests construct `TerminalObserverEvent::Resize` as a
  unit-like variant (`src/app/shell_tests.rs:99`, `tests_tick_integration.rs`
  callers) and will fail to compile once it carries fields → **Mitigation**:
  tasks.md's Resize-payload task updates those call sites to pass an
  explicit size in the same commit that changes the variant, so the crate
  never sits in a non-compiling state between commits.
- **[Trade-off]** `is_right_panel_wide()` does not take a library index, so
  it cannot express "is *this specific* library's wide mount active" the
  way `wide_tv_library_area` does — call sites that need that distinction
  keep using `wide_tv_library_area`/`is_wide_tv_library` rather than the
  generic predicate. This is intentional (Decision 1's alternative), not a
  gap: the four legacy predicates never expressed that distinction either
  (they read a global `LayoutMain` field, not a per-library one), so no
  call site loses precision it previously had.
