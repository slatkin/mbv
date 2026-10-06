# Invariant 19 — A root level's fetch fields stay aligned with the fetch armed to fill it

**Scope:** every site that arms a fetch for a browse root level (`spawn_browse`, `spawn_refresh`,
`spawn_tv_latest`, `spawn_tv_upcoming`, position restore) and `drop_stale_root_completion`, the
#745 guard that rejects a root `Loaded` whose fetch key no longer matches the root level. Landed
by the 2026-10-06 TV Latest/Upcoming pill fix.

## The invariant

When a fetch is armed to fill a root level, the level's fetch fields (`item_types`, `sort_by`,
`sort_order`, `letter_filter`) SHALL be stamped with the same values the arriving response will
carry. A response whose key differs from the root level's is dropped by
`drop_stale_root_completion` and replaced by a generic refetch under the root's old fields — so a
misaligned arming silently converts the intended fetch into the default one.

## Why it matters

The TV pill rework (2026-09-23) armed `spawn_tv_latest`/`spawn_tv_upcoming` on a root level that
kept its legacy `SortName`/`Series` fetch fields, while the pill responses carried
`DateCreated`/`PremiereDate` with `item_types=Episode`. Every pill response was therefore dropped
as "stale" and re-issued as a generic alphabetical all-episodes fetch: the Latest pill showed
4,567 episodes sorted by name (old episodes first) instead of the 30 newest, and Upcoming was
clobbered the same way. The server responses were correct the whole time — only the key mismatch
threw them away. Because fetches run on worker threads, the wrong content arrived without any
error; the only trace was the `browse.items.loaded` line showing alphabetical first items after a
pill switch.

## How the code upholds it today

`tv_mode_fetch_fields` (`src/app/dispatch/library/browse/tv.rs`) is the single source for each
`TvContentMode`'s fetch fields. `build_tv_latest_level`/`build_tv_upcoming_level` stamp their
levels from it, `spawn_tv_latest`/`spawn_tv_upcoming` align the root level from it before
spawning, `refresh_tv_letter_mode` stamps both the level and the `LevelFetchKey` from it (so
leaving a pill mode for `All`/`Range` explicitly restores `Series`/`SortName`), and the position
restore path aligns a restored pill-mode root whose saved fields predate the alignment. The
stale-root drop logs a `browse.root_completion_dropped_stale` warn event when it fires, so a
future misalignment is visible in `~/.local/state/mbv/mbv.log` instead of silently replacing
content.

## Where it still fails

The guard compares full fetch keys, so any future fetch site that mutates some level fields but
not all of them (or builds a response level with literals instead of `tv_mode_fetch_fields`)
re-introduces the mismatch. `LevelFetchKey::from_level` is the key both sides are compared with;
new fetch arming should stamp the level from the same constant source the response builder uses,
and a pill-shaped response appearing right before a `browse.root_completion_dropped_stale` warn
is the signature of this invariant being broken again.
