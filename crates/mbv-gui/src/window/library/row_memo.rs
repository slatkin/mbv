//! The row memo the Library lists share (`gui-change-driven-sections` design
//! D9, the port of `lib/library/row-memo.ts`; `gui-tech-debt-simplification`
//! design D11). A list's painted rows are a pure function of its carried
//! content and its interaction facts: the filter text, the open nodes, the
//! view mode. A scroll step or a cursor move reads the same facts, so the memo
//! hands back the rows it built last, and the rows keep their identity. Each
//! list keeps its own memo, and one type serves every list: each names its own
//! row and its own facts.

use std::sync::Arc;

use dioxus_native::prelude::*;

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former
/// client semantics the memo preserves are shared state: one scroll step reads
/// the same rows the last one built, and the list repaints only its visible
/// window.
struct MemoEntry<Row, Facts> {
    /// The `libraryList` section revision the rows came from (design D6 of
    /// `gui-change-driven-sections`): a section's value at one revision is
    /// immutable, so equal revisions carry equal content, and the hit test
    /// never compares the carried content itself.
    revision: Option<u64>,
    /// The interaction facts over that content that decided which rows were
    /// painted.
    facts: Facts,
    rows: Arc<[Row]>,
}

/// The Library lists' row memo (`row-memo.ts`'s one-slot memo; design D11).
/// One slot, because a list paints one content at a time, and one type,
/// because every list's memo holds the same key, facts, and rows. The list
/// that owns the memo names its `Row` and its `Facts`, and `Facts` decides
/// when the rows stay.
pub struct RowsMemo<Row, Facts> {
    last: Signal<Option<MemoEntry<Row, Facts>>>,
}

impl<Row, Facts> std::fmt::Debug for RowsMemo<Row, Facts> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowsMemo").finish_non_exhaustive()
    }
}

impl<Row, Facts> Clone for RowsMemo<Row, Facts> {
    fn clone(&self) -> Self {
        Self { last: self.last }
    }
}

impl<Row: 'static, Facts: 'static> Default for RowsMemo<Row, Facts> {
    fn default() -> Self {
        Self {
            last: Signal::new(None),
        }
    }
}

impl<Row: 'static, Facts: PartialEq + 'static> RowsMemo<Row, Facts> {
    /// The rows `build` leaves over the carried content `revision` keys and
    /// the interaction `facts`: the ones it built last while the arguments are
    /// the same, and a fresh build otherwise. The rows come keyed on the
    /// `libraryList` section revision they borrowed from (design D6); the
    /// caller must pass the facts of the section's current value.
    #[must_use]
    pub fn rows<Build>(&self, revision: Option<u64>, facts: Facts, build: Build) -> Arc<[Row]>
    where
        Build: FnOnce(&Facts) -> Vec<Row>,
    {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a settle that moves the cursor, a filter's scroll clamp)
        // would re-queue itself on the write below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.facts == facts
        {
            return Arc::clone(&entry.rows);
        }
        let painted: Arc<[Row]> = build(&facts).into();
        let mut last = self.last;
        last.set(Some(MemoEntry {
            revision,
            facts,
            rows: Arc::clone(&painted),
        }));
        painted
    }
}
