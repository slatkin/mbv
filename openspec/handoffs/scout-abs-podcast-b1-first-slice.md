# Scout: ABS Podcast Phase B — first-slice symbol contract

Scope (as instructed): the raw Audiobookshelf podcast keyboard request boundary only
(`components/audiobookshelf_podcast.rs`, `components/msg.rs`,
`shell_audiobookshelf_podcast.rs`, the ShellRequest match in `shell.rs`, and the
podcast arm of `input_browse_dispatch.rs`). Rendering, cover fetch, async content,
mouse, ABS book, persistence internals, and later state deletion are excluded.

HEAD confirmed: `2c6bcce5 5.3d: correct Audiobookshelf podcast push seams`.

---

## 1. Which podcast keys can stop raw `App::handle_key` forwarding in slice 1

The component already applies every key's local mutation itself
(`audiobookshelf_podcast.rs:96-132`), then unconditionally emits the raw key via
`Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastKey(to_crossterm_key_event(key))))`
(`audiobookshelf_podcast.rs:131-133`). The shell forwards that key to
`App::handle_key` (`shell.rs:723-730` → `handle_audiobookshelf_podcast_key` →
`app.handle_key(key)`).

**Safe subset — show-list cursor movement only (the "show-cursor movement only"
preferred slice):** the eight keys in the `if self.state.episode_selection.is_none()`
arms, i.e. `Up/k`, `Down/j`, `Left/h`, `Right/l`, `PageUp`, `PageDown`, `Home`, `End`
(`audiobookshelf_podcast.rs:105-128`). In this branch the component has already moved
its own cursor via `move_cursor`/`state.select`, so it can emit a typed request
carrying the resolved destination instead of the raw key. These map to the App arm at
`input_browse_dispatch.rs:200-224` (`handle_key_audiobookshelf_library`), which owns
no unique effect beyond moving the mirrored browse cursor.

**Must stay raw in this slice:** the `episode_selection.is_some()` branches — `Up/k`
and `Down/j` episode movement (`move_episode`, `audiobookshelf_podcast.rs:129-130`),
`[`/`]` filter cycling (`:131-132`), `Esc`/`Backspace` episode-selection exit
(`:133-135`), and the non-`is_none()`-guarded `Space`/`Enter`/`Ctrl+a` activation
arms in the App handler (`input_browse_dispatch.rs:219-244`: enter episode selection,
play, open selection modal, enqueue). `_ => {}` fallthrough stays raw
(`audiobookshelf_podcast.rs:136-137`).

Why this subset stops clean: each of the 8 keys moves the *show* cursor only; the App
mutations they mirror are pure navigation plus side effects that can be re-bound to
the typed seam (below). They introduce no ownership decision — the component already
owns the cursor (D14 precedent: `BrowserMoveRows`/`BrowserMoveColumn`/
`BrowserJumpCursor` in `msg.rs`).

## 2. Exact typed ShellRequest variant and payload

Add one variant to `ShellRequest` in `components/msg.rs` (insert next to
`AudiobookshelfPodcastKey` at `msg.rs:292-293`):

```rust
/// The mounted Audiobookshelf podcast browser moved its show-list cursor to an
/// absolute destination it resolved locally (the 8 show movement keys).
/// The shell applies the matching App effect (position save + detail fetch via
/// `App::select_audiobookshelf_show`).
AudiobookshelfPodcastShowMove {
    /// Absolute show index the component's cursor now targets.
    cursor: usize,
},
```

Rationale: the component already knows its resolved target after `move_cursor`
(`state.cursor()` after the move). Passing an absolute `cursor` (rather than a legacy
`rows*columns` delta) avoids re-interpreting the delta with `App`'s column-count
quirk (`move_audiobookshelf_show_rows` multiplies by `library_column_count`,
`audiobookshelf_browse_actions.rs:187-192`), while letting the shell call
`select_audiobookshelf_show` (see §4) which drives position-save + detail exactly.

## 3. Exact component match arms to change

`src/app/components/audiobookshelf_podcast.rs:105-128`, `handle_key`:

