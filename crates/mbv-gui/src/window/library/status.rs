//! The Library Status bar's location (`add-gui-library-panel` design D10, the
//! port of `lib/library/status.ts`): the destination, the selected collection
//! names, and the picked item's artist or title, joined by ` › ` as the canvas
//! prints them. Every part reads the carried Library sections, so the line
//! composes no state of its own: the active destination's list part and detail
//! part narrow on their kind.

use mbv::GuiCollections;

use crate::bridge::frame::library::{
    GuiAudiobooksRowFrame, GuiDestinationList, GuiEmbyRowFrame, GuiEntryRowFrame, GuiLibraryDetail,
    GuiLibraryList, GuiLibraryTabFrame, GuiMusicHeroFrame, GuiShowsRowFrame,
};

/// The current destination's label: the tab whose position the frame marks
/// current, absent while the active destination is not one of the tabs.
fn destination_label(tabs: &[GuiLibraryTabFrame], current_tab: Option<usize>) -> Option<String> {
    tabs.iter()
        .find(|tab| Some(tab.position) == current_tab)
        .map(|tab| tab.label.clone())
}

/// The picked item's name (design D5, D10): the Hero's artist, or the album's
/// title while the Service names no artist. Absent while nothing is picked.
fn picked_name(hero: Option<&GuiMusicHeroFrame>) -> Option<String> {
    let hero = hero?;
    Some(if hero.artist.trim().is_empty() {
        hero.album.name.clone()
    } else {
        hero.artist.clone()
    })
}

/// The id and title fields the selected-title segment reads off a carried
/// row list's rows (design D10, D12).
trait LocationRow {
    /// The row's stable id.
    fn row_id(&self) -> &str;

    /// The row's title.
    fn row_title(&self) -> &str;
}

impl LocationRow for GuiShowsRowFrame {
    fn row_id(&self) -> &str {
        &self.id
    }

    fn row_title(&self) -> &str {
        &self.title
    }
}

impl LocationRow for GuiEmbyRowFrame {
    fn row_id(&self) -> &str {
        &self.id
    }

    fn row_title(&self) -> &str {
        &self.title
    }
}

impl LocationRow for GuiAudiobooksRowFrame {
    fn row_id(&self) -> &str {
        &self.id
    }

    fn row_title(&self) -> &str {
        &self.title
    }
}

impl LocationRow for GuiEntryRowFrame {
    fn row_id(&self) -> &str {
        &self.id
    }

    fn row_title(&self) -> &str {
        &self.title
    }
}

/// The location line's last segment a destination's rows name (design D10,
/// D12): the title of the row the core selected, which is the row the Hero
/// shows. A selection that names no carried row leaves the segment out.
fn selected_title<Row: LocationRow>(selected: Option<&str>, rows: &[Row]) -> Option<String> {
    let selected = selected?;
    rows.iter()
        .find(|row| row.row_id() == selected)
        .map(|row| row.row_title().to_owned())
}

/// The selected row's id of a row-list destination's detail part. Music and
/// Home carry no selected row id, so they read `None`.
fn detail_selected(detail: Option<&GuiLibraryDetail>) -> Option<&str> {
    match detail {
        Some(GuiLibraryDetail::Shows(detail)) => detail.selected.as_deref(),
        Some(GuiLibraryDetail::Emby(detail)) => detail.selected.as_deref(),
        Some(GuiLibraryDetail::Audiobooks(detail)) => detail.selected.as_deref(),
        Some(GuiLibraryDetail::Podcasts(detail)) => detail.selected.as_deref(),
        Some(GuiLibraryDetail::Feeds(detail)) => detail.selected.as_deref(),
        _ => None,
    }
}

/// The location line's middle segment a collection selector names (design
/// D10, D6): the checked collections' names, or `All` while the selection is
/// All, joined as one segment. A destination with no selector leaves the
/// segment out.
fn selected_collection(collections: Option<&GuiCollections>) -> Option<String> {
    let names = collections?.selected_names();
    (!names.is_empty()).then(|| names.join(", "))
}

/// The Home detail's Hero title (design D10, D12): the selected row's Hero
/// title while the detail part is Home's, absent otherwise.
fn home_hero_title(detail: Option<&GuiLibraryDetail>) -> Option<String> {
    match detail {
        Some(GuiLibraryDetail::Home(detail)) => detail.hero.as_ref().map(|hero| hero.title.clone()),
        _ => None,
    }
}

