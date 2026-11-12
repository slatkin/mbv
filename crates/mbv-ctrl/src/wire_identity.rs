//! Wire identity: a short deterministic digest of the ctrl wire shapes
//! (`ctrl-protocol` delta, design D1). Two binaries with the same wire report
//! the same value, whatever their build identity. Documentation text does not
//! change it, and neither does the protocol version, which keeps its own check.

use std::sync::LazyLock;

use serde_json::Value;

use crate::{CtrlCmd, CtrlEvent, CtrlHello};

/// Annotation keywords that carry documentation or tooling text, not wire shape.
const ANNOTATION_KEYWORDS: [&str; 6] = [
    "description",
    "title",
    "examples",
    "$comment",
    "$id",
    "$schema",
];

/// Keywords whose value maps names to schemas. Their keys are property or
/// definition names, never keywords, so a property named `title` is kept.
const SCHEMA_MAP_KEYWORDS: [&str; 5] = [
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
];

/// Keywords whose value is instance data, not a schema: never normalized.
const INSTANCE_KEYWORDS: [&str; 3] = ["default", "const", "enum"];

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

static WIRE_ID: LazyLock<String> = LazyLock::new(|| {
    digest_of(vec![
        schema_value(&schemars::schema_for!(CtrlCmd)),
        schema_value(&schemars::schema_for!(CtrlEvent)),
        schema_value(&schemars::schema_for!(CtrlHello)),
    ])
});

/// The wire identity of this binary: 16 lowercase hex digits.
#[must_use]
pub fn wire_id() -> &'static str {
    WIRE_ID.as_str()
}

fn schema_value(schema: &schemars::Schema) -> Value {
    schema.as_value().clone()
}

/// FNV-1a over each normalized schema's bytes, each followed by a `0x00`
/// separator. The fixed algorithm keeps the value stable across toolchains,
/// unlike `std::hash::DefaultHasher`.
fn digest_of(schemas: Vec<Value>) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for mut schema in schemas {
        normalize_schema(&mut schema);
        let bytes = serde_json::to_vec(&schema).expect("a JSON value serializes");
        hash = fnv1a(hash, &bytes);
        hash = fnv1a(hash, &[0]);
    }
    format!("{hash:016x}")
}

fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

/// Removes annotation keywords from every schema object in `value`, keeping
/// names in the schema-map keywords and skipping instance data.
fn normalize_schema(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|keyword, _| !ANNOTATION_KEYWORDS.contains(&keyword.as_str()));
            for (keyword, child) in map.iter_mut() {
                if SCHEMA_MAP_KEYWORDS.contains(&keyword.as_str()) {
                    normalize_schema_map(child);
                } else if !INSTANCE_KEYWORDS.contains(&keyword.as_str()) {
                    normalize_schema(child);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalize_schema),
        _ => {}
    }
}

/// Normalizes each schema under a name-to-schema map; the names stay as they are.
fn normalize_schema_map(value: &mut Value) {
    if let Value::Object(entries) = value {
        entries.values_mut().for_each(normalize_schema);
    }
}

#[cfg(test)]
mod tests;
