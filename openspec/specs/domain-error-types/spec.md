# domain-error-types Specification

## Purpose
Failures produced anywhere in the workspace carry a matchable kind and a
preserved cause instead of an opaque string, so callers can react to the kind
of failure (retry, exit code, user message) and no diagnostic information is
lost in conversion. Tracks https://github.com/slatkin/mbv/issues/846.

## Requirements

### Requirement: Domain crates expose canonical error types

Each crate whose public API can fail SHALL expose one canonical error struct
per failure domain (`ConfigError`, `EmbyError`, `CastError`, `FeedError`,
`CtrlError`, `PlayerError`, `QueueError` already clean, `DaemonError`,
and equivalents for `mbv-core`, `mbv-remote-player`, `mbv-ui-model`,
`mbv-visualizer`, `mbv-components`, `mbvd`), shaped as a struct wrapping a
kind enum per `M-ERRORS-CANONICAL-STRUCTS`. The kind enum SHALL NOT be
matched directly by other crates; each error type SHALL expose `is_*()`
predicates for the kinds callers decide on, and a `kind_name()` returning a
stable crate-qualified dotted name for each kind (the log `error.type`
value). Every error type SHALL implement
`Display` (one summary sentence), `std::error::Error` with the upstream cause
available via `source()`, and `From` for each upstream error it converts
(`M-FROM-ERROR`). Every error type exposed by a crate's public API SHALL carry
a `Backtrace` captured at construction with `Backtrace::capture()` and expose
it through `backtrace()`. Because a backtrace is neither clonable nor
comparable, error types SHALL NOT derive `Clone`, `PartialEq`, or `Eq`;
callers that need to distinguish failures read `kind_name()` or an `is_*()`
predicate instead of comparing error values. No crate SHALL introduce
`anyhow`, `thiserror`, or `eyre`. This supersedes the archived
`2026-09-28-replace-stringly-typed-errors` non-goal that excluded backtrace
fields.

#### Scenario: A failure carries where it came from

- **WHEN** a domain error is constructed
- **THEN** it holds the backtrace captured at that construction site,
  retrievable through `backtrace()`, and it pays no stack-walk cost unless
  `RUST_BACKTRACE` asks for one

#### Scenario: Caller distinguishes failure kinds without parsing text

- **WHEN** a caller receives an error from a domain crate API
- **THEN** it can determine the failure kind through a typed predicate or
  method, without inspecting the rendered message

#### Scenario: Adding a failure kind does not break downstream crates

- **WHEN** a crate adds a new variant to its private kind enum
- **THEN** no downstream crate fails to compile, because matching happens
  only through the owning crate's predicates

#### Scenario: Upstream cause survives conversion

- **WHEN** a domain error wraps an I/O, protocol, or parse failure
- **THEN** `std::error::Error::source()` on the domain error yields the
  underlying cause

### Requirement: Crate public APIs do not return stringly-typed errors

No public function in any workspace crate SHALL return `Result<_, String>`
or `Box<dyn Error>`; fallible public APIs SHALL return the owning crate's
domain error type. `map_err` with `.to_string()` SHALL NOT appear in crate
sources; `map_err` remains only for foreign errors that need added context
at the boundary. Audit: `rg 'Result<.*, String>'` over crate and `src/`
sources returns zero matches.

#### Scenario: Workspace audit finds no stringly-typed signatures

- **WHEN** the change is complete
- **THEN** `rg 'Result<.*, String>' -g '*.rs' crates/ src/` returns zero
  matches and `rg 'map_err' -g '*.rs' crates/ src/` shows only
  context-adding conversions of foreign errors

### Requirement: Daemon exit codes derive from error kinds

`mbvd` SHALL map its domain error kinds to process exit codes: restart
required → 3, usage errors (non-interactive terminal, unsupported service,
bad selectors, unknown argument, missing service, log-level errors) → 2,
all other failures → 1. The mapping SHALL be computed from error-kind
predicates, never by substring-matching the rendered message.

#### Scenario: Restart-required failure exits 3

- **WHEN** `mbvd` fails with a restart-required error
- **THEN** the process prints the error and exits with code 3

#### Scenario: Usage failure exits 2

- **WHEN** `mbvd` fails with any usage-category error (non-interactive
  terminal, unsupported service, bad selectors, unknown argument, missing
  service, log-level error)
- **THEN** the process prints the error and exits with code 2

#### Scenario: Ordinary failure exits 1

- **WHEN** `mbvd` fails with any error outside the restart and usage kinds
- **THEN** the process prints the error and exits with code 1

### Requirement: User-visible failure text and behavior are preserved

Replacing a `String` error with a domain error SHALL NOT change which
operations fail, what text the user sees for a given failure, or which exit
code results, except where a lossy conversion previously destroyed
information (the former `From<String> for AudiobookshelfError`, which
collapsed every failure to connectivity, SHALL classify each site with its
real kind).

#### Scenario: Same failure, same message

- **WHEN** an operation that previously failed with a given message fails
  after conversion
- **THEN** the user sees the same message and the process exits with the
  same code as before
