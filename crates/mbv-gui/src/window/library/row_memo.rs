//! The row memo the Library lists share (`gui-change-driven-sections` design
//! D9, the port of `lib/library/row-memo.ts`). A list's painted rows are a
//! pure function of its carried content and its interaction facts: the filter
//! text, the open nodes, the view mode. A scroll step or a cursor move reads
//! the same facts, so the memo hands back the rows it built last, and the rows
//! keep their identity. Each list keeps its own memo.

use std::sync::Arc;

use dioxus_native::prelude::*;

use super::list::LibraryRow;

/// The memo's arguments for the canvas tree (`state.ts`'s `musicRowsOf`):
/// the carried artists, the open nodes, the filter's closed overrides, and
/// the filter text. The TypeScript memo compared the arguments by identity;
/// borrowed Rust slices have no stable identity across a signal read, so the
/// memo compares them by value — a miss rebuilds, a hit returns the rows it
/// built last, and the rows keep their identity either way.
#[derive(Debug, Clone, PartialEq)]
struct TreeArgs {
    artists: Vec<crate::bridge::frame::library::GuiMusicArtistFrame>,
    open: Vec<String>,
    closed: Vec<String>,
    filter: String,
}

/// The rows the memo handed back last, with the arguments they were built
/// from. The rows sit behind an `Arc` because the Svelte semantics the memo
/// preserves are shared state: one scroll step reads the same rows the last
/// one built, and the list repaints only its visible window.
#[derive(Debug, Clone)]
struct MemoEntry {
    args: TreeArgs,
    rows: Arc<[LibraryRow]>,
}

/// The canvas tree's row memo (`row-memo.ts`'s one-slot memo over
/// `buildLibraryRows`). One slot, because a list paints one content at a
/// time.
#[derive(Clone, Default)]
pub struct RowsMemo {
    last: Signal<Option<MemoEntry>>,
}

impl std::fmt::Debug for RowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowsMemo").finish_non_exhaustive()
    }
}

impl RowsMemo {
    /// The rows `build` leaves over `tree`: the ones it built last while the
    /// arguments are the same, and a fresh build otherwise.
    #[must_use]
    pub fn rows(
        &self,
        artists: &[crate::bridge::frame::library::GuiMusicArtistFrame],
        open: &[String],
        closed: &[String],
        filter: &str,
    ) -> Arc<[LibraryRow]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a settle that moves the cursor) would re-queue itself on the
        // write below otherwise.
        if let Some(entry) = self.last.peek().as_ref() {
            let same = entry.args.artists == artists
                && entry.args.open == open
                && entry.args.closed == closed
                && entry.args.filter == filter;
            if same {
                return Arc::clone(&entry.rows);
            }
        }
        let rows: Arc<[LibraryRow]> = super::list::build_library_rows(super::list::LibraryTree {
            artists,
            open,
            closed,
            filter,
        })
        .into();
        let mut last = self.last;
        last.set(Some(MemoEntry {
            args: TreeArgs {
                artists: artists.to_vec(),
                open: open.to_vec(),
                closed: closed.to_vec(),
                filter: filter.to_owned(),
            },
            rows: Arc::clone(&rows),
        }));
        rows
    }
}
