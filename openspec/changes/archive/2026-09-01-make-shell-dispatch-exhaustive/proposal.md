# Make the shell's `Msg` dispatch exhaustive

## Why

Issue #628. A component can emit a typed request that no shell handler
consumes, and nothing — not the compiler, not a test, not the running app —
says so. The component updates its own view, so the surface *looks* like it
works; only the shell-side effect is missing.

`Model::handle_terminal_message` (`src/app/shell_messages.rs:6`) is a single
flat match over `Msg` carrying **83 inline `Msg::Shell(ShellRequest::Specific)`
patterns** against the **86 `ShellRequest` variants** that exist. It never
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

The original audit read this as a live wrong-item bug via
`ShellRequest::AudiobookshelfPodcastEpisodeTransition` (emitted from five sites
in `components/audiobookshelf_podcast.rs:217-249`, matched **only in tests**).
Verification against the branch showed that premise is stale: commit
`0227d748` (migration task 5.3d.11 U2) already deleted the App-side episode
mirror and this variant's routing, and `AudiobookshelfPodcastComponent` now
solely owns `episode_selection` / `episode_filter`. The
`AudiobookshelfPodcastEpisodeIntent` handler
(`shell_audiobookshelf_podcast.rs:26-29`) resolves its target from the
component, not App state, and two regression tests lock that in
(`shell_audiobookshelf_podcast_tests.rs:103,187`). The Transition `Msg` is
genuinely inert *and correct* — the component is self-sufficient. So this
change surfaces **three** fall-through variants, and all three resolve to
documented no-op arms rather than new handlers; the value is the compile-time
guarantee and the `#627` worklist, not a behaviour fix.

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
  only from `SettingsComponent::handle_mouse` (`settings.rs:318`) and is therefore deliberately
  inert under D16. A documented no-op arm and a wildcard are not the same
  thing: the first is a decision, the second is an accident waiting to be
  repeated.

- **The variants the compiler surfaces are triaged, not blanket-fixed.** Each
  is either wired here when the handler is small and unambiguous, or given a
  no-op arm naming the issue or precedent that owns it. On this branch all
  three fall-through variants (`AudiobookshelfPodcastEpisodeTransition`,
  `DismissSettings`, `SelectionModalRefresh`) resolve to documented no-op arms:
  the first because the component owns its selection after commit `0227d748`,
  the second because it is mouse-only under D16, the third because it is
  consumed synchronously via `handle_selection_modal_request` and never
  reaches `handle_terminal_message`.

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

## Decision: 3.2 — no ast-grep rule

**No rule is added to `rules/interactive-component-boundary/`.**

The enforcement mechanism is the exhaustive top-level `match` over `ShellRequest`
in `Model::handle_terminal_message`: a variant with no arm is a compile error.
`rustc` already gives the guarantee a lint would approximate, with zero false
positives and no fixture maintenance.

A "no `_` arm over a request/intent enum" ast-grep rule cannot be expressed
without false positives here. Section 2 established **12 legitimate `_ => {}`
arms** over `ShellRequest` in the sub-dispatchers (`shell_browser.rs`,
`shell_home.rs`, `shell_tv_workspace.rs`, `shell_playlists.rs`,
`shell_overlays_menus.rs`, and others). Each sits behind an OR-group the
exhaustive top-level match routes to it, so the wildcard is provably dead — but
it is correct and intentional. ast-grep matches syntax, not reachability; it
cannot tell a narrowed sub-dispatcher wildcard from a coverage gap, so it would
fire 12 times on correct code.

A formulation keyed only to the `handle_terminal_message` function was
considered and rejected: that function is precisely the one the compiler already
guards (no wildcard, exhaustive), so the rule would protect the one site that
does not need it while ignoring everywhere a regression could actually be
introduced. It would be pure ceremony.

The spec delta (`specs/interactive-component-framework/spec.md`) records the
requirement in prose and ties it to the compile-time check; that is the durable
artifact.
