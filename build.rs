fn main() {
    println!("cargo:rerun-if-changed=assets/icon.svg");

    let svg = std::fs::read_to_string("assets/icon.svg").expect("assets/icon.svg not found");
    // Emby green (#52B54B = RGB 82, 181, 75), matching palette::IRIS
    let svg_colored = svg.replace("currentColor", "#52B54B");

    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&svg_colored, &opt).expect("failed to parse icon.svg");

    let mut pixmap = resvg::tiny_skia::Pixmap::new(24, 24).expect("pixmap alloc failed");
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // StatusNotifierItem spec requires ARGB32 in network byte order (big-endian)
    let mut argb = Vec::with_capacity(24 * 24 * 4);
    for px in pixmap.data().chunks_exact(4) {
        argb.extend_from_slice(&[px[3], px[0], px[1], px[2]]); // RGBA → ARGB
    }

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("tray_icon.bin");
    std::fs::write(out, argb).unwrap();
}
