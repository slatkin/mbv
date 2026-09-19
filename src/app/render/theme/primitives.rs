//! Former home of the raw `Color` primitives. The `Palette` enum
//! (`palette.rs`) now owns every palette colour as a single `Rgb` literal per
//! variant, and every role and surface row references it directly, so no
//! primitive remains (openspec/changes/palette-enum 3.1/3.2). The module
//! itself is deleted by that change's cleanup row (4.1).
