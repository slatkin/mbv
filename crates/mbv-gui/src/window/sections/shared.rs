//! The one handle a large section's readers share (`share-gui-sections-by-rc`
//! design D1). A component that reads a section deep-clones it on every
//! render, so the four large sections — the Library list, the Library detail,
//! the Queue, and the Settings frame — travel as one [`SectionRef`], built
//! once per section update, and a read clones the handle instead of the
//! content.
//!
//! The handle derefs to its value and never to a mutable one: a section's
//! value at one revision is immutable, which is why pointer equality is a
//! sound equality. A new revision always allocates a new handle, so a changed
//! section never compares equal to the handle the window held before it. The
//! window is single-threaded, so a plain `Rc` suffices; the section batch
//! crosses the core channel as plain owned values, and the handle is made on
//! this side of it.

use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

/// One section's value, shared with every reader that clones this handle
/// (design D1).
pub struct SectionRef<T> {
    /// The shared value. Never handed out as a mutable reference, so the
    /// value behind the handle stays the value it was built from.
    value: Rc<T>,
}

impl<T> SectionRef<T> {
    /// Share `value` through the one handle this section's readers clone.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            value: Rc::new(value),
        }
    }
}

impl<T> Clone for SectionRef<T> {
    fn clone(&self) -> Self {
        Self {
            value: Rc::clone(&self.value),
        }
    }
}

impl<T> Deref for SectionRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> PartialEq for SectionRef<T> {
    /// Pointer equality: the same handle is the same section at the same
    /// revision, and a new revision is a new handle (design D1). A prop's
    /// memo check therefore costs the same for a large section as for a
    /// small one.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.value, &other.value)
    }
}

impl<T: fmt::Debug> fmt::Debug for SectionRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SectionRef").field(&self.value).finish()
    }
}

/// Share a carried `Option` section's value, so a signal that holds
/// `Option<SectionRef<T>>` stores one handle per write (design D2).
#[must_use]
pub fn shared_option<T>(value: Option<T>) -> Option<SectionRef<T>> {
    value.map(SectionRef::new)
}
