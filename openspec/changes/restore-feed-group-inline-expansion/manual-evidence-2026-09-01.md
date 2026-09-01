# Manual / harness evidence — 2026-09-01

A real configured Emby/feed service was not available in this worktree, so no interactive terminal session or real-feed check was possible. The closest available harness is the existing `Model::draw_frame` + Ratatui `TestBackend` fixture in `src/app/tests_narrow_browse_migration.rs`.

Ran:

```text
rtk cargo nextest run -p mbv feed_home_video_group --no-fail-fast
cargo nextest: 10 passed, 1234 skipped (1 binary, 0.064s)
```

This exercises the fixture's 60x20 Narrow and 140x40 Wide captures, including inline expansion/banner content, feed-group pills, single-paint row geometry, and browser scroll projection. The fixture does not exercise 100x30, and the harness assertions are automated rather than manual visual inspection. Therefore the requested 100x30 and real-service interactive evidence remain outstanding; no behavior is claimed for them.
