## Why

The narrow library view currently pins a hero above the list. As selection changes, the hero's
content height changes and pushes the list below it, making scrolling unstable; fixing the height
would require truncating the detail that makes a hero useful. The earlier inline hero convention
kept the selected item's detail in the list flow and made scrolling feel more natural.

## What Changes

- Restore the inline hero for the standard one-column narrow library view.
- Apply the inline treatment consistently to every library browse view using the shared hero/list
  surface, rather than retaining per-library narrow exceptions.
- Make the active row's selected item render its hero inline in the list; moving the cursor moves the
  hero with that row.
- Preserve the existing hero content, metadata, artwork behavior, selection behavior, and wide
  hero-on-left or hero-on-top presentations.
- Remove the narrow requirement that the hero occupy a separate fixed-height area above the list.

## Capabilities

### New Capabilities

### Modified Capabilities

- `right-panel-arrangements`: narrow one-column library presentation changes from hero-on-top to an
  inline hero within the scrolling list, while wide arrangement assignments remain unchanged.
- `library-list-hero`: narrow library hero placement, cursor-following behavior, row mapping, and
  scrolling semantics change to restore the inline hero convention.

## Impact

- Affected TUI arrangement dispatch, one-column library row layout, hero placement, scrolling, and
  mouse hit-target bookkeeping.
- No service/runtime protocol, persisted data, or external API changes.
- Existing wide library layouts and non-library screens are unchanged.
