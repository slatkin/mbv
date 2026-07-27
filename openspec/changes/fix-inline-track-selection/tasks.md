## 1. Display plan row ordering

- [x] 1.1 In `src/app/render/album_plan.rs`, move the track detail row insertion (lines 249–280) from after the album loop into the loop body at lines 241–247, gated on `idx == cursor`, so tracks appear immediately after the cursor album's row and its wrapped continuation rows

## 2. Verification

- [x] 2.1 Run `cargo build` and fix any compilation errors
- [x] 2.2 Run `cargo clippy` and `cargo fmt` to ensure code quality
