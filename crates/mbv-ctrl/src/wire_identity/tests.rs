use rstest::rstest;
use serde_json::json;

use super::*;

#[rstest]
#[case::root_description_is_removed(
    json!({"type": "object", "description": "A command."}),
    json!({"type": "object"}),
)]
#[case::title_keyword_is_removed(
    json!({"type": "string", "title": "Name"}),
    json!({"type": "string"}),
)]
#[case::property_named_title_is_kept(
    json!({"type": "object", "properties": {"title": {"type": "string", "description": "A title."}}}),
    json!({"type": "object", "properties": {"title": {"type": "string"}}}),
)]
#[case::definition_named_description_is_kept(
    json!({"$defs": {"description": {"type": "string", "title": "Text"}}}),
    json!({"$defs": {"description": {"type": "string"}}}),
)]
fn normalization_removes_annotations_and_keeps_names(
    #[case] input: Value,
    #[case] expected: Value,
) {
    let mut value = input;
    normalize_schema(&mut value);
    assert_eq!(value, expected);
}

#[test]
fn annotation_only_difference_keeps_the_digest() {
    let plain = json!({"type": "object", "properties": {"id": {"type": "string"}}});
    let documented = json!({
        "title": "Thing",
        "description": "A documented thing.",
        "type": "object",
        "properties": {"id": {"type": "string", "description": "The id."}}
    });
    assert_eq!(digest_of(vec![plain]), digest_of(vec![documented]));
}
