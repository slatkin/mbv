# Design

## Context

See `proposal.md` for the motivation and `specs/gui-client/spec.md` for the behavior.

Today `QueuePlaybackPanel` (`crates/mbv-gui/src/window/components/queue_playback_panel.rs`) paints a flex column: `HeaderRow`, `StillSlot` (the art box, only when `playback.artwork` is `Some`), the title block, and, behind `playback.show_controls`, `Scrubber`, `TransportRow`, and `TrackChips`. Constraints that shape the approach:

- The window holds no interaction state. Presses go to the core, which resolves them against the hit map that the window reports (`LayoutReporter`). Inside one report key, the last region pushed wins. Today the panel pushes `VisualSlot` after the transport buttons.
- `TrackChips` and `Scrubber` are child components. `TrackChips` reports under its own `LayoutKey`s, created after the panel's key, so its regions already land after the panel's.
- `StillSlot` has `overflow-hidden`, which would clip a track-chip popover that hangs below the art.
- Blitz (`blitz-dom` 0.3.0-beta.4, `stylo` 0.22.0) applies `:hover` to the whole ancestor chain of the hit node (`Document::set_hover_to`). It does **not** support container queries: `query_container_size` returns a default size ("FIXME: Implement container queries"), so `cq*` units do not resolve.
- `GuiArtRef` has `Image`, `Placeholder`, and `Loading`; `None` means the item has no art. The collapsed rail and the compact strip also paint `Placeholder`, so the variant stays.
- `still_caption` is read only by the expanded panel's placeholder.
- The prefs file (`App::save_prefs`, `src/app/input.rs`) already persists `visual_slot_hidden`. `construct.rs` reads it back.

## Goals / Non-Goals

