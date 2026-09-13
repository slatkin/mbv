## REMOVED Requirements

### Requirement: Playback panel renders in queue-only only when playback is active

**Reason**: Superseded by `queue-playback-panel`. The panel is no longer a queue-only surface: it
renders in the queue column in every queue-visible layout (`both` and `queue-only`), and the
"connected transport that is not playing keeps the panel" exception is removed, so idle hides the
panel everywhere.

**Migration**: `queue-playback-panel` — "The playback panel renders in the queue column" and "Idle
collapse in queue-visible layouts". Callers that relied on queue-only being the only in-queue panel
site should read the new capability instead.

### Requirement: Narrow layout stacks image and playback panel vertically

**Reason**: Superseded by `queue-playback-panel`. The stacked placement is not queue-only behaviour;
it is how the queue column arranges its visual slot and panel below 100 columns, in every
queue-visible layout.

**Migration**: `queue-playback-panel` — "The playback panel renders in the queue column", scenario
"Narrow terminal stacks vertically".

### Requirement: Wide layout places image and playback panel side by side

**Reason**: Superseded by `queue-playback-panel`. The side-by-side placement applies to the queue
column in every queue-visible layout, not only queue-only.

**Migration**: `queue-playback-panel` — "The playback panel renders in the queue column", scenarios
"Wide terminal renders two columns" and "Panel height and width follow the visual slot".

### Requirement: Idle queue-visible layouts collapse the card into the queue list

**Reason**: Superseded by `queue-playback-panel`, which keeps the collapse behaviour and replaces the
card-only wording with the Queue playback panel's visual slot plus transport. The connected-but-idle exception in this
requirement was already stated in the panel requirement above and is removed by this change.

**Migration**: `queue-playback-panel` — "Idle collapse in queue-visible layouts".

### Requirement: Wide layout playback panel height matches image height

**Reason**: Superseded by `queue-playback-panel`; the height rule is a property of the queue column's
wide two-column arrangement, not of queue-only mode.

**Migration**: `queue-playback-panel` — "The playback panel renders in the queue column", scenario
"Panel height and width follow the visual slot".

### Requirement: Hero image left-aligned in wide layout

**Reason**: Superseded by `queue-playback-panel`; the visual slot's left alignment is a property of
the queue column's wide two-column arrangement.

**Migration**: `queue-playback-panel` — "The playback panel renders in the queue column", scenario
"Wide terminal renders two columns".
