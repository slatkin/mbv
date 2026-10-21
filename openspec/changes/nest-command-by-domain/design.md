# Design

## Context

`Command` has 27 variants. After issue #956, `App::dispatch` translates each flat variant to a
typed sub-enum and calls a sub-dispatcher. Verified dead ends for fixing the cyclomatic count
inside the flat shape: per-domain helper matches need `_` arms; or-patterns count as one arm
but cannot merge arms that build different sub-enum values; a generating macro hides arms from
the analyzer. Only the type shape can remove the translation.

## Decisions

### 1. Two levels: `Shell` versus `App`

`Model::dispatch_router_command` already intercepts `OpenHelp` and `RefreshCurrentView` and
sends the rest to `App::dispatch` through a `_` arm. The type now states that split:
`Command::Shell(ShellCommand)` and `Command::App(AppCommand)`. `Model` matches both variants
and, for `Shell`, both `ShellCommand` variants. `App::dispatch(&AppCommand)` cannot receive a
Model-only command, so the two `unreachable!` arms and the shell `_` arm go away.

### 2. `AppCommand` wraps the four sub-enums

`AppCommand { Visual(VisualIntent), Playback(PlaybackIntent), Navigation(NavigationIntent),
Panel(PanelIntent), QueuePlayCursor(usize), Quit }`. `dispatch` has six arms, each a direct call.
Cyclomatic is about 7. Each sub-dispatcher keeps its exhaustive match (3 to 13). No `_` arm is
added and no `ast-grep-ignore` is needed.

### 3. Rename the sub-enums to `*Intent`

`CONTEXT.md` lists "playback command" and "navigation command" under *Avoid* for
*Transport command* (the Player-owner protocol term). The #956 sub-enum names reused them.
Renaming all four to `VisualIntent`, `PlaybackIntent`, `NavigationIntent`, `PanelIntent`
removes the collision and keeps the four names uniform. `CONTEXT.md` gets a *UI command* term
for the nested `Command`, with *Avoid*: transport command (different concept).

### 4. Construction sites use `From`

Each sub-enum gets `impl From<X> for Command`. Sites write `PlaybackIntent::Stop.into()` or
pass `impl Into<Command>` where a function takes one. Sites that build
`Command::App(AppCommand::Quit)` or `Command::Shell(ShellCommand::OpenHelp)` write them in full.
The keybind action tables map action ids to `Command` through `key_policy.rs`
`command_for_policy`, which is the single construction hub, so the table files do not change.

### 5. No spec delta

Key routing outcomes, the `RouterOutcome` shape, and every user-visible effect stay the same.
Existing routing tests prove it. No existing requirement text changes.

## Risks / Trade-offs

- A large mechanical diff (about 100 sites) in one change. Mitigation: the compiler finds every
  site, and the commit changes no behavior. Order: types first, then callers.
- `RouterOutcome::Command(Command)` and `Deferred(Command)` keep their payload type, so
  `router.rs` changes only where it names variants.
- `PartialEq` and `Copy` derives: `Command` loses nothing. `SeekRelative(f64)` already blocks `Eq`.

## Open Questions

None. The `*Intent` names are a naming choice recorded in decision 3. The user can veto them
before apply.
