## Why

Issue #626 is the deletion #607's acceptance criterion has been waiting on.
`split-browse-state-interaction-fields` (#621) split content from interaction state in
all three browse structs and re-pointed every reader, but stopped at task 4.4:
`BrowseLevel::cursor` and `BrowseLevel::scroll` are still there. The last
reader, `library_list_render_ctx` (R14), could not be re-pointed because two
narrow surfaces had no mounted component to read a live cursor from.
`migrate-narrow-browse-to-components` fixes that and lands the threading.

This change is the deletion the whole line of work exists to perform, plus the
code it makes unreachable. Its design rationale lives in
`split-browse-state-interaction-fields/design.md` (D1–D5 and the task 1
inventory); it is not restated here.

## What Changes

- Delete `BrowseLevel::cursor` and `BrowseLevel::scroll`.
- Retire what that makes unreachable: `App::apply_lib_cursor_index`, the
  `move_lib_cursor*` / `jump_lib_cursor` delta movers, and the
  `mouse_gestures.rs` call sites that were their only remaining callers.
- Remove the `rules/interactive-component-boundary/` clauses that policed by
  convention what the types now make unrepresentable, and the warning comments
  that documented the old rule.

## Non-goals

- No repair of mouse routing (D16). Where `mouse_gestures.rs` is the sole
  caller of something being deleted, delete the caller with it.
- No behavioural change to browsing, playback, persistence, or restore.
- No doc, ledger, or ADR updates — those are `sync-interactive-surface-docs`.

## Capabilities

No spec change. The framework requirements this work satisfies were added by
`split-browse-state-interaction-fields` and
`migrate-narrow-browse-to-components`; this change makes the code match them.

## Impact

`src/app/types_browse.rs`, `src/app/lib_cursor_actions.rs`,
`src/app/mouse_gestures.rs`, `src/app/actions_navigation.rs`,
`src/app/context_menu_actions.rs`, `src/app/shell_browser.rs`,
`src/app/shell_tv_workspace.rs`, `src/app/shuffle_folder_actions.rs`,
`src/app/render/components/music_wide.rs`,
`src/app/render/components/widgets.rs`,
`src/app/render/components/detail.rs`,
`src/app/render/components/home_video.rs`,
`src/app/render/components/tv_wide.rs`,
`src/app/render/components/list_narrow.rs`,
`rules/interactive-component-boundary/`, and the readers named in
`split-browse-state-interaction-fields/design.md` §1.1/1.1b.

## Sequencing

Depends on `migrate-narrow-browse-to-components` — specifically its **task 2**
(ownership), not the full painter hoist. Blocks
`sync-interactive-surface-docs`.

The Music wide re-point (design §1.1b R16–R18) was scoped to
`split-browse-state-interaction-fields` task 4.4 / `remove-music-workspace-cursor-mirror`
and was left live when those changes closed; it lands here as task 1.1c.

`migrate-narrow-browse-to-components` mounted components at every Emby
breakpoint but left several live per-frame readers of the raw fields on the
legacy render/shell paths — the narrow inline-hero resolvers (§1.1b R19/R20),
the wide-TV ctx `None` fallback, the poster-prefetch scroll read, the
breakpoint hand-off cursor write, and the content-projection seams — plus
three now-orphaned dead readers (`shuffle_play`, `render_compact_detail`,
`render_selected_home_video_detail`). These land as tasks 1.1d–1.1g and the
outcome-2 re-spellings folded into 1.2a.

The deletion itself is two commits: **1.2a** lands the type change plus every
production consequence (`cargo check -p mbv` clean); **1.2b** is the mechanical
test-side migration — `BrowseLevel`'s fields are `pub(super)` and the test corpus
is a sibling module, so 1.2a necessarily lands with `cargo check --tests` red
until 1.2b closes it. Rows 2.1-2.3 then delete the mover and mouse paths that
1.2a only keeps compiling.
