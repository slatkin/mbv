## REMOVED Requirements

### Requirement: Hero title renders as large Octant text

**Reason**: Never implemented (no `BigText` use exists in the tree), and the unified Hero header renders
titles through one title/metadata presentation for all header types. Removed by user decision.

**Migration**: `library-panel` — "Wide Hero header has three types chosen by content kind".

### Requirement: Title wrap width accounts for Octant glyph width

**Reason**: Applies only to the unimplemented Octant title.

**Migration**: None; titles wrap at their text column width.

### Requirement: Title height accounts for Octant line height

**Reason**: Applies only to the unimplemented Octant title.

**Migration**: None; each title line occupies one row.

### Requirement: Title style preserves yellow bold appearance

**Reason**: Applies only to the unimplemented Octant title; the header's title style is owned by the one
title presentation.

**Migration**: `library-panel` — "Wide Hero header has three types chosen by content kind".
