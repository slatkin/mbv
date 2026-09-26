# Rust coding standards

mbv follows the Microsoft Pragmatic Rust Guidelines, with one file per rule
under `rules/`. The rule text is copied unedited from the original and is
MIT-licensed, © Microsoft Corporation.

* Source: https://microsoft.github.io/rust-guidelines/agents/all.txt
* Fetched: 2026-09-26. Refresh by hand. Don't edit rule files, re-fetch them instead.

## How to use this

Scan the checklist below. For any rule a change touches, get the full text:

```sh
qmd query "error type shape"          # by concept
qmd search "M-ERRORS-CANONICAL-STRUCTS" # by ID
```

or read `rules/<ID>.md` directly. Nobody needs to read every rule.

**AGENTS.md wins on conflict.** Specifically:

* M-APP-ERROR allows anyhow. mbv forbids anyhow, thiserror and eyre and uses
  custom domain error types.
* M-LINT-OVERRIDE-EXPECT allows `#[expect]`. mbv allows no lint suppression
  of any kind without per-instance user approval.

## Checklist (applies to mbv)

Library rules also apply to mbv's internal crates (`mbv-core`, and future
splits, see #814).

**Universal**

* M-UPSTREAM-GUIDELINES: follow the upstream Rust guidelines
* M-STATIC-VERIFICATION: use static verification
* M-SMALLER-CRATES: if in doubt, split the crate
* M-SHORT-NAMES: item names are short
* M-WEASEL-WORDS: names are free of weasel words
* M-REGULAR-FN: prefer regular functions over associated functions
* M-PUBLIC-DEBUG: public types are `Debug`
* M-PUBLIC-DISPLAY: public types meant to be read are `Display`
* M-DOCUMENTED-MAGIC: magic values are documented
* M-LOG-STRUCTURED: structured logging with message templates

**AI**

* M-DESIGN-FOR-AI: design with AI use in mind
* M-RUST-SHAPED: Rust code solves Rust problems
* M-SINGLE-ITEM-PATH: items are visible through only one path
* M-TAUTOLOGICAL-TESTS: tests assert ground truth, not the code's own output
* M-NO-META-DESIGN-DOCUMENTATION: avoid meta design documentation

**Application**

* M-MIMALLOC-APPS: use mimalloc
* M-TARGET-CPU: target the highest viable target-cpu

**Correctness**

* M-PANIC-IS-STOP: panic means "stop the program"
* M-PANIC-ON-BUG: detected programming bugs are panics, not errors
* M-PANIC-CONTINUATION: panic continuation is a last resort
* M-PANIC-MESSAGE: custom panics have a helpful message
* M-UNSAFE: unsafe needs a reason and should be avoided
* M-UNSAFE-IMPLIES-UB: unsafe implies undefined behaviour
* M-UNSOUND: all code must be sound

**Documentation**

* M-CANONICAL-DOCS: canonical doc sections (Errors, Panics, Safety, …)
* M-FIRST-DOC-SENTENCE: first sentence is one line, about 15 words
* M-MODULE-DOCS: comprehensive module docs
* M-DOC-INLINE: mark `pub use` items `#[doc(inline)]`

**Library UX**

* M-ERRORS-CANONICAL-STRUCTS: errors are canonical structs
* M-FROM-ERROR: canonical error conversion uses `From`, not `map_err`
* M-ESSENTIAL-FN-INHERENT: essential functionality is inherent
* M-DI-HIERARCHY: prefer concrete types, then generics, then `dyn` traits
* M-AVOID-WRAPPERS: no smart pointers or wrappers in APIs
* M-SIMPLE-ABSTRACTIONS: abstractions don't visibly nest
* M-BALANCED-MODULES: modules are balanced in size and scope
* M-PARAMETER-CONSISTENCY: consistent parameter ordering
* M-INIT-BUILDER: complex construction has builders
* M-INIT-CASCADED: complex initialization hierarchies are cascaded
* M-SERVICES-CLONE: services are `Clone`
* M-COLLECTION-TRAITS: collections implement the right iterator traits
* M-NO-PRELUDE: don't define preludes

**Library resilience**

* M-NO-GLOB-REEXPORTS: don't glob re-export items
* M-AVOID-STATICS: avoid statics
* M-STRONG-TYPES: use the proper type family
* M-STRONG-TYPES-GUARD: newtypes guard their invariants
* M-MOCKABLE-SYSCALLS: I/O and system calls are mockable
* M-TEST-UTIL: test utilities are feature-gated
* M-INTEGRATION-TESTS: integration tests live under `tests/`
* M-LOG-NOT-PRINT: production code logs, it doesn't `println!`
* M-BUILD-RESULT: builders validate in the final `.build()`

**Library interop and building**

* M-DONT-LEAK-TYPES: don't leak external types
* M-FOREIGN-REEXPORTS: items come from their original crate
* M-ESCAPE-HATCHES: native escape hatches
* M-IMPL-ASREF: accept `impl AsRef<>` where feasible
* M-IMPL-IO: accept `impl Read/Write` where feasible ("sans IO")
* M-IMPL-RANGEBOUNDS: accept `impl RangeBounds<>` where feasible
* M-TYPES-SEND: types are `Send`
* M-FEATURES-ADDITIVE: features are additive
* M-OOBE: libraries work out of the box

**Performance**

* M-HOTPATH: identify, profile and optimize the hot path early
* M-THROUGHPUT: optimize for throughput, avoid empty cycles
* M-INITIAL-CAPACITY: create collections with enough initial capacity
* M-MEM-REUSE: reuse allocations
* M-SHRINK-TO-FIT: shrink collections after building
* M-BOX-DST: boxed slices and strings for immutable owned sequences
* M-FAST-HASHER: use a fast hasher where possible
* M-AVOID-INDIRECTION: avoid needless indirection in nested types
* M-LOG-OVERHEAD: telemetry doesn't tank performance

**Project**

* M-CARGO-WORKSPACE: common settings come from the workspace `Cargo.toml`
* M-CRATES-IN-WORKSPACE: the workspace lists and versions all crates
* M-CRATES-FLAT-FOLDER: all crates are siblings in one folder
* M-LATEST-EDITION: new crates target the latest edition
* M-MSRV: MSRV is updated conservatively

## Not applicable

Their rule files are kept for completeness.

* FFI (mbv has no FFI surface): M-FFI-NAMING, M-FFI-TRANSLATES, M-ISOLATE-DLL-STATE, M-SYS-CRATES
* Async (mbv is sync-first, and tokio is edge-only):
  M-ASYNC-FN, M-ASYNC-STACK-SIZE, M-YIELD-POINTS
* Macros (read these only if you're writing a macro): M-MACRO-LAST-RESORT,
  M-EXAMPLE-OVER-PROC, M-MACROS-DONT-LIE, M-MACRO-MAIN-CRATE, M-MACRO-HELPERS,
  M-MACRO-VERSION-PIN, M-PROC-IMPL, M-PROC-IMPLIED-ITEMS
* Superseded by AGENTS.md: M-APP-ERROR, M-LINT-OVERRIDE-EXPECT