**Goals:**
- One structure for the art box that the future inline video (#989) can take over without change to the overlays.
- No new window-held interaction state: hover is the stylesheet's, auto-hide is the core's.

**Non-Goals:**
- The collapsed rail and the compact strip do not change.
- The TUI Client does not change.
- No video rendering.

## Decisions

### D1. Layer structure: a stage with sibling overlays

The art box becomes a **stage**: a `relative` wrapper with no `overflow-hidden`. It holds three siblings:

```
stage  (relative, data-target=visualSlot, the VisualSlot region)
 +-- media      (overflow-hidden: the image; later the video)
 +-- top        (absolute, top: track chips left, badge right; always visible)
 +-- bottom     (absolute, bottom: gradient + two control rows; auto-hide applies)
```

The media box owns the clip, so the overlays and the chip popovers are never clipped. A popover paints above the queue rows with a z-index over the panel. For #989, only `media` changes.

*Alternative:* overlays as children of the clipped box. Rejected: the audio and subtitle popovers would be cut.

### D2. Hover through the stylesheet

The stage carries `data-auto-hide` (from the frame) and `data-playing` (from `playback.status == Playing`). One rule set in `assets/app.css`:

```
.stage[data-auto-hide][data-playing] > .bottom         { opacity: 0 }
.stage[data-auto-hide][data-playing]:hover > .bottom   { opacity: 1 }
```

Each attribute is on the stage only while its fact holds. It is absent otherwise, never `"false"`, because the selectors test presence. Paused is "not playing", so the controls show. The bottom layer stays laid out while hidden, so its regions still report. A press can reach a hidden control only with the pointer over the stage, and then the controls show. The window holds no hover state.

*Alternative:* a core-owned hover fact in the frame, as `GuiQueue.hover`. Rejected: more frame traffic, and nothing in the core needs it.

### D3. Scale from the measured art height

Container units do not resolve in Blitz (see Context), so the window measures. The panel's existing effect already reads the stage's rect through `get_client_rect` for the `VisualSlot` region. From that height it computes

```
scale = min(1.0, (stage_height / 3.0) / BOTTOM_FULL_HEIGHT_PX)
```

and holds it in a window `Signal<f32>`. This is layout, not interaction state (spec: "Layout and the predicted playhead are not interaction state"). The bottom layer sets `--player-scale` from it, and its sizes (button boxes, glyph sizes, font sizes, gaps, the scrubber's block height) are `calc(<px> * var(--player-scale))`. The stage height depends only on the panel width and the image aspect, never on the overlay, so there is no feedback loop. No floor is applied: the one-third limit holds at every height.

The scrubber's pointer math reads its own measured rect, so it stays correct at any scale.

*Alternative:* `transform: scale()`. Rejected: the reported client rects of transformed children are not reliable in Blitz, and the hit map needs real rects.

### D4. Region order

Inside the panel's report key, the `VisualSlot` region is pushed **first**, then play/pause, previous, seek back, seek forward, next, seekbar, auto-hide, collapse, and playback target. The chips already report under later keys.

### D5. The auto-hide preference lives in the core

- `App` gains `gui_player_auto_hide: bool`, read in `construct.rs` from the prefs key `gui_player_auto_hide` (default `false`) and written in `save_prefs`.
- `mbv-ui-msg` `PlaybackRequest` gains `ToggleAutoHide`. The shell arm in `src/app/shell/playback.rs` flips the field and calls `save_prefs`. Every match on `PlaybackRequest` gets an explicit arm.
- `QueuePlaybackTargetId` gains `AutoHide`; `leaves.rs` maps a press on it to `PlaybackRequest::ToggleAutoHide`.
- The section build (`crates/mbv-gui/src/sections.rs`, beside `artwork`) sets `GuiQueuePlayback.auto_hide` from the App field. The GUI-only fact stays out of the shared `QueuePlaybackProjection`.
- No key chord: the requirement that demanded one is removed in this change.

### D6. Image states

| `playback.artwork` | Panel |
| --- | --- |
| `Image` | stage with the image |
| `Loading` | stage with an empty media box at the panel width, `aspect-ratio: 16 / 9`, and both overlays |
| `Placeholder` or `None` | no stage; the no-image column |

The no-image column paints, under the header: the chips-and-badge line, the two control rows at scale 1.0 (always visible, no auto-hide rule applies), then the title block. The auto-hide control still paints and reports its state.

`still_caption` leaves `QueuePlaybackProjection`, `GuiQueuePlayback`, and their tests, because nothing else reads it. `NaturalSizeBounds` use in the panel goes with the placeholder box.

### D7. The stopped state

In the stopped state the frame carries `now_playing_title: None`, `artwork: None`, `show_controls: false`, and `status: Idle` (`now_playing_title_parts` returns `None` while inactive; `queue_card_artwork` returns `Hidden` while idle). So the gate cannot be the title.

The section build adds `GuiQueuePlayback.can_start: bool` from `App::shown_queue_play_request().is_some()` (`src/app/state/named_queues.rs`) and a public shown-queue slot-count read added beside it. That request is exactly the case where play/pause starts the shown queue (`LocalPlaybackTarget::toggle_play_pause`); it is also `Some` for an empty new queue, so the slot count is part of the gate. `can_start` is true only for a shown local queue that holds slots while nothing plays. A shown peer queue and an empty shown local queue give `false`, because play/pause cannot start them.

The two control rows paint when `can_start || now_playing_title.is_some()`. In the stopped case (`can_start` true, title `None`) the panel paints, under `HeaderRow`, only the two control rows at scale 1.0: no stage, no chips-and-badge line, no title. The scrubber paints its empty track (the core resolves no seek, because `seek_to_fraction` does nothing at `runtime_ticks == 0`). No auto-hide attribute applies, because there is no stage.

### D8. Module split

`queue_playback_panel.rs` is 700 lines. The overlay layers move to a child module `queue_playback_panel/art_stage.rs` (stage, top layer, bottom layer, scale), and `TransportRow` is reshaped there into the two rows. The file stays under 800 lines.

## Risks / Trade-offs

- [Blitz hover edge: the pointer leaves the window while over the stage] → `clear_hover` unhovers the chain, so the controls hide as expected.
- [Very short images make tiny controls] → Accepted. The user's rule is one third at most. Panoramic art is rare.
- [The 16:9 loading box differs from the final image aspect] → The box resizes when the image loads. Accepted by the user ("act like we expect the image to load").
- [Opacity-hidden controls still take presses] → Only reachable while hovered, when they show (D2).

## Migration Plan

None. A missing `gui_player_auto_hide` key reads as `false`, which is the old always-visible look.
