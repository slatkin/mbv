## 1. Configure the retained playback cache

- [x] 1.1 Change the mpv playback initialization retained/back demuxer limit from 10MiB to 100MiB while preserving the 50MiB forward limit.
- [x] 1.2 Confirm the initialization change applies consistently to playback runs containing Emby items, feed entries, or both, without changing source URL construction.

## 2. Verify playback behavior

- [x] 2.1 Run `cargo check -p mbv-core`.
- [ ] 2.2 Play the high-quality Nextlander video feed entry through mbv and confirm it no longer repeatedly buffers or produces audio underruns under the tested conditions.
- [ ] 2.3 Confirm a mixed Emby/feed queue still transitions through both item kinds and that Emby playback retains its existing streaming behavior.
