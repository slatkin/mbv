//! The Library rows' `data-library-*` hook names: the computed attribute
//! names the former components built per destination (`{...{ [key]: value
//! }}` spreads over the destination's prefix and a part name). Dioxus
//! attribute names are `'static`, so each distinct name is built once and
//! leaked; the names are the small cross product of the destinations' static
//! prefixes and the fixed part names, so the leak is bounded and each name
//! is paid for one time.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// The names already built, keyed by the unprefixed full name.
static INTERNED: LazyLock<Mutex<HashMap<Box<str>, &'static str>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The `data-library-*` attribute named by the destination's `prefix` and
/// the part name: an empty prefix (the Music list) drops the prefix segment,
/// and an empty part (the Workspace table's row hook) drops the part segment,
/// as the former spreads built them.
#[must_use]
pub fn library_attr(prefix: &str, part: &str) -> &'static str {
    let name = if prefix.is_empty() {
        format!("data-library-{part}")
    } else if part.is_empty() {
        format!("data-library-{prefix}")
    } else {
        format!("data-library-{prefix}-{part}")
    };
    let mut interned = INTERNED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match interned.entry(name.into()) {
        std::collections::hash_map::Entry::Occupied(held) => held.get(),
        std::collections::hash_map::Entry::Vacant(vacant) => {
            let leaked: &'static str = Box::leak(vacant.key().clone());
            vacant.insert(leaked);
            leaked
        }
    }
}
