## 1. Declare Movie Logo Artwork

- [x] 1.1 Extend `EmbyImageTags` parsing and compatible serialization with `ImageTags.Logo`, and update the existing image-tag parsing cases to verify present and absent Logo declarations without constructing a live Service.
- [x] 1.2 Extend the provider-neutral Hero artwork content with an optional decoration source, populate it only for Movies with a declared Logo, remove Logo from the Movie base-image fallback chain, and extend the existing hero policy table to verify Movie/non-Movie and declared/absent cases.

## 2. Fetch Eligible Logo Images

- [x] 2.1 Route an independently tagged Movie Logo cache key through the existing bounded Emby image fetch pipeline, and verify with existing test instrumentation that only a Wide Portrait Movie projection reserves it while Landscape, Narrow, placeholder, non-Movie, and absent-Logo projections do not.
- [x] 2.2 Preserve base-poster readiness while the optional Logo is pending, empty, or failed, and verify the projection reports the ready poster before Logo completion and reconsiders decoration after a successful completion.

## 3. Composite the Wide Portrait Hero

- [x] 3.1 Add the minimal image operation that contain-resizes a transparent Logo within the 60%-width/20%-height bounds, places it at the clamped 5% top-left inset, and alpha-composites it over an already cover-fitted poster; verify one focused pixel/image test covers aspect preservation, placement, alpha blending, and unchanged pixels outside the Logo.
- [x] 3.2 Include the applied Logo cache identity beside the hero box in protocol validity, build one protocol from either the base or composited bitmap, and verify an arriving Logo rebuilds the base-only protocol once while an unchanged Logo, failed Logo, and unchanged box reuse the existing protocol.

## 4. Presentation and Gates

- [x] 4.1 Extend the narrowest existing Library panel/hero characterization to verify that a Wide Portrait Movie paints the composited single image without changing its artwork box, while the same Movie in Narrow and a Wide Landscape Movie remain undecorated; keep the Wide hero read-only and add no hit geometry.
- [x] 4.2 Run `cargo fmt`, `cargo check -p mbv-core -p mbv`, `cargo nextest run -p mbv-core --no-fail-fast`, `cargo nextest run -p mbv --no-fail-fast`, and `cargo clippy --workspace --all-targets -- -D warnings`; then manually inspect one cold and warm portrait Movie Logo in Wide plus the same Movie below the Wide breakpoint under the configured portable image protocols available locally.
