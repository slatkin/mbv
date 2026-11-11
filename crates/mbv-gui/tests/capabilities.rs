//! The main window's capability grants (the `gui-client` spec's ACL layer,
//! add-gui-library-panel design D2): every bridge command the window content
//! invokes is one the capability file grants, because the ACL sits between
//! the webview's `invoke` and the Tauri command. A command the manifest
//! declares but the capability omits — or one missing from `build.rs`'s app
//! manifest, which generates no permission at all — is one the webview
//! cannot call: the ACL denies it and the front end's `void`-ed promise
//! swallows the denial, so the feature dies silently.
//!
//! The list under test is derived, never written here: it comes from the
//! `tauri::generate_handler![ ... ]` invocation in `src/main.rs`, the single
//! place a command is registered. Regression for the live findings that the
//! `library` command and then the `settings` command were declared in neither
//! the manifest nor the capability, so no Library or Settings command ever
//! reached the core; a hand-written case list let the second one through.

use std::fs;

/// The capability file the main window's grants live in.
const CAPABILITIES: &str = "capabilities/default.json";

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

/// The derivation below finds commands at all, so the two guards that walk
/// the derived list cannot pass by reading an empty list.
#[test]
fn the_handler_list_names_the_bridge_commands() {
    assert!(
        !registered_commands().is_empty(),
        "no commands parsed out of src/main.rs's generate_handler! invocation"
    );
}

/// The `build.rs` app manifest declares each bridge command exactly once, so
/// tauri-build generates the `allow-$command` permission the capability file
/// grants. A command missing there has no permission to grant, and the
/// capability file naming one that does not exist fails the build.
#[test]
fn the_app_manifest_declares_every_registered_command_once() {
    let undeclared = commands_where(&registered_commands(), |command| {
        occurrences(&build_manifest(), &quoted(command)) != 1
    });
    assert!(
        undeclared.is_empty(),
        "build.rs's app manifest does not declare these commands exactly once: {undeclared:?}"
    );
}

/// The capability file grants each bridge command exactly once, as
/// `allow-<command>` with the hub's underscore-to-dash spelling.
#[test]
fn the_capability_grants_every_registered_command_once() {
    let ungranted = commands_where(&registered_commands(), |command| {
        grants_of(&permission_of(command)) != 1
    });
    assert!(
        ungranted.is_empty(),
        "the capability does not grant these commands exactly once: {ungranted:?}"
    );
}

/// The bridge commands `src/main.rs` registers with
/// `tauri::generate_handler!`: every command the webview can invoke.
fn registered_commands() -> Vec<String> {
    let text = read("src/main.rs");
    let open = "generate_handler![";
    let start = text
        .find(open)
        .expect("main.rs registers the bridge commands")
        + open.len();
    let end = text[start..].find(']').expect("the handler list closes");
    text[start..start + end]
        .split(',')
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The commands in `commands` that `is_missing` reports.
fn commands_where(commands: &[String], is_missing: impl Fn(&str) -> bool) -> Vec<String> {
    commands
        .iter()
        .filter(|command| is_missing(command))
        .cloned()
        .collect()
}

/// How many times `haystack` carries `needle`.
fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// The `build.rs` app manifest text.
fn build_manifest() -> String {
    read("build.rs")
}

/// A command as the app manifest declares it.
fn quoted(command: &str) -> String {
    format!("\"{command}\"")
}

/// The permission that grants `command`: the hub spells `_` as `-`.
fn permission_of(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

/// How many times the capability file grants `permission`.
fn grants_of(permission: &str) -> usize {
    let capability: serde_json::Value =
        serde_json::from_str(&read(CAPABILITIES)).expect("the capability file parses");
    capability["permissions"]
        .as_array()
        .expect("the capability carries a permission list")
        .iter()
        .filter(|granted| granted.as_str() == Some(permission))
        .count()
}

/// The crate-relative file's text.
fn read(relative: &str) -> String {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/" + relative;
    fs::read_to_string(&path).expect("the crate file reads")
}
