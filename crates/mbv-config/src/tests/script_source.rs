#[cfg(test)]
use crate::{checkout_fonts_dir, checkout_scripts_entry, resolve_script_source};

// Hermetic tests for the mpv overlay script/font source resolution
// (openspec change fix-next-up-accept-and-mpv-script-source, B1-B3).
// The pure resolution cases over injected candidates live in the
// `tests/config` integration binary; these two pin the compile-time
// checkout paths that need the `cfg(test)` re-exports.

#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
fn temp_source_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "mbv-script-source-{}-{}",
        tag,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn checkout_run_resolves_the_checkout_entry_script() {
    // In this build environment the checkout's real scripts/ directory
    // exists, so the compile-time entry (<checkout>/scripts/mbv.lua,
    // derived from CARGO_MANIFEST_DIR via ../..) must win. The legacy
    // candidate is a temp path that does not exist, so no real
    // user directory is touched.
    let entry = checkout_scripts_entry();
    assert!(entry.exists(), "checkout entry {} missing", entry.display());
    assert_eq!(
        entry
            .components()
            .rev()
            .take(3)
            .map(|c| c.as_os_str().to_os_string())
            .collect::<Vec<_>>(),
        vec![
            std::ffi::OsString::from("mbv.lua"),
            std::ffi::OsString::from("scripts"),
            std::ffi::OsString::from(".."),
        ]
    );

    let legacy = temp_source_root("checkout-run").join("legacy/mbv.lua");
    let resolved = resolve_script_source(
        entry.clone(),
        PathBuf::from("/usr/share/mbv/scripts/mbv.lua"),
        legacy,
    );
    assert_eq!(resolved.chosen, entry);
    assert!(resolved.unused_legacy.is_none());
}

#[test]
fn checkout_fonts_dir_exists_in_this_checkout() {
    let fonts = checkout_fonts_dir();
    assert!(
        fonts.exists(),
        "checkout fonts dir {} missing",
        fonts.display()
    );
    assert!(fonts.ends_with("fonts"));
}