| Arm (lines) | Change |
|---|---|
| `Key::Up\|'k' if is_none` (105) | local `move_cursor(-1)` then emit `ShowMove{ ..cursor() }` |
| `Key::Down\|'j' if is_none` (108) | local `move_cursor(1)` then emit `ShowMove{cursor()}` |
| `Key::Left\|'h' if is_none` (111) | local `move_cursor(-1)` then emit |
| `Key::Right\|'l' if is_none` (114) | local `move_cursor(1)` then emit |
| `Key::PageUp if is_none` (117) | `move_cursor(-page_size)` then emit |
| `Key::PageDown if is_none` (120) | `move_cursor(page_size)` then emit |
| `Key::Home if is_none` (123) | `state.select(0)` then emit `cursor()` |
| `Key::End if is_none` (126) | `state.select(len.saturating_sub(1))` then emit |

Concretely: replace the raw trailing emit (`:131-133`) — it becomes an unconditional
`Some(Msg::Shell(AudiobookshelfPodcastKey(..)))` **only** for the non-converted arms.
Each converted arm `return`s the typed `Msg::Shell(AudiobookshelfPodcastShowMove{cursor()})`
and the component's locals unchanged. **Do not** alter the two episode arms
(`:129-130`), filter arms (`:131-132`), or `Esc`/`Backspace` (`:133-135`).

## 5. Raw paths that must remain for a later slice

- `shell.rs:723-724` existing `AudiobookshelfPodcastKey` arm: keep it for the
  non-show-movement keys (episode movement, filter, exit, activation/play/enqueue)
  — this is the raw forwarding path that stays intact for slice 2. The converted
  show-move keys should no longer reach it.
- `handle_key_audiobookshelf_library` arms at `input_browse_dispatch.rs`: episode-cursor
  (`:200-201`), filter (`:212-215`), exit (`:216-217`), space/enter/play/ctrl+a
  (`:219-247`) — unchanged in this slice.
- `shell_audiobookshelf_podcast.rs:8-10` `handle_audiobookshelf_podcast_key` — still
  needed by the surviving `AudiobookshelfPodcastKey` arm; no change in this slice.

## 3 + 6. Files to change (≤3 production) and tests to adapt

Production files (the 3-file ceiling):
1. `src/app/components/msg.rs` — add `AudiobookshelfPodcastShowMove { cursor: usize }`.
2. `src/app/components/audiobookshelf_podcast.rs` — convert the 8 show arms; component
   already computes the target cursor (its inventory of its own move is complete).
3. `src/app/shell.rs` — add match arm next to `:723`:
   ```rust
   Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { cursor }) => {
       self.app.select_audiobookshelf_show(cursor);
       self.push_audiobookshelf_podcast_content();
   }
   ```
   `select_audiobookshelf_show` is `pub(super)` on `App`
   (`audiobookshelf_browse_actions.rs:134-157`), internally does
   `state.select(cursor.min(len-1))` + `save_audiobookshelf_position(index)` +
   `start_audiobookshelf_detail(id)` — exact effects session-side. It leaves
   `episode_selection` cleared via `select` (matching the legacy move path).

Tests to adapt:
- `tests/shell_audiobookshelf_podcast.rs:145-152` `abs_podcast_shell_mounts_and_routes_component`
  asserts `ShellRequest::AudiobookshelfPodcastKey` for a `Key::Down` and then calls
  `handle_audiobookshelf_podcast_key`. This breaks: after conversion, `Down` should
  yield `AudiobookshelfPodcastShowMove { cursor: 1 }`, and the assertion must call
  the new shell arm instead (assert `model.app.audiobookshelf_browse[0].cursor()==1`).
- `tests/components/audiobookshelf_podcast_component_tests.rs:8-37`
  `abs_podcast_component_keeps_local_show_cursor...` presses `Down`, asserts
  `message.is_some()` and `component.cursor()==0`. The cursor truth is unchanged
  (component local move is identical); only the emitted `Msg` variant changes. Should
  either assert the new variant or still pass as `is_some()` (verify).