/// The active Music detail's Hero, absent while another destination holds the
/// window or no album is picked.
fn music_hero(detail: Option<&GuiLibraryDetail>) -> Option<&GuiMusicHeroFrame> {
    match detail {
        Some(GuiLibraryDetail::Music(detail)) => detail.hero.as_ref(),
        _ => None,
    }
}

/// The location line's raw segments for the active destination, in canvas
/// order. A `None` segment drops out when the line is joined.
fn segments_of(
    tabs: &[GuiLibraryTabFrame],
    current_tab: Option<usize>,
    list: Option<&GuiLibraryList>,
    detail: Option<&GuiLibraryDetail>,
) -> Vec<Option<String>> {
    let label = destination_label(tabs, current_tab);
    let Some(list) = list else {
        return vec![label];
    };
    match &list.destination {
        GuiDestinationList::Home(_) => {
            vec![Some("Home".to_owned()), home_hero_title(detail)]
        }
        GuiDestinationList::Shows(shows) => {
            // The series list is the whole library (design D8): Shows paints
            // no collection selector, so the line names no collection.
            let selected = detail_selected(detail);
            vec![label, selected_title(selected, &shows.rows)]
        }
        GuiDestinationList::Emby(emby) => {
            let selected = detail_selected(detail);
            let mut segments = vec![label];
            if emby.feed_group {
                segments.extend(emby.collection_names.iter().cloned().map(Some));
            }
            segments.push(selected_title(selected, &emby.rows));
            segments
        }
        GuiDestinationList::Audiobooks(audiobooks) => {
            let selected = detail_selected(detail);
            vec![label, selected_title(selected, &audiobooks.rows)]
        }
        GuiDestinationList::Podcasts(podcasts) => {
            let selected = detail_selected(detail);
            vec![
                label,
                selected_collection(Some(&podcasts.collections)),
                selected_title(selected, &podcasts.rows),
            ]
        }
        GuiDestinationList::Feeds(feeds) => {
            let selected = detail_selected(detail);
            vec![
                label,
                selected_collection(Some(&feeds.collections)),
                selected_title(selected, &feeds.rows),
            ]
        }
        GuiDestinationList::Music(music) => {
            let mut segments = vec![label];
            segments.extend(music.collections.selected_names().into_iter().map(Some));
            segments.push(picked_name(music_hero(detail)));
            segments
        }
    }
}

/// The location line's segments in canvas order. On Home the line reads
/// `Home`, then the selected row's Hero title (design D10, D12). On Shows it
/// reads the destination tab's label and the selected row's title, because
/// the series list carries no collection selector (design D8). On a plain Emby
/// library it reads the destination tab's label and the selected row's title
/// (row 8.1), and in feed-group mode the checked folder names in between. On an
/// Audiobooks library it reads the destination tab's label and the selected
/// book's title (row 9.1). On a Podcasts library it reads the destination tab's
/// label, the checked shows, and the selected episode's title (row 10.1). On a
/// Feeds destination it reads the destination tab's label, the checked
/// subscriptions, and the selected entry's title (row 11.1). A segment the
/// content does not fill drops, so an unpicked Hero leaves the destination and
/// the collections, and a destination outside the tabs leaves the rest.
#[must_use]
pub fn library_location_segments(
    tabs: &[GuiLibraryTabFrame],
    current_tab: Option<usize>,
    list: Option<&GuiLibraryList>,
    detail: Option<&GuiLibraryDetail>,
) -> Vec<String> {
    segments_of(tabs, current_tab, list, detail)
        .into_iter()
        .flatten()
        .filter(|segment| !segment.trim().is_empty())
        .collect()
}

/// The location line the Library Status bar paints: its segments joined by the
/// canvas's separator.
#[must_use]
pub fn library_location(
    tabs: &[GuiLibraryTabFrame],
    current_tab: Option<usize>,
    list: Option<&GuiLibraryList>,
    detail: Option<&GuiLibraryDetail>,
) -> String {
    library_location_segments(tabs, current_tab, list, detail).join(" › ")
}

#[cfg(test)]
mod tests;
