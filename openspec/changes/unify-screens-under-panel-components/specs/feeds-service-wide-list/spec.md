## REMOVED Requirements

### Requirement: Narrow behavior is preserved

**Reason**: Narrow Feeds changes by design: its second pill bar is removed, the Watched filter moves to
the List controls row, and its inline hero adopts the one Narrow inline hero form. Feeds conforms to
the Emby screens rather than preserving its own output.

**Migration**: `library-panel` — "The Browser pane has one Selector row and one List controls row" and
"Narrow inline hero has one form".
