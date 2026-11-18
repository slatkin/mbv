# Proposal

## Why

The GUI Queue panel stacks the album art, the title, the progress bar, a centred transport row, and the track chips one under the other. This uses a lot of height and horizontal space. If the player controls go over the art, the panel is more compact. This is also the first step of issue #989: the art box becomes the future place of the inline video. The video then gets the same overlays that a video player usually has.

## What Changes

- The player controls move onto a bottom overlay over the album art in the expanded Queue panel. The bottom overlay has two rows:
  - Row 1: play/pause, the progress bar (with its chapter ticks, intro and credits marks, and hover preview), and the elapsed and remaining clocks.
  - Row 2: previous, back 10 s, forward 30 s, next, and the auto-hide tickbox at the right end.
- Play/pause is a normal-size button at the left of the progress bar. It is no longer the large centred control.
- The bottom overlay takes no more than the bottom 1/3 of the image. Its controls scale down with the image height to stay inside that limit.
- The track chips (video label, audio, subtitles) move to a top-left overlay on the image. They always show. Auto-hide does not apply to them.
- The season and episode badge moves to the top right of the image.
- The title and the parent line stay under the image, outside the overlays.
- The bottom overlay shows at all times by default. The auto-hide tickbox (a small box with a tooltip and no text label) lets the user turn on auto-hide. With auto-hide on, the bottom overlay hides while an item plays and the pointer is not over the image. It shows while the pointer is over the image, while the playback is paused, and while the player is stopped.
- The auto-hide choice persists across launches.
- While the image loads, the panel keeps an image box of the expected size and paints the overlays on it, as if the image were there.
- When the item has no image, no image box paints. Under the panel header, one line holds the track chips at the left and the badge at the right, then the two control rows, then the title line. Auto-hide does nothing in that case, but the tickbox still shows and keeps its setting.
- In the stopped state (an item is in the queue but nothing plays), the bottom overlay shows the buttons. The progress bar paints an empty track and the track chips paint nothing. A press on play/pause starts the shown queue, as it does today.
- **Removed:** the placeholder art of the expanded panel (the fixed-height box with the bundled placeholder image and the `[source]` caption). The collapsed rail and the compact strip keep their own art paint.

Assumption recorded: "stopped" counts as "not playing", so auto-hide keeps the bottom overlay on screen in the stopped state, the same as while paused.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the expanded player panel's arrangement and the artwork scenarios change; new requirements cover the art overlays, auto-hide, the no-image arrangement, and the stopped state. The paint change itself is an intentional look change and needs no delta to the "looks as the Svelte front end looked" requirement. The requirement "Every action is reachable from the keyboard" is removed, because later changes superseded it.

## Impact

- `crates/mbv-gui/src/window/components/queue_playback_panel.rs` and its `scrubber` and `track_chips` children: new arrangement, overlay layers, scale factor, and region order.
- `crates/mbv-gui/assets/app.css`: the overlay, gradient, and auto-hide hover rules.
- `crates/mbv-gui/src/bridge/layout.rs`, `crates/mbv-gui/src/leaves.rs`: a new pointer target for the tickbox.
- `crates/mbv-ui-msg/src/playback.rs`, `src/app/shell/playback.rs`: a new playback request that toggles auto-hide.
- `src/app/state/app_struct.rs`, `src/app/state/construct.rs`, `src/app/input.rs`: the persisted auto-hide preference in the prefs file.
- `crates/mbv-gui/src/sections.rs`, `crates/mbv-gui/src/bridge/frame/playback.rs`: the frame carries the auto-hide fact; the still caption leaves the frame.
- `src/app/queue_playback_projection.rs`: the `still_caption` field is removed (only the expanded panel's placeholder used it).
- No new dependencies. The TUI Client does not change.
