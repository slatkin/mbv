# Proposal

## Why

The Queue chooser (`QueueChooserMenu`) and the playback-target chooser
(`TargetChooserPopover`) paint one menu shell and one row mechanic in two
places: the card style, the row height, padding, radius and type, the cursor
fill, the `menu` and `menuitem` roles, and the `onmounted` mark call. The
target chooser's own comment says it copies the Queue chooser's card. A fix to
one reaches the other only by hand. Invariant 38 lists this pair as a known
failure, and GitHub issue #1059 tracks it.

## What Changes

- Add one `ChooserMenu` card component and one `ChooserRow` component in
  `crates/mbv-gui/src/window/components/chooser_menu.rs`.
- `ChooserMenu` owns the card chrome, the `menu` role, the width bounds, the
  padding, the item gap, the drop under its anchor, and `--hover-veil:
  transparent`.
- `ChooserRow` owns the row height, padding, gap, radius, type, cursor fill,
  the `menuitem` role, and the call of the mounted mark. It reports its mark and
  resolves no press.
- `QueueChooserMenu` keeps its row content: leading check, label or rename
  draft, playing, current and peer marks, and count.
- `TargetChooserPopover` keeps its row content: leading check, target icon,
  label, and the Refresh row.
- Cursor state and pointer dispatch stay with their current owners (the core and
  the region sets). No behavior changes: no look, hit-region, mark-order or key
  change.
- The two choosers' row marker attributes unify under the shared components
  (see design D3), and the one stylesheet rule and the test selectors that name
  the old markers follow.
- `LibraryDropdown`, `QueueRowMenu`, and the track popover are out of scope
  (different token families or menu semantics).
- Invariant 38 and its issue note record the pair as shared.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a pure refactor with no spec-level behavior change, so the change
sets `skip_specs: true`.

## Impact

- `crates/mbv-gui/src/window/components/`: new `chooser_menu.rs`;
  `queue_chooser_menu.rs` and `queue_target_control.rs` shrink;
  `components.rs` registers the module.
- `crates/mbv-gui/assets/app.css` and the generated `app.generated.css` (one
  selector).
- `crates/mbv-gui/src/window/components/queue_panel/tests/cross_panel.rs` (one
  selector).
- `docs/invariants/38-repeated-gui-shapes-have-one-mechanics-owner.md`.
- No dependency, protocol, core, or frame change.
