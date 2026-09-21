# Design

## Context

See `proposal.md` for motivation and `specs/home-latest-sections/spec.md` for behavior. Home currently stores Latest sections as title/source/items tuples and projects their titles as plain strings into the Library panel's shared Selector row. The Home owner already owns section selection while the shell independently merges Emby, Audiobookshelf, and Feed sections as their asynchronous data arrives.

The canonical media-list row already supports a fixed right-aligned `MediaListTrailing::Gutter(String)` in the green metadata role, and `fmt_publish_date_short` already produces the required `17 Sep` form. Home currently projects no trailing metadata. Provider timestamps already survive in the item snapshots (`EmbyItem::date_added`, Audiobookshelf and Feed `pub_date_secs`), although Emby's text timestamp still needs normalization before comparison.

The in-progress `persist-tui-launch-state-on-exit` change deliberately writes a navigation snapshot only at orderly exit. This change requires a different lifecycle: read-and-advance a global launch cutoff immediately at startup. The two records must remain separate.

## Goals / Non-Goals

**Goals:**

- Derive one frozen launch interval and apply it consistently as provider sections arrive asynchronously.
- Keep visited/acknowledged Latest-section state inside the Home Interactive Component, keyed by stable `HomeLatestSource` identity.
- Reuse the shared Selector row and canonical media-list gutter without a Home-only painter or parallel row path.
- Make startup timestamp replacement tolerant and atomic without adding a dependency.

**Non-Goals:**

- Unread counts, per-item read state, per-library acknowledgement timestamps, or synchronization through a Player owner.
- Marking primary navigation tabs, Continue, Audiobookshelf book libraries, or hidden Latest sections.
- Reacting to content first published after the current process's launch instant.
- Changing Home section ordering, item limits, refresh behavior, or provider APIs.

## Decisions

### 1. Persist one dedicated startup cutoff, separate from TUI launch-location state

Add a small versioned per-user state record containing the last client-launch Unix timestamp. Startup captures `current_launch` once, tolerantly loads `previous_launch`, and atomically replaces the record with `current_launch` immediately. The process retains the immutable pair as its launch window.

A dedicated record preserves the two lifecycle contracts: this timestamp is a startup write, while `TuiLaunchState` remains an orderly-exit-only navigation snapshot. Reusing `prefs.json` or the launch-location file would make an unrelated write silently violate one of those contracts.

The state writer should reuse the project's existing state-directory and atomic-replacement conventions. Concurrent clients intentionally share one global baseline; each uses the value it read before its own replacement, and the last completed startup replacement supplies the next launch's baseline. No lock, Client identity, or merge is added.

Alternative considered: save the current launch time at orderly exit. Rejected because a crashed process would make the next client compare against something other than the actual previous launch.

### 2. Normalize provider timestamps once, then use a closed launch window

Derive an optional Unix timestamp for each Home Latest item:

- Emby: parse `date_added` (`DateCreated`) as ISO 8601;
- Audiobookshelf podcast and Feed: use `pub_date_secs`;
- every other or invalid value: `None`.

One helper should supply the normalized timestamp used by both the launch-window comparison and the row-date formatter, preventing marker and gutter eligibility from diverging. An item is new only when `previous_launch < timestamp && timestamp <= current_launch`. The upper bound prevents future-dated provider data or clock skew from producing a marker on every launch. With no previous timestamp, the process establishes a baseline and classifies nothing as new.

Alternative considered: compare item identities with the prior launch's Latest lists. Rejected because it would persist bounded provider snapshots, mistake list-window churn for unread state, and solve a different problem from the requested provider-time semantics.

### 3. The shell projects launch-relative facts; Home owns acknowledgement

The shell owns the persisted launch window and provider normalization. Extend the Home section snapshot beyond the current tuple so each section carries its stable source, title, items, and whether any item is new in the frozen launch window. The shell recomputes that fact whenever it merges or replaces provider content.

The Home Interactive Component owns an in-memory set of visited `HomeLatestSource` identities. A pill is marked only when its projected section says it has new content, its source is not visited, and it is not the selected section. Selecting a section inserts its source into the visited set before the next paint. When selected content arrives asynchronously, content application inserts that selected source as visited. Since acknowledgement is stable-identity keyed, later refresh, merge, or reorder cannot resurrect the marker.

This keeps persistence and provider interpretation in the shell while keeping local visit state in the component. The shell never reads back or mirrors the visited set.

Alternative considered: clear a shell-owned marker in response to section selection. Rejected because that turns component-local acknowledgement into another shell mirror and makes asynchronous merge behavior depend on round trips.

### 4. Extend shared Selector-pill content with a semantic marker

The Selector row must carry typed pill content rather than encoding `•` into the label string. Add the narrowest marker field to the shared pill content model; the shared pill painter appends `•` in a semantic Iris role and includes its width in overflow and hit-region calculations. Other selectors supply no marker and retain their current appearance and behavior.

The Home owner never marks its active pill, because selection acknowledges the source before painting. This also avoids the poor Iris-on-selected-FOAM contrast identified during exploration without adding a selected-marker colour override.

Alternative considered: append `•` to Home's label string. Rejected because the glyph could not receive its own semantic colour and shared truncation, overflow, and pointer geometry could disagree about the label width.

### 5. Reuse the canonical media-row gutter for all dated Latest rows

While projecting the active Home section, set `MediaListTrailing::Gutter(fmt_publish_date_short(timestamp))` for a Latest item with a normalized provider timestamp. Continue projection always leaves `trailing` absent. The existing row painter owns placement, width, right alignment, and green role in every Panel mode; no rendering branch or new date format is needed.

Alternative considered: a Home-specific date column. Rejected because the canonical gutter already expresses exactly this provider-neutral row metadata and is shared across Wide, Narrow, and Mini geometry.

## Risks / Trade-offs

- [Two clients start nearly simultaneously and observe the same prior cutoff] -> Both may show the same markers, which is acceptable for one global per-user launch baseline; atomic replacement prevents corruption.
- [A provider reports an incorrect old date for newly discovered content] -> It is not marked, by explicit provider-timestamp semantics; the row still displays the reported date when parseable.
- [A provider reports a future timestamp] -> The closed upper bound suppresses the marker until a later launch window contains it; the date gutter still truthfully displays the provider date.
- [The latest endpoint returns only a bounded item set] -> The marker answers whether the returned Latest section contains a qualifying item, not whether the server has any qualifying item outside that endpoint's window.
- [Selector content-type changes affect every destination at compile time] -> Keep the field optional/default-unmarked and update shared painter tests once; this is preferable to a second Home-only selector painter.
- [The startup state file is malformed or unwritable] -> Treat malformed/missing data as no baseline; log write failure and continue startup without markers rather than delaying Service-independent startup.

## Migration Plan

1. Introduce the launch-cutoff state record and tolerant atomic startup read-and-replace path without touching the exit-only TUI launch-location snapshot.
2. Add normalized provider timestamp projection and carry launch-relative facts into typed Home Latest sections.
3. Add component-owned visited-source state and typed Selector-pill markers.
4. Project provider dates into the existing canonical gutter for Latest rows only.
5. Remove no legacy state: the first launch after upgrade establishes the baseline and intentionally shows no markers. Rollback can ignore or delete the dedicated timestamp file without affecting other state.
