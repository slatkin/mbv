//! The `mbv` program: a thin process entry over the `mbv` library, which owns
//! start-up, the Owner link, and the TUI Client run.

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    mbv::run();
}