No other test grep'd for `abs_podcast`/`AudiobookshelfPodcast`.
(`lib_event_actions.rs` and `shell_selection_modal_tests.rs` podcast references are the
selection-modal detail refresh path, out of scope here.)

## 7. Ready implementer prompt

Prompt (3-file ceiling, 35-turn ceiling):

> Implement the smallest Phase B first slice for the Audiobookshelf podcast browser in
> `feat/migrate-tui-to-tuirealm` at `2c6bcce5` — convert only the show/cursor-movement
> keys off the raw key bridge, preserving exact behavior. Use at most these 3 production
> files and finish within ~35 agent turns:
> 1. `src/app/components/msg.rs` — add `ShellRequest::AudiobookshelfPodcastShowMove{cursor: usize}`.
> 2. `src/app/components/audiobookshelf_podcast.rs` — in `handle_key`, for the 8
>    `is_none()` show-move arms, `return Some(Msg::Shell(AudiobookshelfPodcastShowMove{cursor}))`
>    after the existing local motion; leave all episode-selection/filter/activation
>    arms and the final raw `AudiobookshelfPodcastKey` emit intact.
> 3. `src/app/shell.rs` — add an arm beside the existing `AudiobookshelfPodcastKey` arm:
>    `Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove{cursor}) => { self.app.select_audiobookshelf_show(cursor); self.push_audiobookshelf_podcast_content(); }`.
>
> Verification: `rtk cargo check -p mbv` clean; adapt
> `tests_shell_audiobookshelf_podcast.rs::abs_podcast_shell_mounts_and_routes_component`
> and the component `is_some` test as needed; `rtk cargo nextest run` targeted podcast
> tests pass. Do not change `ShellRequest::AudiobookshelfPodcastKey`, the episode-cursor /
> filter / activation keys, or `handle_audiobookshelf_podcast_key`. Do not touch
> rendering, async content, persistence, or mouse.

## Architecture

Component owns cursor (`audiobookshelf_podcast.rs` state + `move_cursor`/`select`);
shell owns App effects (`select_audiobookshelf_show` → position save + detail fetch);
App stays authoritative for playback/enqueue/selection-modal targets
(`input_browse_dispatch.rs:219-247`). The existing D4 `ShellRequest` bridge (`msg.rs`)
is the sole outbound seam; this slice funnels the 8 show-move keys through a typed
message so they stop reaching `App::handle_key`, while the non-move raw arm remains as
the strangler seam for later slices.

## Start here

`src/app/components/audiobookshelf_podcast.rs:96-138` (`handle_key`) — the two-part
change (typed emit on the `is_none()` arms + keep the raw catch-shaped tail) is the
whole behavioral diff; `msg.rs` and `shell.rs` are mechanical. Then verify
`select_audiobookshelf_show` in `audiobookshelf_browse_actions.rs:134-157` is the
correct single effect entry point (it is — it carries `save_audiobookshelf_position` +
`start_audiobookshelf_detail`, both excluded from this probe).

## Notes / residual risk

- **Column-count quirk:** legacy `Down/j` calls `move_audiobookshelf_show_rows(1)` which
  multiplies by `library_column_count`; the component's local `move_cursor(1)` is a
  single-position move. Sending absolute `cursor` (rather than a delta) bypasses the
  quirk and lands App on the show the component painted — a deliberate improvement, but
  it changes the App-side target for multi-column layouts that relied on the old stride.
  This is the intended D4 decoupling (match `BrowserMoveRows` precedent), but flag it to
  the user before shipping.
- `select_audiobookshelf_show` calls `save_audiobookshelf_position` (persistence) and
  `start_audiobookshelf_detail` (async) internally — both out of probe scope; the slender slice
  preserves them by routing through this single existing App method rather than
  re-deriving cursor writes app-side.
- The component's episode arms (`move_episode`, filter, Esc, Space/Enter) keep raw
  forwarding; this slice leaves a partial 2-state mirror (show move decoupled, episode
  move still raw) — acceptable and explicit for the strangler cadence.
- Existing dual-mirror (`component.selected_id` preserved by `set_content`) holds the
  component as render truth; App `audiobookshelf_browse` mirror is now cursor-driven via
  the typed path, so re-projection converges on the same target.

