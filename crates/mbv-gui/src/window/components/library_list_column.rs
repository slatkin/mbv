//! The Library lists' one column (`gui-tech-debt-simplification` design D14):
//! the destination's header, its notice lines, and the scroll shell over its
//! rows, beside its letter rail where it paints one. Every Library list — the
//! Music tree, Home, Shows, and the four lettered destinations — paints its
//! column through [`library_list_column`], so the wiring each of them repeated
//! exists once: the shell's three reports (the measured viewport, the native
//! scroll offset, and the rows in view), the header block, the notice block,
//! and the row that holds the shell beside the rail.
//!
//! The scaffold is a plain render function, not a component, for the reason
//! [`crate::window::components::library_virtual_list`]'s shell is: the shell's
//! hooks attach to the calling destination's scope, and a component boundary
//! would skip the repaint the rows' own state reads need. Each destination
//! keeps its row model, its header facts, its notice lines and texts, and its
//! row painter; this module holds only the wiring they share, and the column's
//! own hook (`data-library-<word>-list-column`) is built here from the
//! destination's attribute word, so no destination hand-writes it.

use dioxus_native::prelude::*;

use crate::window::components::library_attr::library_attr;
use crate::window::components::library_list_header::{LibraryListHeader, LibraryListHeaderProps};
use crate::window::components::library_virtual_list::library_virtual_list;
use crate::window::components::list_frame_notices::{ListFrameNotices, ListFrameNoticesProps};
use crate::window::library::list::GeometryRow;
use crate::window::library::scroll::ListScroll;
use crate::window::pointer_target::PointerList;

/// The scroll shell's inputs, as the destination carries them (design D14): the
/// rows it paints, its scroll store's two values, its attribute word and
/// pointer list, its padding, and the three reports it makes — the measured
/// viewport height back to its store, the native scroll offset back to it, and
/// the rows in view to the core for their row art fetches
/// (gui-delineated-rows design D7).
#[derive(Debug, Clone, Copy)]
pub struct ListColumnWiring<'a, R> {
    /// The rows the shell paints.
    pub rows: &'a [R],
    /// The destination's scroll store: the native offset and the viewport it
    /// measured (design D9).
    pub view: ListScroll,
    /// The attribute word the destination's DOM hooks carry: empty for Music,
    /// the destination's own word for every other list.
    pub attr: &'static str,
    /// The destination's list kind, which the scroller carries as its pointer
    /// marker (`gui-pointer-target-interaction` D1).
    pub list: PointerList,
    /// The padding the scroller applies.
    pub padding: &'static str,
    /// The measured viewport height, reported to the destination's store.
    pub on_measure: Callback<f32>,
    /// The native scroll offset, reported to the destination's store.
    pub on_scroll: Callback<f32>,
    /// The rows in view, reported to the core: the painted ids plus the
    /// overscan either side. A destination that paints no thumb reports none.
    pub rows_in_view: Option<Callback<Vec<String>>>,
}

/// Where the shell's rows sit under the column's header (design D14).
#[derive(Debug)]
pub enum ListColumnBody {
    /// The shell fills the column's remaining height: Home paints no rail, so
    /// it paints no row around the shell either.
    Shell,
    /// The shell beside the destination's letter rail, both inside the one
    /// hidden-overflow row that every other list paints. A date-sorted lettered
    /// list's rows carry no letter, so it paints the row holding the shell
    /// alone.
    Rail(Option<Element>),
}

/// The one list column every Library destination paints (design D14): the
/// column's hook, the destination's header, its notice lines, and the scroll
/// shell over its rows — beside the destination's letter rail where it paints
/// one. `paint_row` is the destination's own row painter, and `header` and
/// `notices` are its own facts and texts; the wiring between them is this
/// module's.
pub fn library_list_column<R: GeometryRow>(
    wiring: &ListColumnWiring<'_, R>,
    paint_row: impl Fn(&R) -> Element,
    header: LibraryListHeaderProps,
    notices: Option<ListFrameNoticesProps>,
    body: ListColumnBody,
) -> Element {
    let shell = library_virtual_list(
        wiring.rows,
        wiring.view,
        wiring.attr,
        wiring.list,
        wiring.padding,
        wiring.on_measure,
        wiring.on_scroll,
        wiring.rows_in_view,
        paint_row,
    );
    let body = match body {
        ListColumnBody::Shell => shell,
        ListColumnBody::Rail(rail) => {
            rsx! {
                div {
                    class: "flex min-h-0 flex-1",
                    style: "overflow: hidden;",
                    {shell}
                    {rail}
                }
            }
        }
    };
    let column_attrs = vec![Attribute::new(
        library_attr(wiring.attr, "list-column"),
        "",
        None,
        false,
    )];
    rsx! {
        div {
            class: "flex min-h-0 min-w-0 flex-1 flex-col",
            ..column_attrs,
            div { class: "flex flex-none flex-col", LibraryListHeader { ..header } }
            if let Some(notices) = notices {
                ListFrameNotices { ..notices }
            }
            {body}
        }
    }
}
