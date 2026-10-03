//! Raw declarations for `libpinwin`'s C ABI (`pinwin/src/pinwin_api.h`).
//!
//! Private on purpose: the public API exposes owned Rust types, never these
//! pointers or repr(C) structs.

/// `PINWIN_SIDE_*` (`options.h`).
pub(crate) const SIDE_LEFT: i32 = 0;
pub(crate) const SIDE_RIGHT: i32 = 1;

/// `PINWIN_KEYBOARD_*` (`pinwin.h`); `pinwin_api.h` pins these as ABI values.
pub(crate) const KEYBOARD_NONE: i32 = 0;
pub(crate) const KEYBOARD_EXCLUSIVE: i32 = 1;
pub(crate) const KEYBOARD_ON_DEMAND: i32 = 2;

/// `PINWIN_OK` and the `PINWIN_ERR_*` result codes (`pinwin_api.h`).
pub(crate) const OK: i32 = 0;
pub(crate) const ERR_INVALID: i32 = 1;
pub(crate) const ERR_ALREADY_RUNNING: i32 = 2;
pub(crate) const ERR_NOT_RUNNING: i32 = 3;
pub(crate) const ERR_NO_DISPLAY: i32 = 4;
pub(crate) const ERR_INTERNAL: i32 = 5;

/// Mirror of `PinwinLayout`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PinwinLayout {
    pub(crate) side: i32,
    pub(crate) cols: i32,
    pub(crate) top: i32,
    pub(crate) bottom: i32,
    pub(crate) left: i32,
    pub(crate) right: i32,
}

/// Mirror of `PinwinStartup`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PinwinStartup {
    pub(crate) master_fd: i32,
    pub(crate) layout: PinwinLayout,
    pub(crate) keyboard_mode: i32,
}

unsafe extern "C" {
    pub(crate) fn pinwin_start(startup: *const PinwinStartup) -> i32;
    pub(crate) fn pinwin_apply_layout(layout: *const PinwinLayout) -> i32;
    pub(crate) fn pinwin_stop();
}
