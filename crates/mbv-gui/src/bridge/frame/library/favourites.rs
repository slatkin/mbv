//! The Favourites list part of the Library frame (emby-favourites design D6):
//! the rows the Shows, Music, and plain Emby library list parts carry while
//! their view is Favourites, the list's own pick, and the picked row's Hero.
//! The rows are the plain-library list's own row frame, so the window paints
//! them through the shared flat-row parts; a command names a row by the stable
//! Service id the frame carries, never by position.

use crate::gui_core::GuiFavouritesProjection;

use super::emby::{GuiEmbyHeroFrame, GuiEmbyRowFrame};

/// One library's Favourites list (emby-favourites design D6), as the Shows,
/// Music, and plain Emby library list parts carry it while their view is
/// Favourites.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuiFavouritesList {
    /// Whether the list has not landed yet. A failed first load lands an empty
    /// list, so this is never a failure mark (design D6).
    pub loading: bool,
    /// The rows in paint order: the collections first, then the media items,
    /// each group in Title order (design D6, D9).
    pub rows: Vec<GuiEmbyRowFrame>,
    /// The row the Favourites Hero shows, as the frame painted it: a select
    /// command sends it back. The first row is the Hero default.
    pub selected: Option<String>,
    /// The picked row's Hero facts; `None` while no row is picked. Every
    /// Favourites row paints one, folders included (design D6).
    pub hero: Option<GuiEmbyHeroFrame>,
}

impl GuiFavouritesList {
    /// The Favourites list part for `projection` (design D6).
    pub(crate) fn from_projection(projection: &GuiFavouritesProjection) -> Self {
        Self {
            loading: projection.loading,
            rows: projection.rows.iter().map(GuiEmbyRowFrame::from).collect(),
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiEmbyHeroFrame::from),
        }
    }
}
