use super::*;
use std::collections::HashMap;

mod catalog;
mod playback;

#[test]
fn validated_setup_debug_redacts_api_key() {
    let setup = AudiobookshelfValidatedSetup::new(
        crate::config::AudiobookshelfSetup::new("http://abs:13378"),
        AudiobookshelfUser {
            id: "user-id".to_string(),
            username: "user".to_string(),
        },
        "abs-secret-key".to_string(),
    );
    let rendered = format!("{:?}", setup);
    assert!(rendered.contains("AudiobookshelfValidatedSetup"));
    assert!(!rendered.contains("abs-secret-key"));
}
