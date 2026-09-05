# Design: canonical-list-duration-kind

## Context

See proposal.md Why. One painter (`wide_media_row`) paints `duration: Option<String>` verbatim; each parent formats its own string (`short` vs `approx` vs `mmss` vs feeds duplicate). Folder-ness lives in `primary` strings, copied twice in `browser/mod.rs`.

## Goals / Non-Goals

Goals: one duration format in rows; `MediaKind` on `Item`; fold `mmss` + feeds duplicate.
Non-goals: hero/detail/modal `approx` strings; year-per-destination policy; moving count suffixes out of `primary` (kind documents intent, strings stay for now).

## Decisions

1. `MediaKind { Collection, Media }` field on `MediaListRow::Item` (enum, not bool): carries drill-in vs play intent and future count ownership; bool only hides duration and leaves the `browser/mod.rs` string duplication.
2. Painter suppresses `duration` for `Collection` even if projected — one enforcement point, parents can't re-diverge.
3. One helper `list_duration_secs(i64) -> Option<String>` over `fmt_duration_short` with the `>0` guard every site hand-rolls; callers pass raw seconds.
4. Fold `fmt_duration_mmss` (modal-only, unbounded `62:03`) and `feeds_model::format_duration` (byte-identical clone of short) into the helper; delete both. `approx` stays for hero/modal (follow-up).
5. `duration: None` rows unchanged — no new columns, no width pressure on browser/albums/shows.

## Risks / Trade-offs

- Characterization tests expecting `4m` break → update to `M:SS` in same commit.
- `short` is wider than `approx` (`4:32` vs `4m`) → painter already truncates `primary` against duration reserve; buffer test at narrow width covers it.
- Queue `pos / dur` active-row form stays — helper formats each side, no behavior change.

## Migration Plan

Implement kind + helper + painter rule, migrate TV/music/book/feed/modal projections, delete duplicates, run `cargo nextest` + buffer tests + `ast-grep scan`. No rollback needed (pure UI text).
