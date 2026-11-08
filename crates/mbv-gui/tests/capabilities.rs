//! The main window's capability grants (the `gui-client` spec's ACL layer,
//! add-gui-library-panel design D2): every bridge command the window content
//! invokes is one the capability file grants, because the ACL sits between
//! the webview's `invoke` and the Tauri command. A command the manifest
//! declares but the capability omits — or one missing from `build.rs`'s app
//! manifest, which generates no permission at all — is one the webview
//! cannot call: the ACL denies it and the front end's `void`-ed promise
//! swallows the denial, so the feature dies silently. Regression for the
//! live finding that the `library` command was declared in neither file, so
//! no Library command ever reached the core.

use std::fs;

use rstest::rstest;

/// The capability file the main window's grants live in.
const CAPABILITIES: &str = "capabilities/default.json";

/// The identifier that grants one bridge command: the three input bridges
/// (design D4, D11) and the Library panel's `library` command
/// (add-gui-library-panel design D2).
#[rstest]
#[case("key", "allow-key")]
#[case("pointer", "allow-pointer")]
#[case("report_layout", "allow-report-layout")]
#[case("library", "allow-library")]
fn the_capability_grants_the_bridge_command(#[case] command: &str, #[case] permission: &str) {
    assert_eq!(
        grants_of(permission),
        1,
        "the capability does not grant {permission}; the webview cannot invoke {command}"
    );
}

/// The frame event the core thread emits; the window's `listen` needs its
/// grant (design D4).
#[test]
fn the_capability_grants_the_frame_event_listener() {
    assert_eq!(
        grants_of("core:event:allow-listen"),
        1,
        "the capability does not grant core:event:allow-listen"
    );
}

/// The `build.rs` app manifest declares the bridge command, so tauri-build
/// generates the `allow-$command` permission the capability file grants. A
/// command missing there has no permission to grant, and the capability file
/// naming one that does not exist fails the build.
#[rstest]
#[case("key")]
#[case("pointer")]
#[case("report_layout")]
#[case("library")]
fn the_app_manifest_declares_the_bridge_command(#[case] command: &str) {
    let text =
        fs::read_to_string(env!("CARGO_MANIFEST_DIR").to_string() + "/build.rs").expect("build.rs");
    assert_eq!(
        text.matches(&format!("\"{command}\"")).count(),
        1,
        "build.rs's app manifest does not declare {command} once"
    );
}

/// How many times the capability file grants `permission`.
fn grants_of(permission: &str) -> usize {
    let text = fs::read_to_string(env!("CARGO_MANIFEST_DIR").to_string() + "/" + CAPABILITIES)
        .expect("the capability file exists");
    let capability: serde_json::Value =
        serde_json::from_str(&text).expect("the capability file parses");
    capability["permissions"]
        .as_array()
        .expect("the capability carries a permission list")
        .iter()
        .filter(|granted| granted.as_str() == Some(permission))
        .count()
}
