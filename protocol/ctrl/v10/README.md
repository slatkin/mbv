# ctrl protocol v10

Versioned public contract for the Client-to-Player-owner ctrl protocol. The
directory name is the `CTRL_PROTOCOL_VERSION` it pins
(`crates/mbv-ctrl/src/protocol.rs`): this directory pins version 10.

## Layout

```text
protocol/ctrl/v10/
  README.md                         # this file: update and bump rules
  schema/
    ctrl-cmd.schema.json            # client-to-owner command line (CtrlCmd)
    ctrl-event.schema.json          # owner-to-client event line (CtrlEvent)
    ctrl-hello.schema.json          # handshake message both sides exchange
  fixtures/
    commands/<Tag>[.<case>].json    # one golden CtrlCmd per variant and case
    events/<Tag>[.<case>].json      # one golden CtrlEvent per variant and case
    hello/<shape>.json              # one golden CtrlHello per negotiated shape
    shared/<Type>.<case>.json       # nested-only shapes (e.g. QueueItem kinds)
```

The schemas are generated from the wire types with `schemars` and carry the
protocol version in `$id` and title. Fixtures are pretty-printed JSON, one
wire shape per file, so a wire change reviews as a small diff. The
conformance harness that checks all of this lives in
`crates/mbv-ctrl/tests/ctrl/conformance.rs`.

## Update rules

Bump `CTRL_PROTOCOL_VERSION` only when an old peer would *misbehave*, not
when it would merely fail to understand (the full rules are the comment on
the constant in `crates/mbv-ctrl/src/protocol.rs`):

- **Additive — no bump, update this directory in place.** A new `CtrlCmd` or
  `CtrlEvent` variant, a new negotiated capability, or a `#[serde(default)]`
  field on an existing message: add the variant's fixture(s) and regenerate
  the schemas inside `protocol/ctrl/v10/`. `CTRL_PROTOCOL_VERSION` stays 10.
- **Breaking — bump, open `protocol/ctrl/v11/`.** Renaming or removing a
  field or variant, changing the meaning, units, or nullability of an
  existing field, or changing handshake order or framing. The prior
  directory stays unchanged as the record of version 10.
- **A bump requires explicit user approval.** Never bump as a ride-along
  with a refactor.

## Regenerating: bless

All corpus and schema updates go through the harness's bless mode — never by
hand-editing the generated files:

```sh
MBV_PROTOCOL_BLESS=1 cargo nextest run -p mbv-ctrl conformance
```

Bless rewrites the fixtures of every table row and the three schema
documents from the wire types in code. It never deletes anything: when a
variant is removed, delete its orphaned fixture yourself (`git rm`) and then
run bless. Review the resulting diff as part of the wire change — blessing
is a deliberate act, not a way to silence the gate.

## Fixture classes

- **Round-trip fixtures** (the default): encoding the canonical value must
  produce the fixture, decoding the fixture must re-encode to it, and the
  fixture must satisfy its root schema.
- **Decode-only fixtures** (`shared/`): tolerated legacy shapes the wire no
  longer emits — today the legacy bare `EmbyItem` queue item and the
  bare-scalar `run_identity`. Decoding must succeed; no re-encode equality,
  and no schema validation, because the schema describes emitted shapes
  only. Shared fixtures validate against a named `$defs` entry of the root
  schema that carries the type.
