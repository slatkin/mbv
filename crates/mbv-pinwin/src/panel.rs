//! The handle for a running pinwin panel.

use std::marker::PhantomData;
use std::os::fd::{AsRawFd, BorrowedFd};

use crate::error::PinwinError;
use crate::ffi;
use crate::layout::{KeyboardMode, Layout};

/// A running pinwin panel.
///
/// The handle borrows the master fd for its whole life: the library reads and
/// writes that fd until the panel stops, which happens when the handle drops.
#[derive(Debug)]
pub struct Panel<'fd> {
    _master: PhantomData<BorrowedFd<'fd>>,
}

impl<'fd> Panel<'fd> {
    /// Mirror of `PINWIN_ANIM_DEFAULT_MS` (`pinwin_api.h`): the documented
    /// default duration for a width transition, in milliseconds.
    pub const ANIM_DEFAULT_MS: u32 = 200;

    /// Start a panel on `master`, opening with `layout`.
    ///
    /// `master` stays the host's pty master and stays open for as long as the
    /// returned handle (and therefore the panel) lives.
    pub fn start(
        master: BorrowedFd<'fd>,
        layout: Layout,
        keyboard_mode: KeyboardMode,
    ) -> Result<Self, PinwinError> {
        let startup = ffi::PinwinStartup {
            master_fd: master.as_raw_fd(),
            layout: layout.to_abi(),
            keyboard_mode: keyboard_mode.to_abi(),
            accent: ffi::PinwinAccent::DISABLED,
        };
        // SAFETY: `startup` is a live, fully initialized `PinwinStartup`, and
        // the C ABI only reads it for the duration of the call.
        let code = unsafe { ffi::pinwin_start(&raw const startup) };
        if code == ffi::OK {
            Ok(Self {
                _master: PhantomData,
            })
        } else {
            Err(PinwinError::from_code(code))
        }
    }

    /// Apply a full new layout to the running panel, animating a width-only
    /// change over `duration_ms`.
    ///
    /// When only the column count differs from the applied layout,
    /// `duration_ms` is greater than zero and GTK animations are enabled, the
    /// panel width and the space it reserves ease to the target over
    /// `duration_ms`. Anything else applies in one step. On success the panel
    /// moves and resizes with its terminal state preserved. On
    /// `PINWIN_ERR_INVALID` the applied layout is unchanged.
    pub fn apply_layout_animated(
        &self,
        layout: Layout,
        duration_ms: u32,
    ) -> Result<(), PinwinError> {
        let abi = layout.to_abi();
        // SAFETY: `abi` is a live, fully initialized `PinwinLayout`, and the C
        // ABI only reads it for the duration of the call.
        let code = unsafe { ffi::pinwin_apply_layout_animated(&raw const abi, duration_ms) };
        if code == ffi::OK {
            Ok(())
        } else {
            Err(PinwinError::from_code(code))
        }
    }
}

impl Drop for Panel<'_> {
    fn drop(&mut self) {
        // SAFETY: a `Panel` exists only after `pinwin_start` returned `OK`, so
        // a panel is running; the C ABI's `pinwin_stop` is a no-op otherwise.
        // Rust code never runs on the library's GTK thread, so this is not the
        // forbidden GTK-thread call.
        unsafe { ffi::pinwin_stop() };
    }
}
