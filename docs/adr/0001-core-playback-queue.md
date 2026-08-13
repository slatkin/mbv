# Core Playback Queue

We will model the playback queue as a core playback concept in `mbv-core`, not as TUI-only state. The queue model owns ordered queue slots, stable runtime slot identity, active playback identity, queue revision, and progress merge/protection rules; client-specific cursor, scroll, focus, and visible local/remote scope remain UI state. This lets local playback, direct remote control, daemon/thin-client control, and mpv adapter events share one queue authority instead of maintaining parallel index-based mirrors.

A Playback run operates over one `PlaybackQueue`: standalone playback is a
one-slot queue, not a separate domain concept. mpv projects mbv's queue and
reports adapter observations such as playlist position changes, but it does not
own queue authority. The projection may mirror every slot or materialize only
the active slot for a lifecycle-backed source. Server refresh/enrichment can
update metadata for inactive slots, but it must not overwrite active local
progress or progress with a pending Service synchronization.
