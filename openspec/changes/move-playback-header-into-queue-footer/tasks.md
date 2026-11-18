# Tasks

## 1. Frame: drop the now-playing status fact

- [ ] 1.1 Delete `GuiNowPlayingStatus` and the `status` field from `GuiQueuePlayback`
  (`crates/mbv-gui/src/bridge/frame/playback.rs`), drop the conversion in the bridge
  sections builder, and remove the field from every test fixture that constructs a
  playback section. The library projection keeps its status fact; only the conversion
  goes.

## 2. Player panel: remove the header row

- [ ] 2.1 Remove `HeaderRow` and the popover/chooser plumbing from
  `queue_playback_panel.rs`: the artwork becomes the panel's topmost content (design D6),
  the component stops reporting `PlaybackTarget`, `Collapse`, and chooser-row regions,
  and its module comment no longer names the header row.
- [ ] 2.2 Move `TargetChooserPopover` to `queue_footer.rs` and flip its anchor to
  `bottom-full` so it opens above the footer band (design D3).

## 3. Footer: own the target and collapse controls

- [ ] 3.1 Paint the playback-target control and the collapse control at the left of
  `queue_footer.rs`'s left group, ahead of the autosave/source/dirty content, compacted
  to the 40 px band (design D1, D2). The status word does not move: no surface paints it.
- [ ] 3.2 Report the moved regions: the footer's contribution carries the `PlaybackTarget`
  and `Collapse` regions, and the open chooser's rows report under a second key the
  footer owns (design D4). Update the footer's module comment — the band no longer
  resolves no target.

## 4. Tests and docs

- [ ] 4.1 Move the header-row and popover coverage with the surfaces: the playback panel
  suite loses the header's regions; the footer suite gains the controls, the popover's
  upward anchor, and the no-status-word claim. Name the contract each moved test owns.
- [ ] 4.2 Update `docs/architecture/gui-client.md` and both components' module comments
  to describe the headerless panel and the footer's new controls.

## 5. Verification

- [ ] 5.1 `cargo check -p mbv-gui`, `cargo nextest run -p mbv-gui`, `cargo fmt`,
  and the complexity pass over the touched files.
