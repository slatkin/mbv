## Context

See proposal.md - Why/What Changes for motivation and scope.

Today, `wide_row.rs::wide_media_row` (called from `render_wide_media_list` and `render_inline_media_browser_with_geometry` in `src/app/render/components/media_list/wide.rs`, and from `render/components/queue.rs`) truncates every row's `primary` text with `ui_util::trunc_str` into a fixed-width slot, with no distinction for the selected row. Ownership of per-list interaction state (`cursor`, `scroll`) lives on `MediaList<Target>` in `src/app/components/media_list/mod.rs`; `WideMediaList<Target>` and `InlineMediaBrowser<Target>` each wrap a `core: MediaList<Target>` and configure presentation-specific geometry/policy around it.

A marquee primitive already exists for the player strip's Now Playing / idle-feed title: `chrome_player.rs::marquee_spans` + `marquee_col` + `colored_width_window`, driven by a shared `App`-owned clock (`app_struct.rs::marquee_text`/`marquee_started_at`) because those two callers show one conceptual text and must animate in lockstep. That sharing model does not fit a list row: each embedded list's selection is independent, and multiple lists can be visible/mounted at once. `queue_playback_panel.rs`/`library_playback_panel.rs` already establish the alternative pattern used elsewhere in this codebase — a panel-local (not shell-global) `marquee_text`/`marquee_started_at` pair.

**Naming caveat:** an unrelated, already-planned rename lands `wide_media_row` → `media_list_row` before this change is implemented. This design refers to "the shared row painter" and gives the current name only for locating it; the implementer should use whatever name it carries at the time (see design's Risks section).

The shell's tick loop polls at a fixed ~50ms regardless of playback/idle state (`shell_run.rs`), which is why the existing player-strip marquees already animate with no user input. No new redraw plumbing is needed for a row marquee.

## Goals / Non-Goals

**Goals:**
- One marquee implementation shared by the player-strip title and the selected-row title, not two.
- Marquee state that lives with the list it animates (`MediaList<Target>`), independent across lists, surviving a Wide↔Narrow presentation swap the same way cursor/scroll already do.
- Zero behavior change for every row except the selected+focused, overflowing one.

**Non-Goals:**
- Legacy grid/letter-group painters (`plain_rows.rs`, `list_rows.rs`, `list_letter_groups.rs`) are untouched; their existing "showcase the full title below" treatment for overflow is a separate, already-shipped mechanism and out of scope.
- No change to the marquee's timing/feel — this reuses the existing cadence exactly, it does not tune or make it configurable.
- No change to which row is highlighted/selected, only to how its title renders when it overflows.

## Decisions

**Extract the marquee primitive out of `chrome_player.rs`.** `marquee_col` (pure, already unit-tested) and the width-windowing function move to a shared location (a small new file alongside the other `render/components` primitives, e.g. `marquee.rs`) generalized to take `&mut String` + `&mut Instant` directly instead of `chrome_player`'s `PlaybackRenderContext`. `chrome_player.rs` becomes a caller passing its own two fields; the row painter becomes a second caller passing `MediaList<Target>`'s fields. Alternative considered: duplicate the ~30 lines into the row painter. Rejected — two independently-tuned marquee implementations is exactly the kind of divergence this codebase's conventions call out as a flaw once they inevitably drift (differing cadence between the player title and a row title would look like a bug, not two features).

**Marquee clock lives on `MediaList<Target>`, not `WideMediaList`/`InlineMediaBrowser` individually.** Both presentation wrappers hold `core: MediaList<Target>` and can convert into one another (`into_media_list`/`from_media_list`) without losing cursor/scroll; the marquee clock is the same kind of row-flow-local state and belongs in the same place so a Wide↔Narrow transition doesn't reset or duplicate it. Alternative considered: put it on each presentation wrapper. Rejected — would need to be copied across `from_media_list`/`into_media_list`, duplicating a concern `MediaList` already owns for cursor/scroll.

**Reset trigger is content-based, not event-based.** Rather than hooking every cursor-mutating method (`move_selection`, `select_first`, `select_last`, `set_content`) to reset the clock, the row painter compares the marqueed row's current primary text against the last-seen text (stored alongside the clock) each frame and resets on mismatch — exactly the trick `chrome_player.rs::marquee_spans` already uses for the same reason (a new title should never open mid-scroll, regardless of *why* it changed). Alternative considered: reset explicitly in `move_selection`/`select_first`/`select_last`. Rejected — misses `set_content` swapping the item under an unmoved cursor, and duplicates a comparison this codebase already has a working pattern for.

**Gating is `selected && focused`, matching the existing highlight condition.** `wide_row.rs` already computes `let selected = selected && focused;` before deciding the row's highlight color/background — an unfocused list's cursor row gets no visual distinction today. The marquee reuses that same computed condition rather than introducing a second one, so "is this row visually called out" and "does this row marquee" never disagree.

**`InlineMediaBrowser`'s row-paint entry point becomes `&mut`.** `render_inline_media_browser_with_geometry` currently takes `list: &InlineMediaBrowser<Target>` (no mutation needed today — `finish_view` handles all state writes after painting). Advancing the marquee clock during paint needs `&mut`; its caller (`render_inline_media_browser_component`) already holds `&mut InlineMediaBrowser<Target>`, so this is a signature-only ripple, not a new ownership path.

## Risks / Trade-offs

- **Rename lands mid-flight.** If implementation starts before the planned `wide_media_row` → `media_list_row` rename merges, the function/file will still carry the old name. → Not a blocker: apply this change against whatever name is current at the time; there is no behavior difference to reconcile, only a name.
- **Single-color assumption.** The row title is always one color per state (unlike the player strip's title, which can carry multiple differently-colored parts). → The shared primitive's windowing function is used with a one-element `(text, color)` slice for rows; no need to carry `chrome_player`'s multi-part case into the extraction, it already supports a single part today.
- **`&mut` ripple on the Inline path.** Changing `render_inline_media_browser_with_geometry`'s `list` parameter to `&mut` touches one call site and one signature; low risk, called out here so it isn't a surprise mid-implementation.

## Migration Plan

Single self-contained change, no data migration, no feature flag: the new behavior only activates for a row that is simultaneously selected, focused, and overflowing, so there is no visible change until a long title is actually selected. Land and merge directly.
