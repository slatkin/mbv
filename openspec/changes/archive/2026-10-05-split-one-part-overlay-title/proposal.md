# Proposal

## Why

A one-part title (no context part) uses only the top scrim row on the queue column's now-playing artwork. Long titles, which are common for home videos, shrink to the minimum size or get an ellipsis, but the bottom row stays empty.

## What Changes

- A one-part title that does not fit its row at the nominal size splits at the space nearest its middle. The first half goes in the top row and the second half in the bottom row. Both halves paint yellow.
- Each half then uses the existing fit rule: it shrinks, and then it ends with an ellipsis.
- A title with no space does not split. It shrinks as it does today.
- The rule applies to every one-part title: movies, home videos, feed entries, tracks without context and remote-session items.
- A one-part title with a loaded logo stays the logo alone. Two-part titles do not change.

## Capabilities

### New Capabilities

### Modified Capabilities
- `queue-artwork-title-overlay`: a one-part title can use the bottom row, and splitting comes before shrinking.

## Impact

- `crates/mbv-images/src/title_overlay.rs` (`compose_title_overlay`, plus a split helper). The cache key and the header-or-artwork site decision do not change.
