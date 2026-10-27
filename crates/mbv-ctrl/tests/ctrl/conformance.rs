//! Ctrl protocol conformance gate (design D5 of `ctrl-protocol-conformance`).
//!
//! The corpus under `protocol/ctrl/v10/fixtures/` and the schema documents
//! under `protocol/ctrl/v10/schema/` are committed artifacts. Every check
//! here compares the wire types in code against those artifacts, so wire
//! drift fails CI until the corpus is regenerated deliberately with
//! `MBV_PROTOCOL_BLESS=1 cargo nextest run -p mbv-ctrl conformance`.
//!
//! The corpus is one fixture per wire shape, seeded per corpus by the units
//! of the change: `hello/` (handshake shapes), `commands/` (one `CtrlCmd`
//! per variant, `PlayerCmd` per `WireCommand` tag), `events/` (one
//! `CtrlEvent` per variant, `PlayerEvent` per `Player.<variant>` row), and
//! `shared/` (one `QueueItem` per kind, `PlayerStatus`, and
//! `UnifiedQueueStateData` with its transition fields, plus the tolerated
//! legacy decode shapes). A fixture directory that holds a file the
//! table does not name — or the reverse — fails reconciliation.

mod builders;
mod commands;
mod events;
mod hello;
mod schema;
mod shared;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Serialize, de::DeserializeOwned};

use super::*;

/// Root of the versioned protocol directory this harness pins.
fn protocol_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocol/ctrl/v10")
}

fn fixtures_dir() -> PathBuf {
    protocol_dir().join("fixtures")
}

/// Whether the harness runs in bless mode: rewrite the committed fixtures
/// and schemas from the code instead of failing on drift. The single
/// deliberate-update path; the resulting diff is reviewed as part of the
/// wire change.
fn bless() -> bool {
    std::env::var_os("MBV_PROTOCOL_BLESS").is_some_and(|value| value == "1")
}

/// Which direction a fixture pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    /// The wire emits this shape: encoding the canonical value must produce
    /// the fixture, and decoding the fixture must re-encode to it.
    RoundTrip,
    /// A tolerated legacy shape the wire never emits any more: decoding the
    /// fixture must yield exactly the canonical value the same shape encodes
    /// to (the legacy bytes themselves differ from that encoding — that is
    /// what makes them legacy).
    DecodeOnly,
}

/// The schema document a fixture validates against. A wire root is its own
/// document; a shared nested-only shape has no root of its own and
/// validates against a named `$defs` entry of the root schema that carries
/// the type (design D4).
enum SchemaAnchor {
    Root(schema::Root),
    DefsEntry {
        root: schema::Root,
        name: &'static str,
    },
}

impl SchemaAnchor {
    /// The schema document this anchor resolves to.
    fn document(self) -> serde_json::Value {
        match self {
            SchemaAnchor::Root(root) => schema::document(root),
            SchemaAnchor::DefsEntry { root, name } => schema::document_entry(root, name),
        }
    }
}

/// One row of the fixture table: a fixture path, its direction, the root
/// schema it must validate against (`None` for decode-only rows — the schema
/// describes emitted shapes only), and the canonical value.
struct FixtureRow {
    path: String,
    direction: Direction,
    schema: Option<SchemaAnchor>,
    value: Box<dyn FixtureValue>,
}

/// Type-erased canonical wire value. The concrete wire type is known only to
/// the row builder; the harness encodes it and decodes files back into it.
trait FixtureValue {
    /// The canonical wire JSON this row pins.
    fn encode(&self) -> serde_json::Value;

    /// Decode `file` into the concrete wire type and re-encode the decoded
    /// value, so a field the decoder silently drops fails the row.
    fn reencode_from_file(&self, file: &serde_json::Value) -> Result<serde_json::Value, String>;
}

impl<T> FixtureValue for T
where
    T: Serialize + DeserializeOwned,
{
    fn encode(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("ctrl wire type serializes")
    }

    fn reencode_from_file(&self, file: &serde_json::Value) -> Result<serde_json::Value, String> {
        let decoded = T::deserialize(file).map_err(|error| format!("decode failed: {error}"))?;
        serde_json::to_value(&decoded).map_err(|error| format!("re-encode failed: {error}"))
    }
}

/// Wrap a concrete wire value as a table-row value.
fn fixture_value<T>(value: T) -> Box<dyn FixtureValue>
where
    T: Serialize + DeserializeOwned + 'static,
{
    Box::new(value)
}

/// Every fixture row of the corpus. Later units of the change append their
/// corpora here (`commands::rows`, `events::rows`, `shared::rows`).
fn rows() -> Vec<FixtureRow> {
    let mut rows = Vec::new();
    hello::rows(&mut rows);
    commands::rows(&mut rows);
    events::rows(&mut rows);
    shared::rows(&mut rows);
    rows
}

