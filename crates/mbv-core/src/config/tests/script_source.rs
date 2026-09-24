use super::*;

// Hermetic tests for the mpv overlay script/font source resolution
// (openspec change fix-next-up-accept-and-mpv-script-source, B1-B3).
// Resolution is a pure function over injected candidates; these tests
// never read a real HOME, config, or state directory.

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
fn script_source_prefers_checkout_when_present() {
    let root = temp_source_root("checkout");
    let checkout = root.join("scripts/mbv.lua");
    std::fs::create_dir_all(checkout.parent().unwrap()).unwrap();
    std::fs::write(&checkout, b"--").unwrap();
    let package = root.join("package/mbv.lua");
    let legacy = root.join("legacy/mbv.lua");

    let resolved = resolve_script_source(checkout.clone(), package, legacy);

    assert_eq!(resolved.chosen, checkout);
    assert!(resolved.unused_legacy.is_none());
}

#[test]
fn script_source_falls_back_to_package_without_checkout() {
    let root = temp_source_root("package");
    let checkout = root.join("scripts/mbv.lua"); // never created
    let package = root.join("package/mbv.lua");
    std::fs::create_dir_all(package.parent().unwrap()).unwrap();
    std::fs::write(&package, b"--").unwrap();
    let legacy = root.join("legacy/mbv.lua");

    let resolved = resolve_script_source(checkout, package.clone(), legacy);

    assert_eq!(resolved.chosen, package);
    assert!(resolved.unused_legacy.is_none());
}

#[test]
fn script_source_ignores_but_reports_legacy_copy() {
    let root = temp_source_root("legacy");
    let checkout = root.join("scripts/mbv.lua");
    std::fs::create_dir_all(checkout.parent().unwrap()).unwrap();
    std::fs::write(&checkout, b"--").unwrap();
    let package = root.join("package/mbv.lua");
    let legacy = root.join("legacy/mbv.lua");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, b"-- stale installer copy --").unwrap();

    let resolved = resolve_script_source(checkout.clone(), package, legacy.clone());

    assert_eq!(resolved.chosen, checkout);
    assert_eq!(resolved.unused_legacy, Some(legacy));
    assert_ne!(resolved.chosen, resolved.unused_legacy.unwrap());
}

#[test]
fn script_source_without_legacy_reports_none() {
    let root = temp_source_root("no-legacy");
    let checkout = root.join("scripts/mbv.lua");
    let package = root.join("package/mbv.lua");
    std::fs::create_dir_all(package.parent().unwrap()).unwrap();
    std::fs::write(&package, b"--").unwrap();
    let legacy = root.join("legacy/mbv.lua"); // absent

    let resolved = resolve_script_source(checkout, package, legacy);

    assert!(resolved.unused_legacy.is_none());
}

#[test]
fn font_source_mirrors_script_cases() {
    // Fonts resolve under the same rule: checkout dir when present,
    // else package; legacy installer dir reported, never used.
    let root = temp_source_root("fonts");
    let checkout = root.join("fonts");
    std::fs::create_dir_all(&checkout).unwrap();
    let package = root.join("package-fonts");
    let legacy = root.join("legacy-fonts");
    std::fs::create_dir_all(&legacy).unwrap();

    let resolved = resolve_script_source(checkout.clone(), package.clone(), legacy.clone());
    assert_eq!(resolved.chosen, checkout);
    assert_eq!(resolved.unused_legacy, Some(legacy.clone()));

    // Checkout absent -> packaged font directory wins.
    std::fs::remove_dir_all(&checkout).unwrap();
    let resolved = resolve_script_source(checkout, package.clone(), legacy);
    assert_eq!(resolved.chosen, package);
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