## 5.3d.4 parity resolution — 2026-08-26

A read-only end-to-end probe at accepted HEAD `2c6bcce5` supersedes the
absolute-cursor contract and the ready implementer prompt above.

### Proven production behavior

For show-list movement, the component first mutates its own `selected_id`, then
emits the raw key. The legacy App path runs next. At two columns, Up/Down and
PageUp/PageDown select a row-stride target in App; `select_audiobookshelf_show`
saves that position and starts detail fetching for that target. The shell then
pushes the App snapshot into the component, but `set_content` restores the
component's pre-push `selected_id` when it still exists. The next render therefore
paints the component's local target, not the App effect target.

Thus current behavior is deliberately described at both boundaries:

- painted cursor: component-local `±1`, component `page_size`, first, or last;
- position-save/detail-fetch target: existing App item, row, page-row, first, or
  last operation, including `library_column_count(left_area.width)`.

They agree at one column and diverge for row/page operations at two columns. D17
makes both the current visible behavior and current effect target authoritative.
An absolute `cursor: usize` request would align the effect target to the painted
cursor, which is a behavior change, so it is rejected for this migration.

### Exact typed contract for 5.3d.5

Add the closed payload enum `PodcastShowMove` and emit
`ShellRequest::AudiobookshelfPodcastShowMove(PodcastShowMove)` for only these
show-list keys:

| Key | Payload | Existing shell/App effect entry |
| --- | --- | --- |
| Up / `k` | `PreviousRow` | `move_audiobookshelf_show_rows(-1)` |
| Down / `j` | `NextRow` | `move_audiobookshelf_show_rows(1)` |
| Left / `h` | `PreviousItem` | `move_audiobookshelf_show_cursor(-1)` |
| Right / `l` | `NextItem` | `move_audiobookshelf_show_cursor(1)` |
| PageUp | `PreviousPage` | `move_audiobookshelf_show_rows(-(lib_page_size() as i64))` |
| PageDown | `NextPage` | `move_audiobookshelf_show_rows(lib_page_size() as i64)` |
| Home | `First` | `jump_audiobookshelf_show_cursor(false)` |
| End | `Last` | `jump_audiobookshelf_show_cursor(true)` |

The component keeps its existing local mutations before emitting the typed
operation. The shell runs the exact existing App entry point, then calls
`push_audiobookshelf_podcast_content()`. Episode movement, filter, exit,
activation, play, enqueue, and modal keys continue through
`AudiobookshelfPodcastKey` for later rows.

This remains a three-production-file unit:
`components/msg.rs`, `components/audiobookshelf_podcast.rs`, and `shell.rs`.
Adapt the existing component and shell tests only; do not add a broad behavior
fixture. The shell test should assert the typed operation instead of a raw Down
key and preserve the existing App cursor/effect assertion.

### Evidence anchors

- Local move and raw emit: `src/app/components/audiobookshelf_podcast.rs:86-133`.
- Snapshot restoration: the same file's `set_content`, especially the
  pre-snapshot `selected_id` restore.
- Raw shell route and post-effect push: `src/app/shell.rs`,
  `ShellRequest::AudiobookshelfPodcastKey` arm.
- Legacy operation map: `src/app/input_browse_dispatch.rs`,
  `handle_key_audiobookshelf_library`.
- Effect boundary: `src/app/audiobookshelf_browse_actions.rs`,
  `select_audiobookshelf_show`, `move_audiobookshelf_show_cursor`,
  `move_audiobookshelf_show_rows`, and `jump_audiobookshelf_show_cursor`.
- Row stride: `src/app/library_column_width.rs::library_column_count`.
- Page rows: `src/app/actions.rs::lib_page_size`.

Residual race: `set_content` can restore the component target only while that
show id remains in the snapshot. A concurrent show-list replacement can let the
App snapshot win transiently; this pre-existing refresh behavior is outside the
show-movement unit.

### 5.3d.5 landed

