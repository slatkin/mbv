# queue-canonical-list

## MODIFIED Requirements

### Requirement: Queue projection is bounded presentation data

Queue SHALL project selectable rows with stable opaque `QueueSlotId` targets and presentation metadata, semantic state distinguishing the now-playing row from resume-progress rows, and optional integer `progress_percent` clamped to `0..=100`. The projection SHALL NOT carry ticks, runtime, source preparation, credentials, callbacks, or provider effects.

The now-playing row SHALL show its total duration like every other row. It SHALL project the `NowPlaying` semantic state with live progress following the playback position; the live percentage renders inline as trailing metadata, exactly as resume progress does. No throbber glyph SHALL appear in the row. When runtime is unknown the row SHALL project no `progress_percent` and show no duration. A non-active row SHALL project its stored resume position as inline trailing progress metadata, whatever the item's provider kind — feed, Audiobookshelf, or Emby — with the duration slot unchanged. The playing row's live ticks SHALL win over that stored position.

The projection's semantic active state — which queue scope is playing, which slot within it, and whether that slot is confirmed by the playback owner or is an optimistic prediction awaiting confirmation — SHALL be one owned value, reconciled against playback-owner status in one place outside the render path. An optimistic prediction SHALL record why it is optimistic: a queue edit relocated the still-playing item, or a different item was selected to play. Reconciliation SHALL clear a prediction once the owner's reported slot and queue length match it.
