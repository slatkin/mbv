# Delta for unified-playback-queue

## Modified Requirements

### Requirement: Queue occurrences have stable slot identity

A Client-side queue replacement SHALL preserve monotonic slot allocation within its `PlaybackQueue`; replacement SHALL NOT re-mint an identity already issued by that tab, even when the old Playback run still contains that numeric slot. While a local owner is active, a Client SHALL compare its queue generation with `PlayerStatus.sequence_generation` before issuing a slot-addressed jump. On mismatch, explicit play SHALL submit the canonical queue at the selected cursor rather than issue `JumpTo`.

### Requirement: Each queue has one canonical ordered representation

A populate-only queue replacement MAY update the Client's Composed snapshot without submitting it to the active owner. Until explicit play submits the replacement, the Client SHALL NOT present an unconfirmed slot from that replacement as the playing slot.
