# Defect: task 4.10 reimplemented the playback panel instead of migrating it

Found by user report after 4.10 landed (`5248e99`): the right-panel playback
view renders wrong; the mini view is fine.

## What happened

The real renderer is `render_player_panel` in
`src/app/render/components/chrome_player.rs` (559 lines) — marquee scrolling for
overflowing titles (`marquee_spans`, `marquee_col`), `colored_width_window`,
uppercase span handling, and a separate `render_title_row`.

4.10 did not lift it. It wrote a new 130-line `render_playback_chrome_content`
(`src/app/render/components/playback.rs`) painting a seekbar and title area from
scratch, and pointed `PlaybackComponent` at that.

## Why only the right panel is visibly wrong

All three legacy call sites still paint underneath (correct mirror-first
behaviour):

    root.rs:342  render_player_panel(.. player_area .. playback_panel_bg)
    root.rs:389  render_player_panel(.. panel_area  .. SURFACE_CHROME)
    root.rs:405  render_player_panel(.. panel_area  .. SURFACE_CHROME)

`PlaybackComponent` paints over `projection.player_area` only — the first site.
The card and mini variants are uncovered, so they still show the real renderer.

## The tell

`render_player_panel` takes seven arguments (`player_h`, `show_controls`,
`now_playing_title`, and a **per-call-site** background). `PlaybackProjection`
carries `player_area` + `focused`. A faithful extraction cannot lose inputs —
the compiler forces every one into the new signature. A fresh reimplementation
drops them silently. The 130-vs-559 line ratio was the number to notice.

Contributing cause: the task text and its agent prompt said "reduced
playback-status projection only", which licensed a lookalike for a surface with
this much painting logic.

## Fix

Do for playback what 4.2 did for TV: lift `render_player_panel` out of
`impl App` into a free function taking an explicit context whose fields are
those seven parameters, point `PlaybackComponent` at it, and delete
`render_playback_chrome_content`.

Wrinkle: `marquee_spans` is `&mut self` and time-dependent (`elapsed_ms`), so
the marquee cursor is genuine component-local state — own it in the component,
do not mirror it from the shell.

Sequence after 5.2 lands; both touch `shell_playback.rs` / `shell.rs`.
