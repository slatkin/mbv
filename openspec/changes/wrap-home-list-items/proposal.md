## Why

The Home tab's right-hand list currently truncates item labels to keep every item at one terminal row. This makes the panel scan-dense, but hides useful information—especially long movie, episode, and music labels—when the user is trying to understand the current Home data at a glance.

## What Changes

- Replace Home list label truncation with terminal-width-aware wrapping of the content that the list already displays.
- Keep non-episode items at one row when their existing rendered label and duration fit; add indented continuation rows only when needed.
- Keep an episode in the existing inline series/title layout when it fits; when it does not, render it as a coherent stacked series/title layout instead of squeezing two truncated columns together.
- Keep the current Home content semantics unchanged. In particular, do not add artist, album, year, or other metadata for Music items; wrap whatever the Home list currently displays for that item.
- Preserve marker, duration, color, selection, and mouse behavior while making them span or align correctly across variable-height items.
- Update scrolling, content height, scrollbar position, hitmaps, and tests for rows that occupy more than one terminal line.

## Capabilities

### New Capabilities

None. This improves the presentation of existing Home list content.

### Modified Capabilities

- `home-list-wrapping`: Home list items remain fully readable by wrapping rather than truncating their existing labels.

## Impact

- **Code**: Primarily `src/app/render/home.rs`, with related Home rendering tests and any small layout/scroll helper adjustments required by the existing variable-height list patterns.
- **Behavior**: Home list items may occupy multiple terminal rows and will reflow when the terminal width changes.
- **Data/API**: No changes. Existing `MediaItem` fields and Home section semantics remain unchanged.
- **Risk**: Medium. Rendering is localized, but cursor visibility, physical scroll offsets, scrollbar mapping, and mouse hitboxes must remain synchronized with variable row heights.
