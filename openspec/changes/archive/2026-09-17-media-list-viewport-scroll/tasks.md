## 1. Raise rule keeps the group label

- [x] 1.1 In `MediaListCore::resolve_viewport`, extend the raise branch over the contiguous
      non-selectable rows directly above the selection, clamped at row 0.
- [x] 1.2 Owner tests: moving the cursor back up into a group re-shows that group's `Heading`
      above its first item; the walk stops at the previous group's selectable row; a list with no
      heading above the selection raises exactly to the selection's row; the bottom branch and the
      stored scroll are unchanged.
