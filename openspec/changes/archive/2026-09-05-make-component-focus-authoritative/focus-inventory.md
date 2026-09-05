# Focus inventory (task 1.1)

Working note. Every mounted Interactive Component field / setter arg / render-context
value named or used as `focused`, classified as **framework focus** (a projection of
`PanelFocus` / `Application` focus that must move to `Attribute::Focus`) or **semantic
emphasis** (playback / disabled / selected-but-unfocused identity — unchanged).

## Mounted components that gate behaviour/paint on framework focus

| Component | Focus carrier (before) | Shell writer | Classification | Keyboard/paint use |
|---|---|---|---|---|
| `HomeComponent` (`home.rs`) | `focused: bool` field, `set_focused(bool)` setter | `shell_home.rs` `push_home_content` + `render_home_component` (`!matches!(effective_panel_focus(), Queue)`) | framework | `home.rs:386` `if !self.focused { return None }`; `home.rs:657` paint |
| `BrowserComponent` (`browser/mod.rs`) | `focused: bool` field, `set_content(content, focused)` arg | `shell_browser.rs:334` (`matches!(effective_panel_focus(), Library)`) | framework | `browser/keyboard.rs:48,112` gate; `browser/paint.rs:44,108,128,146,169,205` |
| `FeedsComponent` (`feeds.rs`) | `focused: bool` field, `set_content(..., focused)` arg | `shell_feeds.rs:17` (`Library`) | framework | `feeds.rs:264` gate; `feeds.rs:447` paint |
| `AudiobookshelfBookComponent` (`audiobookshelf_book.rs`) | `focused: bool` field, `set_content(snapshot, focused, images)` arg | `shell_audiobookshelf_book.rs:156` (`Library`) | framework | `audiobookshelf_book.rs:220` gate; `:451` paint. NOTE `chapters_visible` / `chapter_selection` / `chapters_focused` are **semantic pane state** (retained local), not framework focus |
| `AudiobookshelfPodcastComponent` (`audiobookshelf_podcast.rs`) | `focused: bool` field, `set_content(snapshot, focused, images)` arg | `shell_audiobookshelf_podcast.rs:176` (`Library`) | framework | `audiobookshelf_podcast.rs:211` gate; `:439` paint |
| `MusicWorkspaceComponent` (`music_workspace.rs`) | `context.focused` (field of `MusicWideRenderCtx`), `MusicWideRenderCtx::new(.., focused, ..)` | `music_wide.rs:310` `wide_music_render_ctx` (`matches!(effective_panel_focus(), Library)`), pushed via `shell_music_workspace.rs:156` `set_content(context)` | framework | `music_workspace_keys.rs:37,69,109,113,122,126` gates; `music_workspace.rs:344` `can_enter_track_focus`; `:586` paint. `track_cursor: Option<usize>` is **retained local pane state** (album vs track), combine with framework focus at view/event time |
| `TvWorkspaceComponent` (`tv_workspace/mod.rs`) | `context.focused` (field of `TvWideRenderCtx`), `TvWideRenderCtx::new(.., focused, ..)` | `shell_tv_workspace.rs:344` (`Library`); helper `tv_wide.rs:201` `wide_tv_render_ctx(idx, focused, _)` also ANDs `Library` | framework | `tv_workspace/keyboard.rs:28` gate; `tv_wide.rs:226,227,231,242,290,302,321,335` paint. `pane: Pane` (Series/Episodes) + `season_cursor`/`episode_cursor` derived local state — combine with framework focus. `episode_cursor`/`season_cursor` on the ctx are navigation projection, NOT framework focus |
| `QueueComponent` (`queue.rs`) | `focused: bool` field, `set_content(slots, cursor, scope, focused, playback, title)` arg | `shell_queue.rs:52` (`matches!(effective_panel_focus(), Queue) && !blocking_overlay_active()`) — same file also drives `Application::active`/`blur` for `ComponentId::Queue` at `:25-31` | framework | `queue.rs:439` paint. Queue keyboard is not focus-gated in the component (router owns it) |

## Semantic booleans — NOT framework focus, unchanged

- `PlaybackComponent` `focused: bool` (`playback.rs:30`) — set by `shell_playback.rs:43` from
  `matches!(effective_panel_focus(), Queue)`; this is a *paint-emphasis* input for the
  transport chrome, and `PlaybackComponent` already forwards `attr` to `self.props`.
  Playback is not in the change's stated scope list; leave as-is unless review says otherwise.
- `MediaSemanticState` (row-level: playback emphasis / watched dimming) in every media list — semantic.
- `feeds_manage.rs` `form.focus: FeedFormField` — intra-form field cursor, not framework focus.
- `WatchedFilter`, `chapters_focused`, `chapters_visible`, Music `track_cursor`,
  TV `pane` — component-private pane/selection state that must survive blur.

## Shell focus mirrors / focus-only projection seams

- `shell_home.rs` `render_home_component` re-pushes `set_focused` every frame (focus-only) — delete.
- `shell_home.rs` `push_home_content` passes `focused` — remove arg.
- `shell_browser.rs` / `shell_feeds.rs` / `shell_audiobookshelf_book.rs` /
  `shell_audiobookshelf_podcast.rs` compute `let focused = matches!(effective_panel_focus(), Library)`
  purely to feed `set_content` — remove.
- `shell_queue.rs` computes `queue_focused` for BOTH the `Application::active`/`blur` call
  (keep) and the `set_content` arg (remove).
- `shell_music_workspace.rs` / `shell_tv_workspace.rs` build the render ctx with a `focused`
  positional — remove; ctx `focused` field stays but is component-owned, applied from `attr`.

## `Application::active` / `blur` already driven by the shell

- `shell_library.rs` `sync_active_destination` → `application.active(library_child_id)` /
  falls back to `UiRoot`; short-circuits when Queue owns focus.
- `shell_queue.rs` `sync_queue` → `application.active(Queue)` / `application.blur()`.
- Overlays: `shell_overlays_*`, `shell_root.rs`, `shell_feeds_manage.rs` all call `active`.

So `Attribute::Focus(Flag(_))` is already emitted to mounted components on every Panel-focus
transition; the components just discard it today (`fn attr(..) {}`).
