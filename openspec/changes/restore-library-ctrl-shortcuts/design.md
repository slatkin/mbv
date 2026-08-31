# Design

## D1. One shell handler, `Library*` variants, not `Music*`/`Tv*`

`BrowserComponent`'s five effect requests (`BrowserPlay`, `BrowserEnqueue`,
`BrowserToggleWatched`, `BrowserShuffle`, `BrowserRescan` + bare-`r`
`BrowserRefresh`) all route through `Model::handle_browser_request`
(`src/app/shell_browser.rs:20`), which:

1. derives `lib_idx` from `self.app.tab.emby_library_index()` (defensive
   no-op if absent),
2. calls a Service-agnostic `App` tail with `(lib_idx, item)`:

| Request | Tail | Signature |
|---|---|---|
| `BrowserPlay` | `play_or_activate_lib_item` | `(usize, EmbyItem)` |
| `BrowserEnqueue` | `enqueue_lib_item` | `(usize, EmbyItem)` |
| `BrowserToggleWatched` | `toggle_watched_item` | `(usize, EmbyItem)` |
| `BrowserShuffle` | `shuffle_play_selected` | `(usize, EmbyItem)` |
| `BrowserRefresh` | `refresh_lib` | `(usize)` |
| `BrowserRescan` | `ask_confirm(RescanLibrary(lib_idx))` | — |

None of these tails is video-specific. The Music and TV panels are also Emby
libraries selected in the same left panel, so `emby_library_index()` resolves
them identically. Therefore the migration needs **no new effect code** — only
a way for the Music and TV components to reach the existing tails.

Decision: add `LibraryPlay { item }`, `LibraryEnqueue { item }`,
`LibraryToggleWatched { item }`, `LibraryShuffle { item }`, `LibraryRefresh`,
`LibraryRescan` — one set, emitted by both `MusicWorkspaceComponent` and
`TvWorkspaceComponent` — and route them through the existing
`handle_browser_request` body (rename to `handle_emby_library_request`, or
add a thin `handle_library_request` that shares the match). Rejected:
per-surface `Music*` / `Tv*` variants, which would triple the variant count
and the dispatch arms for zero behavioral difference.

> Naming note: `CONTEXT.md` — confirm `Library*` does not collide with an
> existing `LibraryList*` / `LibraryRow*` term before finalizing; fall back
> to `EmbyLibrary*` if it does.

## D2. Target resolution already exists on both components

- `MusicWorkspaceComponent::selected_item() -> Option<EmbyItem>`
  (`music_workspace.rs:142`) resolves the highlighted album from
  `album_order` + the component cursor.
- `TvWorkspaceComponent::selected_item() -> Option<EmbyItem>`
  (`tv_workspace.rs:141`) resolves the highlighted series/episode.

Each new key arm does `self.selected_item().map(|item| ShellRequest::Library…
{ item })` — the exact shape of `browser.rs:373-393`. An empty list yields
`None` and the chord stays unclaimed, matching the browser.

## D3. Key-arm placement and precedence

### Music (`MusicWorkspaceComponent::handle_key`)

The early return is `if !self.context.focused { return None; }`. Existing
track-level arms (`music_workspace.rs:231-297`) must keep winning when
`track_cursor.is_some()`. Add the album-level arms guarded on
`track_cursor.is_none()`, before the `[`/`]` group-pill arms:

| Chord | Guard | Emits |
|---|---|---|
| `Ctrl+P` | `track_cursor.is_none()` | `LibraryPlay { item }` (legacy: non-folder album → `select` == activate; `play_or_activate_lib_item` covers both) |
| `Ctrl+A` | `track_cursor.is_none()` | `LibraryEnqueue { item }` |
| `Ctrl+W` | `track_cursor.is_none()` | `LibraryToggleWatched { item }` |
| `Ctrl+S` | `track_cursor.is_none()` | `LibraryShuffle { item }` |
| `Ctrl+R` | `track_cursor.is_none()` | `LibraryRescan` |
| bare `r` (no CONTROL/ALT) | `track_cursor.is_none()` | `LibraryRefresh` |

`Ctrl+R` arm comes before bare `r`, matching legacy precedence
(`input_lib_keys.rs:282` before `:291`).

### TV (`TvWorkspaceComponent::handle_key`)

The `_ => None` at `tv_workspace.rs:393` is the drop site. Add the same five
chords + bare `r`. The Episodes-pane arms (`tv_workspace.rs:319-346`) already
claim `[`/`]`/`j`/`k` when `pane == Pane::Episodes`; the Ctrl chords act on
`selected_item()` regardless of pane (legacy `handle_lib_key` did not gate
them on drill level), so place them after the pane-specific arms and before
the letter-pill arm.

Open question for the implementer: does legacy `Ctrl+W` on a TV series toggle
the whole series watched, or the highlighted episode when the Episodes pane
is focused? Check `toggle_watched` in `~/Dev/mbv/src/app/input_lib_keys.rs`
and mirror it via which `EmbyItem` `selected_item()` returns per pane.

## D4. No router change

`view_dispatch` (`key_policy.rs:369`, owner `Active(None)`) already forwards
every unclaimed key to the active component's `on`. The Ctrl chords are not
matched by any earlier policy layer:

- `playback` layer (`key_policy.rs:306`) consults
  `playback_command_for_key` (`action.rs:136-155`), whose only Ctrl-relevant
  arms are `Char('a') if gated && !ctrl` and `Char('z') if !ctrl` — both
  *exclude* CONTROL. Lowercase `p`/`s`/`w`/`r` + CONTROL fall to `_ => None`.
- `ctrl_l_force_clear` matches only `Ctrl+L`.
- `alt_swallow` matches only ALT.

So the chords reach the component untouched today; they are dropped solely by
the components' own `_ => None`.

## D5. Verification

Component-level tests (no App, per `feedback_no_e2e_unit_tests` — component
`on` returning the right `Msg`, not a full flow):

1. `music_workspace_ctrl_s_on_album_emits_library_shuffle` — album focused,
   no track focus → `Msg::Shell(ShellRequest::LibraryShuffle { item })` with
   the highlighted album's id.
2. `music_workspace_ctrl_s_with_track_focus_does_not_shuffle` — track
   focused → the chord is not claimed as a library shuffle (track context
   wins / stays unclaimed).
3. `music_workspace_ctrl_p_empty_list_is_unclaimed` → `None`.
4. `tv_workspace_ctrl_r_emits_library_rescan`.
5. `tv_workspace_ctrl_w_emits_library_toggle_watched` with the pane-correct
   `EmbyItem`.

Shell-level (Model, one per new variant) mirroring
`shell_browser_tests.rs:81-211`: drive the key through the mounted music/TV
component, assert the `App` side ran (queue grew, confirm modal raised, etc.).

Gates: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
`rtk cargo clippy --workspace --all-targets`, `rtk cargo fmt`,
`rtk make check-code-file-lines` (watch `shell_browser.rs`),
`rtk ast-grep scan` (the `no-raw-fallback-variants` rule must stay green —
no `GlobalViewKey`).
