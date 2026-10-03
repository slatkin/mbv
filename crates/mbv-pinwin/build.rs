//! Build the pinned pinwin Zig dependency and link the panel into this crate.
//!
//! Zig installs its artifacts under `OUT_DIR`, never into the source tree
//! (`--prefix`/`--cache-dir`); only Zig's own `zig-pkg/` dependency extraction
//! stays beside this crate's `build.zig`, as it does for `zig build` run by hand.
//! Link order
//! matters: the static archive of `pinwin` precedes `ghostty-vt-static` (which it
//! references) and the GTK libraries, and `gtk4-layer-shell` is linked before
//! the other GTK libraries because it only shims `libwayland-client` when the
//! dynamic linker loads it first.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo always sets CARGO_MANIFEST_DIR"),
    );
    let out_dir =
        PathBuf::from(env::var("OUT_DIR").expect("cargo always sets OUT_DIR for a build script"));

    for name in ["build.zig", "build.zig.zon"] {
        let path = manifest_dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let prefix = out_dir.join("pinwin");
    build_pinwin(&manifest_dir, &prefix, &out_dir.join("zig-cache"));

    let lib_dir = prefix.join("lib");
    strip_syslib_refs(&lib_dir);
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Static archives first, in dependency order: pinwin references
    // ghostty-vt, ghostty-vt references the two vendored SIMD archives. Zig
    // installs all four beside each other (this crate's build.zig).
    for lib in ["pinwin", "ghostty-vt-static", "simdutf", "highway"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    // gtk4-layer-shell first among the GTK libraries (design, Risks).
    for package in ["gtk4-layer-shell-0", "gtk4", "pangocairo"] {
        pkg_config::Config::new()
            .probe(package)
            .unwrap_or_else(|error| panic!("pkg-config failed for {package}: {error}"));
    }
}

/// Zig bakes the system `.so` paths the pinwin static archive links against
/// into the archive as members. rust-lld chokes on each with
/// "archive member ... is neither `ET_REL` nor LLVM bitcode" and then links
/// fine without them: the same libraries resolve via pkg-config below.
/// Delete every non-object member so the compile is warning-free.
fn strip_syslib_refs(lib_dir: &Path) {
    let archives: Vec<PathBuf> = std::fs::read_dir(lib_dir)
        .expect("zig build should install lib/ archives")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension().is_some_and(|ext| ext == "a")).then_some(path)
        })
        .collect();
    for archive in &archives {
        let table = Command::new("zig")
            .arg("ar")
            .arg("t")
            .arg(archive)
            .output()
            .expect("failed to run `zig ar t`");
        assert!(
            table.status.success(),
            "`zig ar t` failed for {}: {}",
            archive.display(),
            String::from_utf8_lossy(&table.stderr)
        );
        let listing = String::from_utf8_lossy(&table.stdout);
        let junk: Vec<&str> = listing
            .lines()
            .filter(|member| {
                !Path::new(member)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("o"))
            })
            .collect();
        if junk.is_empty() {
            continue;
        }
        let status = Command::new("zig")
            .arg("ar")
            .arg("d")
            .arg(archive)
            .args(&junk)
            .status()
            .expect("failed to run `zig ar d`");
        assert!(
            status.success(),
            "`zig ar d` failed for {}",
            archive.display()
        );
    }
}

fn build_pinwin(zig_dir: &Path, prefix: &Path, cache_dir: &Path) {
    let status = Command::new("zig")
        .current_dir(zig_dir)
        .arg("build")
        .arg("-Doptimize=ReleaseSafe")
        .arg("--prefix")
        .arg(prefix)
        .arg("--cache-dir")
        .arg(cache_dir)
        .status()
        .expect("failed to run `zig build` for the pinwin library");
    assert!(
        status.success(),
        "`zig build` for the pinwin library failed: {status}"
    );
}