fn read_fixture_json(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

/// Pretty-printed JSON with a trailing newline, so fixture and schema diffs
/// stay reviewable (design D4).
fn write_fixture_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    text.push('\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

/// Check one row against its fixture file. In bless mode a round-trip row
/// rewrites its fixture from the canonical value; decode-only fixtures are
/// tolerated legacy shapes and are never rewritten.
fn check_row(row: &FixtureRow) -> Result<(), String> {
    let path = fixtures_dir().join(&row.path);
    match row.direction {
        Direction::RoundTrip => {
            let encoded = row.value.encode();
            if bless() {
                write_fixture_json(&path, &encoded)?;
                return Ok(());
            }
            let file = read_fixture_json(&path)?;
            if file != encoded {
                return Err(format!(
                    "{}: encoding the canonical value no longer produces the fixture \
                     — regenerate deliberately with MBV_PROTOCOL_BLESS=1",
                    row.path
                ));
            }
            let reencoded = row.value.reencode_from_file(&file)?;
            if reencoded != file {
                return Err(format!(
                    "{}: decoding the fixture and re-encoding it changes the payload \
                     — a field is dropped or altered on decode",
                    row.path
                ));
            }
        }
        Direction::DecodeOnly => {
            let file = read_fixture_json(&path)?;
            let reencoded = row.value.reencode_from_file(&file)?;
            if reencoded != row.value.encode() {
                return Err(format!(
                    "{}: decoding the legacy shape does not yield the canonical value \
                     — a field is dropped or altered on decode",
                    row.path
                ));
            }
        }
    }
    Ok(())
}

/// Every table row conforms: encode produces the fixture bytes and decode
/// round-trips them (round-trip rows), or the legacy shape decodes
/// (decode-only rows).
#[test]
fn every_fixture_row_conforms() {
    for row in rows() {
        check_row(&row).unwrap_or_else(|error| panic!("fixture {} failed: {error}", row.path));
    }
}

/// The fixture table and the on-disk corpus name exactly the same files, so
/// a fixture with no wire shape (orphan) and a wire shape with no fixture
/// (missing) both fail.
#[test]
fn fixture_table_reconciles_with_the_corpus_directory() {
    let mut on_disk = BTreeSet::new();
    if fixtures_dir().is_dir() {
        collect_fixture_files(&fixtures_dir(), &mut Vec::new(), &mut on_disk);
    }
    let table: BTreeSet<String> = rows().into_iter().map(|row| row.path).collect();

    let orphans: Vec<&String> = on_disk.difference(&table).collect();
    assert!(
        orphans.is_empty(),
        "orphaned fixture files with no table row (remove them with `git rm`; \
         bless never deletes): {orphans:?}"
    );
    // In bless mode the missing files are written by the round-trip pass of
    // the same run, so only orphans fail here.
    if bless() {
        return;
    }
    let missing: Vec<&String> = table.difference(&on_disk).collect();
    assert!(
        missing.is_empty(),
        "table rows with no fixture file (run MBV_PROTOCOL_BLESS=1 to seed them): {missing:?}"
    );
}

fn collect_fixture_files(dir: &Path, prefix: &mut Vec<String>, out: &mut BTreeSet<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read fixture directory {}: {error}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("fixture directory entry: {error}"));
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            prefix.push(name);
            collect_fixture_files(&path, prefix, out);
            prefix.pop();
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let mut parts = prefix.clone();
            parts.push(name);
            out.insert(parts.join("/"));
        }
    }
}

/// Regenerating every wire-root schema must reproduce the committed documents
/// byte-for-byte, and each document must carry the `CTRL_PROTOCOL_VERSION` it
/// pins — a version bump without regeneration fails here.
#[test]
fn committed_schemas_are_fresh_and_carry_the_protocol_version() {
    for (root, document) in schema::documents() {
        schema::assert_version_bound(&document, root);
        let generated = schema::serialize(&document);
        let path = protocol_dir().join(root.file_name());
        if bless() {
            write_schema_if_changed(&path, &generated);
        } else {
            let committed = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(
                committed,
                generated,
                "{} disagrees with the regenerated schema — run \
                 MBV_PROTOCOL_BLESS=1 cargo nextest run -p mbv-ctrl conformance",
                root.file_name()
            );
        }
    }
}

fn write_schema_if_changed(path: &Path, generated: &str) {
    match fs::read_to_string(path) {
        Ok(committed) if committed == generated => {}
        _ => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
            }
            fs::write(path, generated)
                .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        }
    }
}

/// Every round-trip fixture must satisfy the schema its row anchors to — a
/// wire root document, or the named `$defs` entry a shared shape validates
/// against — so a `#[schemars(with = ...)]` override that no longer matches
/// what serde emits cannot pass silently. Decode-only rows skip validation:
/// the schema describes emitted shapes only.
#[test]
fn every_round_trip_fixture_validates_against_its_root_schema() {
    for row in rows() {
        let Some(anchor) = row.schema else {
            assert_eq!(
                row.direction,
                Direction::DecodeOnly,
                "{}: a round-trip row needs a schema anchor",
                row.path
            );
            continue;
        };
        let document = anchor.document();
        let instance = if bless() {
            // The fixture file may not exist yet in a seeding bless run; the
            // canonical value is exactly what the round-trip pass writes.
            row.value.encode()
        } else {
            read_fixture_json(&fixtures_dir().join(&row.path))
                .unwrap_or_else(|error| panic!("fixture {}: {error}", row.path))
        };
        jsonschema::validate(&document, &instance)
            .map_err(|error| format!("{}: {error}", row.path))
            .unwrap_or_else(|error| panic!("schema validation failed: {error}"));
    }
}
