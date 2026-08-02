## 1. TV Browse Scope

- [x] 1.1 Trace the TV library root, refresh, pagination, and position-restore paths and ensure top-level TV browse requests use `Series` items while nested season and episode requests retain their current scopes.
- [x] 1.2 Update letter-pill eligibility and default-filter handling so large TV roots use the true unfiltered series total without changing movie behavior.
- [x] 1.3 Verify that active TV letter filters are passed through initial loads, refreshes, pagination, and restored positions with the existing range bounds.

## 2. Pill Interaction And Rendering

- [x] 2.1 Reuse the shared alphabet-pill renderer for eligible TV tabs and confirm the row reserves the same layout space and exposes the same hitboxes as the movie row.
- [x] 2.2 Confirm mouse selection, keyboard cycling, cursor/scroll reset, loading state, wraparound, and saved-position restoration operate for TV libraries.
- [x] 2.3 Confirm TV list grouping and sorting use series/show names and do not group or filter by episode or season names.

## 3. Verification

- [x] 3.1 Add action tests for large/small TV eligibility, `Series` query scope, range selection, wraparound, and restoration behavior.
- [x] 3.2 Add rendering tests that assert TV alphabet pills and per-letter grouping render correctly, while preserving existing movie regression coverage.
- [x] 3.3 Run the focused Rust tests, the relevant full test suite, and `openspec validate --change "add-tv-letter-pills"`.
