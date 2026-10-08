use mbv_config::resolve_script_source;
use std::path::PathBuf;

// Resolution is a pure function over injected candidates; these tests
// never read a real HOME, config, or state directory.

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
