# Tasks

## 1. Frame: drop the now-playing status fact

- [x] 1.1 Delete `GuiNowPlayingStatus` and the `status` field from `GuiQueuePlayback`
  (`crates/mbv-gui/src/bridge/frame/playback.rs`), drop the conversion in the bridge
  sections builder, and remove the field from every test fixture that constructs a
  playback section. The library projection keeps its status fact; only the conversion
  goes.

## 2. Player panel: remove the header row

- [x] 2.1 Remove `HeaderRow` and the popover/chooser plumbing from
  `queue_playback_panel.rs`: the artwork becomes the panel's topmost content (design D6),
  the component stops reporting `PlaybackTarget`, `Collapse`, and chooser-row regions,
  and its module comment no longer names the header row.
- [x] 2.2 Move `TargetChooserPopover` to `queue_footer.rs` and flip its anchor to
  `bottom-full` so it opens above the footer band (design D3).

## 2b. Global header and Library Status bar

- [x] 2b.1 Move the `LibraryTopBar` mount from `library_panel.rs` to `app.rs`, above the
  panel row and across both columns or the collapsed rail; absent on the Settings screen
  (design D7).
- [x] 2b.2 In Mini with the Queue showing, a tab press makes the destination active and
  shows the Library (design D8).
- [x] 2b.3 Delete `HINTS` from `library_status_bar.rs`; move the Search field and its
  dropdown from `library_top_bar.rs` to the bar's left side, dropdown anchored
  `bottom-full`; prefix and toast paint after the field (design D9).

## 3. Footer: own the target and collapse controls

- [x] 3.1 Paint the playback-target control and the collapse control at the left of
  `queue_footer.rs`'s left group, ahead of the autosave/source/dirty content, compacted
  to the 40 px band (design D1, D2). The status word does not move: no surface paints it.
- [x] 3.2 Report the moved regions: the footer's contribution carries the `PlaybackTarget`
  and `Collapse` regions, and the open chooser's rows report under a second key the
  footer owns (design D4). Update the footer's module comment — the band no longer
  resolves no target.

## 4. Tests and docs

- [x] 4.1 Move the header-row and popover coverage with the surfaces: the playback panel
  suite loses the header's regions; the footer suite gains the controls, the popover's
  upward anchor, and the no-status-word claim. The top-bar suite loses the Search
  field; the Library Status bar suite gains it and loses the hints. Add the header's
  mount placement and its absence on Settings, and the Mini tab press. Name the
  contract each moved test owns.
- [ ] 4.2 Update `docs/architecture/gui-client.md` and both components' module comments
  to describe the headerless panel, the footer's new controls, the global header,
  the Search field's place, and the `Ctrl+K` rule.

## 5. Verification

- [ ] 5.1 `cargo check -p mbv-gui`, `cargo nextest run -p mbv-gui`, `cargo fmt`,
  and the complexity pass over the touched files.
