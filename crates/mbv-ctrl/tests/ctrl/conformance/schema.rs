//! Wire-root schema documents (design D2): one JSON Schema per wire root,
//! generated from the wire types with `schemars`, carrying the
//! `CTRL_PROTOCOL_VERSION` it pins in `$id` and title.

use serde_json::json;

use super::*;

/// The wire roots. `CtrlCmd` is the client-to-owner command line,
/// `CtrlEvent` the owner-to-client event line, `CtrlHello` the handshake
/// message both directions exchange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Root {
    Cmd,
    Event,
    Hello,
}

impl Root {
    /// Path of the committed schema document, relative to the protocol
    /// directory.
    pub(crate) fn file_name(self) -> &'static str {
        match self {
            Root::Cmd => "schema/ctrl-cmd.schema.json",
            Root::Event => "schema/ctrl-event.schema.json",
            Root::Hello => "schema/ctrl-hello.schema.json",
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Root::Cmd => "ctrl-cmd",
            Root::Event => "ctrl-event",
            Root::Hello => "ctrl-hello",
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Root::Cmd => "command",
            Root::Event => "event",
            Root::Hello => "hello",
        }
    }

    fn generate(self) -> schemars::Schema {
        match self {
            Root::Cmd => schemars::schema_for!(CtrlCmd),
            Root::Event => schemars::schema_for!(CtrlEvent),
            Root::Hello => schemars::schema_for!(CtrlHello),
        }
    }
}

/// Generate one root schema and stamp the protocol version into `$id` and
/// title, so a `CTRL_PROTOCOL_VERSION` change is caught by the byte-compare
/// even before any fixture moves.
fn root_document(root: Root) -> serde_json::Value {
    let mut schema = root.generate();
    let map = schema
        .as_object_mut()
        .expect("a generated root schema is a JSON object");
    let kind = root.kind();
    map.insert(
        String::from("$id"),
        json!(format!(
            "https://mbv.dev/schemas/ctrl/v{CTRL_PROTOCOL_VERSION}/{kind}.schema.json"
        )),
    );
    map.insert(
        String::from("title"),
        json!(format!(
            "mbv ctrl {} (protocol version {CTRL_PROTOCOL_VERSION})",
            root.noun()
        )),
    );
    schema.as_value().clone()
}

/// All root schema documents, regenerated from the current wire types.
pub(crate) fn documents() -> Vec<(Root, serde_json::Value)> {
    vec![
        (Root::Cmd, root_document(Root::Cmd)),
        (Root::Event, root_document(Root::Event)),
        (Root::Hello, root_document(Root::Hello)),
    ]
}

/// Regenerate one root schema on demand (fixture validation).
pub(crate) fn document(root: Root) -> serde_json::Value {
    root_document(root)
}

/// A named `$defs` entry of one root schema, as a standalone document with
/// the root's full `$defs` attached so inner `$ref`s still resolve. Shared
/// nested-only shapes (`QueueItem`, `PlayerStatus`, `UnifiedQueueStateData`)
/// have no root of their own; their fixtures validate against the entry of
/// the root that carries the type (design D4).
pub(crate) fn document_entry(root: Root, name: &str) -> serde_json::Value {
    let document = root_document(root);
    let defs = document
        .get("$defs")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{} carries no $defs", root.file_name()))
        .clone();
    let mut entry = defs
        .get(name)
        .cloned()
        .unwrap_or_else(|| panic!("{} carries no $defs entry `{name}`", root.file_name()));
    let entry_object = entry
        .as_object_mut()
        .expect("a $defs entry is a schema object");
    entry_object.insert(String::from("$defs"), serde_json::Value::Object(defs));
    entry
}

/// Deterministic on-disk form: pretty-printed JSON with a trailing newline.
/// `serde_json` sorts object keys (no `preserve_order` in this workspace), so
/// the bytes are stable across runs.
pub(crate) fn serialize(document: &serde_json::Value) -> String {
    let mut text = serde_json::to_string_pretty(document).expect("a generated schema serializes");
    text.push('\n');
    text
}
