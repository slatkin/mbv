//! ctrl-protocol-conformance task 1.3: the `QueueItem` JSON Schema must
//! offer all four `kind` tags, so every queue-item kind validates inside the
//! shapes that carry it (slots, snapshots, queue commands).

use super::*;

/// The `QueueItem` schema's variant branches pin the four `kind` tags —
/// `Emby`, `Feed`, `Audiobookshelf`, `AudiobookshelfBook` — one per branch.
#[test]
fn queue_item_schema_offers_all_four_kind_tags() {
    let schema = schemars::schema_for!(QueueItem);
    let value = schema.as_value();
    let branches = ["oneOf", "anyOf"]
        .iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_array))
        .expect("the QueueItem schema is a oneOf/anyOf of variant branches");
    let kind_of = |branch: &serde_json::Value| -> String {
        let kind = &branch["properties"]["kind"];
        kind["const"]
            .as_str()
            .or_else(|| kind["enum"].as_array()?.first()?.as_str())
            .expect("each variant branch pins its kind tag")
            .to_owned()
    };
    let kinds: std::collections::BTreeSet<String> = branches.iter().map(kind_of).collect();
    assert_eq!(
        kinds,
        std::collections::BTreeSet::from([
            "Emby".to_owned(),
            "Feed".to_owned(),
            "Audiobookshelf".to_owned(),
            "AudiobookshelfBook".to_owned(),
        ])
    );
}
