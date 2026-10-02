//! Owned layout types, mirroring the C ABI's `PinwinLayout` and startup mode.

use crate::ffi;

/// The screen edge the panel docks to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Docked to the left edge.
    Left,
    /// Docked to the right edge.
    Right,
}

impl Side {
    pub(crate) const fn to_abi(self) -> i32 {
        match self {
            Side::Left => ffi::SIDE_LEFT,
            Side::Right => ffi::SIDE_RIGHT,
        }
    }
}

/// The panel's gutters in logical pixels; negative values are allowed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gutters {
    /// Inset from the output's top edge.
    pub top: i32,
    /// Inset from the output's bottom edge.
    pub bottom: i32,
    /// Inset from the output's left edge.
    pub left: i32,
    /// Inset from the output's right edge.
    pub right: i32,
}

/// A full panel layout: docking side, width in columns and gutters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The edge the panel docks to.
    pub side: Side,
    /// Panel width in terminal columns (`1..=65535`; the ABI rejects `0`).
    pub cols: u16,
    /// The panel's four gutters.
    pub gutters: Gutters,
}

impl Layout {
    pub(crate) fn to_abi(self) -> ffi::PinwinLayout {
        ffi::PinwinLayout {
            side: self.side.to_abi(),
            cols: i32::from(self.cols),
            top: self.gutters.top,
            bottom: self.gutters.bottom,
            left: self.gutters.left,
            right: self.gutters.right,
        }
    }
}

/// The keyboard interactivity requested when the panel starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardMode {
    /// The panel never takes keyboard input.
    None,
    /// The panel has the keyboard as soon as it opens.
    Exclusive,
    /// The panel takes the keyboard only after a click.
    OnDemand,
}

impl KeyboardMode {
    pub(crate) const fn to_abi(self) -> i32 {
        match self {
            KeyboardMode::None => ffi::KEYBOARD_NONE,
            KeyboardMode::Exclusive => ffi::KEYBOARD_EXCLUSIVE,
            KeyboardMode::OnDemand => ffi::KEYBOARD_ON_DEMAND,
        }
    }
}
