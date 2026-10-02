//! Build `pinwin/` with Zig and link the panel into this crate.
//!
//! Zig installs its artifacts under `OUT_DIR`, never into the source tree
//! (`--prefix`/`--cache-dir`); only Zig's own `zig-pkg/` dependency extraction
//! stays beside `pinwin/`, as it does for `zig build` run by hand. Link order
//! matters: the static archive of `pinwin` precedes `ghostty-vt` (which it
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
    let pinwin_dir = manifest_dir.join("../../pinwin");
    let out_dir =
        PathBuf::from(env::var("OUT_DIR").expect("cargo always sets OUT_DIR for a build script"));

    for path in [
        pinwin_dir.join("src"),
        pinwin_dir.join("build.zig"),
        pinwin_dir.join("build.zig.zon"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let prefix = out_dir.join("pinwin");
    build_pinwin(&pinwin_dir, &prefix, &out_dir.join("zig-cache"));

    let lib_dir = prefix.join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Static archives first, in dependency order: pinwin references
    // ghostty-vt, ghostty-vt references the two vendored SIMD archives. Zig
    // installs all four beside each other (upstream build.zig).
    for lib in ["pinwin", "ghostty-vt", "simdutf", "highway"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    // gtk4-layer-shell first among the GTK libraries (design, Risks).
    for package in ["gtk4-layer-shell-0", "gtk4", "pangocairo"] {
        pkg_config::Config::new()
            .probe(package)
            .unwrap_or_else(|error| panic!("pkg-config failed for {package}: {error}"));
    }
}

fn build_pinwin(pinwin_dir: &Path, prefix: &Path, cache_dir: &Path) {
    let status = Command::new("zig")
        .current_dir(pinwin_dir)
        .arg("build")
        .arg("-Doptimize=ReleaseSafe")
        .arg("--prefix")
        .arg(prefix)
        .arg("--cache-dir")
        .arg(cache_dir)
        .status()
        .expect("failed to run `zig build` for the vendored pinwin library");
    assert!(
        status.success(),
        "`zig build` for the vendored pinwin library failed: {status}"
    );
}
