# Make the shell's `Msg` dispatch exhaustive

## Why

Issue #628. A component can emit a typed request that no shell handler
consumes, and nothing — not the compiler, not a test, not the running app —
says so. The component updates its own view, so the surface *looks* like it
works; only the shell-side effect is missing.

`Model::handle_terminal_message` (`src/app/shell_messages.rs:6`) is a single
flat match over `Msg` carrying **85 inline `Msg::Shell(ShellRequest::Specific)`
patterns** against the **89 `ShellRequest` variants** that exist. It never
destructures `Msg::Shell(request)` and dispatches on the inner enum. Its final
arm is:

```rust
// No other Msg variants are produced yet.
_ => {}
```

That comment is false. Because the match is flat, the wildcard does not only
catch unproduced `Msg` variants — it catches **every `ShellRequest` variant
with no handler**, silently. The arm was written believing it guarded an
unreachable case; it is in fact the app's largest silent failure surface.

The live consequence found so far:
`ShellRequest::AudiobookshelfPodcastEpisodeTransition` is emitted from five
sites in `components/audiobookshelf_podcast.rs:217-249` and matched **only in
tests**. In ABS podcast episode mode, `j`/`k` move the component's own view but
never the App-side episode selection. `AudiobookshelfPodcastEpisodeIntent`
(Space / Enter / Ctrl+A) then resolves its target episode from App state
(`msg/shell.rs:254-257`), so the action fires on the **wrong episode**. Filter
cycling and mode exit do not propagate either. This is not a dead key; it is
silent wrong-item playback.

This change is scheduled **first** among the post-migration repairs, because it
converts the rest of that backlog from manual discovery into a compiler
worklist. Every subsequent routing fix (#627) gets its missing wiring pointed
at by `rustc` rather than found by a maintainer using the app.

## What Changes

- **`handle_terminal_message` destructures `Msg::Shell(request)` and dispatches
  to an exhaustive inner match over `ShellRequest`, with no wildcard arm.** A
  new `ShellRequest` variant with no handler becomes a compile error.

- **Every variant gets either a real handler or an explicit no-op arm carrying
  its reason** — for example `ShellRequest::DismissSettings`, which is emitted
  only from `settings.rs:312`'s `handle_mouse` and is therefore deliberately
  inert under D16. A documented no-op arm and a wildcard are not the same
  thing: the first is a decision, the second is an accident waiting to be
  repeated.

- **The variants the compiler surfaces are triaged, not blanket-fixed.** Each
  is either wired here when the handler is small and unambiguous, or given a
  no-op arm naming the issue that owns it. `AudiobookshelfPodcastEpisodeTransition`
  is wired here — it is a live wrong-item bug, not a routing-policy question.

- **The other twelve wildcard arms in the shell dispatch are triaged in the
  same pass**: `shell.rs:237`, `shell_browser.rs:100`, `shell_feeds_manage.rs:116,128`,
  `shell_root.rs:42`, `shell_overlays_menus.rs:257,274,473`,
  `shell_tv_workspace.rs:39,46`, `shell_playlists.rs:153`, `shell_home.rs:52`.
  Each is either made exhaustive or kept with a comment stating what closed set
  it matches and why the wildcard is genuinely unreachable. Only arms over a
  request/intent enum are in scope.

## Capabilities

Updates the existing `interactive-component-framework` capability with one
rule: a component's typed request must have a shell handler or an explicit
documented no-op, enforced by exhaustive matching rather than by convention.
No new capability.

## Out of Scope

- **The routing-policy gaps themselves** (#627) — keys that never reach a
  component, catch-all swallows shadowing specific bindings, policy entries
  producing no command. This change makes missing *handlers* visible; it does
  not repair *resolution*.
- **Mouse paths** — accepted-broken per D16. Mouse-only variants get documented
  no-op arms, not handlers.
- **Painter and ownership defects** (#625, #626) — a different family entirely.
- Per-variant coverage inside the intent sub-enums (`ContextMenuIntent`,
  `FeedsManageIntent`, `SettingsIntent`, `QueueIntent`,
  `AudiobookshelfBookIntent`, …). Making the top level exhaustive is expected
  to make a second sweep unnecessary; if the sub-enums prove to hide the same
  pattern, they get their own follow-up rather than expanding this change.
