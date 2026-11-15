# Invariant 33 — A row's resume state matches what Play does

**Scope:** the row resume-state derivation
(`MediaSemanticState::from_progress` and its per-provider constructors
`from_emby`, `from_feed_entry`, `from_audiobookshelf`, and `from_queue_item`
in `crates/mbv-ui-model/src/media_list.rs`), the shared gate
(`mbv_emby_model::should_resume` in `crates/mbv-emby-model`), the Player's
start-position decision (`resume_source` and `source_start_seconds` in
`crates/mbv-player/src/resume.rs`), the feed store that keeps feed rows in
shape (`crates/mbv-feed/src/entry_state.rs`), and every list site that
derives row state from an item (TUI Podcasts and Feeds content, the GUI
Library projections, and the Queue). Landed by the
`row-state-follows-playback-resume` change.

## The invariant

A row paints Active exactly when the Playback resume rule would resume the
item from a stored position: a position that passes the `should_resume`
one-percent gate against a known or unknown runtime. It paints Played only
when Play would start the item over, and it never paints Active on a fact
that Play would ignore. The row rule is a prediction of the Player's start
decision, kept in step by construction: both sides gate a position with the
same `should_resume`, and the Audiobookshelf finished rule — a finished
item restarts from the beginning — is the row's prediction of the server's
own behavior.

## Why it matters

The row state is a promise to the user. A row that shows Played while Play
would resume (or a row that shows a resume percentage while Play would
start over) tells the user the wrong thing about the next press of Play:
the track they expect to hear from the top picks up mid-sentence, or the
podcast they expect to continue restarts. The two sides cannot share code
— `resume_source` is `mbv-player`-internal and reads the opened session,
which no row can see — so the row rule duplicates the Player's gate. A
silent drift between the two copies is exactly the kind of bug this
invariant exists to catch, and a per-list hand match on `played` or
position is how the drift started: before one body existed, four list
sites let the played flag win over a resumable position, so those rows
said "played" for items Play would resume.

## How the code upholds it today

- `MediaSemanticState::from_progress` is the one body: `should_resume`
  wins as `Active` with the bounded percentage, then `played` wins as
  `Played`, otherwise the row is `Ordinary`. No list matches on `played`
  or position itself; every list derives through a per-provider
  constructor that owns the provider exceptions on top of the body.
- `from_emby` keeps the music rule (fire-and-forget, always `Ordinary`),
  `from_feed_entry` feeds the body the entry's locally recorded position,
  and `from_audiobookshelf` applies the Audiobookshelf finished rule: a
  finished item yields `Played` whatever position is stored, because the
  server's `PlaybackSessionManager.startSession` (advplyr/audiobookshelf)
  keeps the start time at 0 for a finished item. `from_queue_item`
  dispatches per kind and adds nothing of its own.
- The Player side never reads `played` either: `source_start_seconds`
  derives the start position from `resume_source` per kind and gates the
  stored position with the same `mbv_emby_model::should_resume`. Emby
  audio always starts from the beginning, matching `from_emby`'s
  `Ordinary` rule.
- The feed store is the one writer of feed row state. Playback completion
  and a Mark Played write position zero with the played flag, so a played
  feed entry cannot carry a resumable position that would fight the body;
  `load` applies the same rule to rows it reads from disk.

## Where it still fails

- A new list that derives row state by hand — matching on `played`, a
  position, or a provider progress type directly instead of calling a
  per-provider constructor — reintroduces the old precedence split.
  Nothing at the type level forces a new site onto the constructors.
- A `resume_source` or gate change without a row-rule change breaks the
  prediction: the Player starts from somewhere the rows no longer show.
  The two bodies sit in different crates, so a compiler cannot link them;
  the Playback resume spec (`openspec/specs/playback-resume/spec.md`)
  owns both statements, and a source change must update both.
- The Audiobookshelf finished rule is a prediction of server behavior,
  not a contract with it. If the server's
  `PlaybackSessionManager.startSession` ever resumes a finished item from
  its stored position, rows keep painting Played (restart) while Play
  resumes, until the rule here changes.
- Queue slots of feed entries take a Feeds mark through the
  `QueueOp::ApplyProgress` path, and Home `Recently added` rows of feed
  entries update only on the next fetch. A row can lag its store state
  until its own projection refreshes; that lag is by design and distinct
  from a rule mismatch.
