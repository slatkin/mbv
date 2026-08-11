## Why

The current grouped Music view uses the same hero-on-top and two-column list composition at wide widths as other libraries, but music browsing has a different hierarchy: the selected album and its tracks are a persistent workspace, while the grouped album list is supporting navigation. A Home-style wide split makes that relationship visible without changing the current narrow view.

## What Changes

- Preserve the current grouped Music layout below the existing 82-column breakpoint: group pills across the top, album hero above, and a one-column grouped album list below.
- At and above the breakpoint, render a Music-only horizontal split with a large Home-style album hero and persistent track list on the left and the album browser on the right.
- Move the music-group pills into the top of the right rail in wide mode.
- Render one album per row in the wide right rail, grouped beneath non-selectable artist headers; do not use the shared two-column album packing there.
- Show the selected album's tracks in the wide left pane even before track selection is active.
- Use the Home palette to shift visual focus between the right album browser and left track list without moving or replacing either pane.
- Make persistent wide-mode track rows mouse-interactive: single-click selects a track and shifts focus left; double-click plays it.
- Leave every non-Music library unchanged.

## Capabilities

### New Capabilities

- `music-library-hero`: Responsive grouped Music composition, persistent wide-mode album and track detail, one-column right-rail browsing, focus treatment, and track interaction.

### Modified Capabilities

- `library-list-hero`: Clarify that grouped Music preserves the standard hero-above-list composition below the breakpoint but uses its Music-specific side hero at wide widths.
- `stable-music-library-grouping`: Preserve settled grouping, album selection, and viewport continuity across both responsive Music compositions.

## Impact

- **Code**: Responsive library geometry and pill routing, grouped album rendering and cursor geometry, album hero/detail rendering, track loading and scrolling, Music keyboard/mouse hitmaps, and focus-aware styles under `src/app/`.
- **Behavior**: Narrow grouped Music is unchanged. Wide grouped Music replaces two-column album packing with a left album/track workspace and right one-column browser. Other libraries are unchanged.
- **Data/API**: None.
- **Dependency**: Assumes `remove-selectable-music-artist-headers` is applied so artist headers are visual labels only.
- **Risk**: Medium-high. The wide view introduces two independently scrollable visual regions and width-specific pill placement; real-terminal verification is required.

## Non-Goals

- Changing the current narrow grouped Music composition or interactions.
- Applying the Home-style wide split to movies, TV, podcasts, home video, feeds, or ungrouped music libraries.
- Changing music-level configuration, grouping identity, ordering, or settlement.
- Adding artist-level summary or bulk-action content.
