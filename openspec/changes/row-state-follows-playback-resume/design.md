# Design

## Context

See `proposal.md` for the motivation. The facts below shape the approach.

- `MediaSemanticState` (`crates/mbv-ui-model/src/media_list.rs`) is the row state type of every TUI list and the Queue. `from_progress(played, position, runtime)` lets any position above zero win. `from_emby` and `from_queue_item` add the music rule on top of it. The progress percentage already has one constructor, `ActiveProgress::of_ticks` (#995).
- Four sites do not use `from_progress`. They let played win:
  - TUI Podcasts: `crates/mbv-components/src/podcast_content.rs`, an inline `match` on `AudiobookshelfProgress`.
  - TUI Feeds: `crates/mbv-components/src/feeds_content.rs`, an inline `if` on `FeedEntry`.
  - GUI Podcasts: `src/app/gui_library/podcasts_projection.rs` (`episode_row_state`), through `GuiHomeRowState::from_progress`.
  - GUI Feeds: `src/app/gui_library/feeds_projection.rs` (`feed_row_state`), through `GuiHomeRowState::from_progress`.
- `GuiHomeRowState` (`src/app/gui_library/home.rs`) is a second enum with the shape of `MediaSemanticState` minus `NowPlaying`. `media_row_state` converts one into the other. The bridge then converts `GuiHomeRowState` into the wire DTO `GuiHomeRowStateFrame` (`crates/mbv-gui/src/bridge/frame/library/home.rs`). The Queue DTO `GuiSemanticState` converts directly from `MediaSemanticState`.
- The Player's start position comes from `resume_source` and `source_start_seconds` (`crates/mbv-player/src/resume.rs`). They never read `played`. They gate a position with `mbv_emby_model::should_resume`, the one-percent rule. For Audiobookshelf the position comes from the opened session. The server starts a finished item from zero (`PlaybackSessionManager.startSession`: `if (userProgress.isFinished)` keeps `userStartTime` at 0), but returns the raw `currentTime` for an unfinished item without mbv's gate. `session_start_seconds` applies the same gate to the session position and duration before mpv loads an episode or book.
- The date gutter repeats `provider_timestamp_secs(item).map(fmt_publish_date_short)` (or `published_at` for Audiobookshelf episodes) at about ten sites. Some sites filter the empty string that `fmt_publish_date_short` returns for an invalid timestamp, and some do not.
- `FeedEntryStore` (`crates/mbv-feed/src/entry_state.rs`) is the only writer of feed state. Playback completion writes played with position zero (`feed-queue-item` spec). `set_played` keeps the position. The loaded Feeds rows (`feed_tab.entries`, `feed_tab.all_entries`) read the store only when a fetch lands (`hydrate_feed_entries_for_subscription`).

## Goals / Non-Goals

**Goals:**

- One function body decides Active, Played, or Ordinary from (played, position, runtime) with the `should_resume` gate. Per-provider constructors feed it and own the provider exceptions (music, finished Audiobookshelf).
- Every list in both Clients calls a per-provider constructor. No list matches on played or position itself.
- One row-date helper.
- The Audiobookshelf Session source applies the one-percent gate to the position and duration its opened session returns.
- No change to any GUI wire DTO or to the Svelte front end.

**Non-Goals:**

- No change to the Handoff start-position override. It stays the one ungated start position.
- Queue slots of feed entries do not take a Feeds mark at once. They are Owner-held, and an update needs the `QueueOp::ApplyProgress` path that Audiobookshelf uses. That is separate work.
- Home `Recently added` rows of feed entries do not take a Feeds mark at once. They update on the next Home fetch, as today.
- The Music tree's album node (`crates/mbv-components/src/music_content/tree_target.rs`) keeps its own call to `from_progress(false, ..)`. Only the new gate applies to it.
- Labels, sort order, and Hero facts stay per Client (ADR 0036).
- The Watched and Unwatched filters keep reading the played flag.

## Decisions

### D1. Per-provider constructors over one gated body

`MediaSemanticState` keeps `from_progress(played, position_ticks, runtime_ticks)` as the one body:

```
if should_resume(position, runtime) -> Active { progress: ActiveProgress::of_ticks(position, runtime) }
else if played                     -> Played
else                               -> Ordinary
```

The constructors map provider facts onto it:

| Constructor | Input | Provider rule |
|---|---|---|
| `from_emby(&EmbyItem)` | exists | music -> `Ordinary`; else the body |
| `from_feed_entry(&FeedEntry)` | new | the body over `played`, `position_ticks`, `duration_ticks` |
| `from_audiobookshelf(finished, position_ticks, duration_ticks)` | new | `finished` -> `Played`; else the body with `played = false` |
| `from_queue_item(&QueueItem)` | exists | dispatches: Emby -> `from_emby`, Feed -> `from_feed_entry`, Audiobookshelf -> `from_audiobookshelf(item.played(), ..)`, Twitch -> `Ordinary` |

`from_audiobookshelf` takes ticks. The Podcasts callers convert `current_time_seconds` and `duration_seconds` with `mbv_emby_model::seconds_to_ticks`, as they do today for the percentage.

Alternative: one constructor that takes a `ResumeSource`-like provider tag. Rejected, because the provider exceptions then hide in a match on a tag, and `QueueItem`-free callers (the Podcasts rows hold `AudiobookshelfProgress`) would build a tag by hand.

Alternative: move `resume_source` into `mbv-queue` and derive the row from it. Rejected, because the Audiobookshelf start position comes from the session, which a row cannot read. The finished rule is a prediction of the server's behavior, and it belongs next to the row rule.

### D2. `GuiHomeRowState` goes

The GUI Library projections (`GuiHomeRow`, the Podcasts, Feeds, Shows, and entry-list rows) hold `MediaSemanticState`. `media_row_state` and `GuiHomeRowState::from_progress` are deleted. The bridge implements `From<&MediaSemanticState> for GuiHomeRowStateFrame` with today's `media_row_state` mapping: `NowPlaying` maps to `Active` with its progress. The wire DTO and the front end do not change.

Alternative: keep `GuiHomeRowState` as a projection type. Rejected, because a second enum with the same values is how the two precedence rules came to exist.

### D3. One row-date helper

`mbv_ui_model::ui_util::row_date(secs: Option<u64>) -> Option<String>` formats with `fmt_publish_date_short` and returns `None` for an empty result. Every date-gutter site calls it, with `provider_timestamp_secs(item)` or the episode's `published_at`. The only visible effect: an invalid timestamp paints no gutter on every list (some lists painted an empty gutter).

### D4. The feed store keeps played rows at position zero

`FeedEntryStore::put` stores position zero when `played` is true. `set_played` goes through `put`, so a Mark Played stores zero. Its doc comment changes. `FeedEntryStore::load` applies the same rule to the rows it reads. When it changed a row, it saves the store once and logs one line with the count. A save failure is logged and the corrected rows stay in memory, the same as other feed-state save failures.

Alternative: correct the rows in `App` construction. Rejected, because the store is the one owner of the rule, and its own tests can prove it.

### D5. A Feeds mark updates the loaded rows

After `set_feed_entries_played` writes the store, it applies each entry's stored state to every matching loaded entry in `feed_tab.entries` and `feed_tab.all_entries`, matched by `feed_id` and `guid`. The TUI sync pass and the GUI section ledger already re-project feed content from that state, so both Clients show the change on the next frame. The selection follows the existing filter rules when a filter hides the row.

### D6. Documents

- ADR 0036 gets an amendment section: row resume state, progress percentage, and row dates are Service facts. One shared rule per provider in `mbv-ui-model` derives them, and the two Library panels differ only in which facts they show and where. It points at `openspec/specs/playback-resume/spec.md` and does not name this change (`docs/adr/README.md` rules).
- A new invariant `docs/invariants/33-row-resume-state-matches-play.md` records: a row's resume state matches what Play does; how the code keeps it (D1, the gate shared with the Player); where it can fail (a new list that matches on played itself, a Resume source change without a row-rule change, a server that changes its finished behavior).
- `CONTEXT.md` *Playback resume* gets one sentence: a row shows Active exactly when this rule would resume the item, and a finished Audiobookshelf item restarts.

## Risks / Trade-offs

- [The Audiobookshelf finished rule predicts server behavior] -> The invariant names the server source it depends on. If the server changes, the row and Play disagree until the rule changes.
- [The Audiobookshelf server does not gate unfinished progress] -> `session_start_seconds` applies mbv's gate to the session's returned position and duration. If a future session path bypasses it, rows can show `Ordinary` while Play resumes.
- [Emby keeps a position after Mark Played] -> The row follows what Emby reports after its refetch. If Emby keeps a position, the row shows Active and Play resumes, which is accurate.
- [Tests pin the old precedence] -> Tests in `podcast_content`, `feeds_content`, `mbv-ui-model` `media_list`, and `src/app/tests/gui_library_{home,podcasts,feeds,shows}.rs` change with the rule. Each changed assertion moves to the new expected state; no test is deleted only because it fails.
- [`load` writes a file] -> It writes only when it changed a row, and at most once for each load.

## Migration Plan

The store correction runs at the first load after the upgrade (D4). Rollback: an older build reads the corrected file without change, because the row shape does not change.