Commit `4eeee915` implemented this exact closed-operation contract in three
production files and adapted the two existing focused tests. Writer evidence:
focused and full nextest (1,152 passed), cargo check, workspace clippy, unchanged
69-finding ast-grep baseline with none in touched files, and fmt all passed. A
fresh Luna review returned `ACCEPT` with no findings. The surviving raw episode,
filter, exit, activation, play, enqueue, and modal endpoint is intentionally the
subject of rows 5.3d.6–5.3d.7.

### Exact typed contract for 5.3d.6

A bounded orchestrator trace at `4eeee915` found no Player, Service, persistence,
or detail effect in episode movement, filter cycling, or episode-selection exit.
The component and App use the same clamped episode movement and exit result, but
filter cycling differs: the component clamps through `ui_util::move_cursor`, while
App wraps with `rem_euclid(3)`. Both results remain current behavior because
`set_content` restores the component-local filter after the App snapshot push.

Use the closed `PodcastEpisodeTransition` variants `PreviousEpisode`,
`NextEpisode`, `PreviousFilter`, `NextFilter`, and `Exit`. The component performs
its existing local transition and emits the matching typed operation. The shell
calls the existing App method (`move_audiobookshelf_episode_cursor(±1)`,
`cycle_audiobookshelf_filter(±1)`, or
`leave_audiobookshelf_episode_selection()`), then pushes podcast content. This
preserves both authoritative boundaries without forwarding a raw key. Enter,
play, enqueue, modal, and unrelated keys remain on `AudiobookshelfPodcastKey` for
5.3d.7. The unit remains the same three production files as 5.3d.5 and may adapt
only the two existing focused component/shell tests.

Commit `0d8a4ef0` landed this contract. Writer evidence: focused podcast and full
nextest (1,154 passed), cargo check, workspace clippy, unchanged 69-finding
ast-grep baseline with none in touched files, and fmt all passed. A fresh Luna
review returned `ACCEPT` with no findings.

### 5.3d.7 landed

Commit `d6f67656` replaced Space/Enter/Ctrl+A with closed typed action intents
and deleted `ShellRequest::AudiobookshelfPodcastKey`. Review found that returning
`None` for unmatched keys dropped global shortcuts because TuiRealm does not
fall through from its focused component. Correction `e7abcb13` routes only those
unmatched keys through the shared `Msg::Legacy` framework bridge; it does not
restore the podcast-specific raw endpoint. Focused/full nextest (1,156 passed),
check, clippy, and fmt passed, and a fresh Luna review accepted the correction.

## Files retrieved (evidence)

1. `src/app/components/audiobookshelf_podcast.rs:9-136` — component `handle_key`
   (local motion + raw `AudiobookshelfPodcastKey` emit), `move_cursor`, `page_size`,
   `move_episode`, `cycle_filter`, `set_content`, `cursor()`.
2. `src/app/components/msg.rs:188-310` — `ShellRequest` numbered the audiobookshelf
   keys at 289-294; `AudiobookshelfPodcastKey` at 292-294; patterns for typed absolute
   targets (`BrowserMoveRows`, `BrowserJumpCursor` etc.).
3. `src/app/shell.rs:715-732` — the ShellRequest match incl. `AudioBookhelfPodcastKey`
   arm at 723-730.
4. `src/app/input_browse_dispatch.rs:176-242` — `handle_key_audiobookshelf_library`
   arm mapping the movement versus activation keys; exact App unit methods.
5. `src/app/audiobookshelf_browse_actions.rs:126-200` — `audiobookshelf_kind_at`;
   `select_audiobookshelf_show:154`-157, `move_audiobookshelf_show_cursor:173-187`,
   `move_audiobookshelf_show_rows:187-192`, `jump_audiobookshelf_show_cursor:194-199`,
   `move_audiobook_episode_cursor:223+`.
6. `src/app/shell_audiobookshelf_podcast.rs:8-10` (`handle_audiobookshelf_podcast_key`)
   and `tests_shell_audiobookshelf_podcast.rs:302-324` — the test that must adapt.
7. `src/app/components/audiobookshelf_podcast_component_tests.rs:8-37` — component test.