//! Name-addressed Music artist art (gui-delineated-rows design D5). A
//! folder-based Music library groups its artist rows by name and holds no
//! Service artist id for them. Two sources name the Service artist item whose
//! `Primary` image the row shows: the level fills' tracks (an artist name paired
//! with the id the tracks carry), and, for a name no track names (untagged
//! files), Emby's artist search scored by the one shared fuzzy matcher. The id
//! found either way feeds the row's image source only: it never becomes an
//! `ArtistKey`, so artist keys, artist-track queries, and the Hero stay on
//! `ArtistItems` identity.

use crate::app::App;
use crate::app::dispatch::library::artist_lookup::ArtistNameLookup;

impl App {
    /// The Service artist item id known for an artist name, if any.
    pub(in crate::app) fn gui_artist_item_id(&self, name: &str) -> Option<&str> {
        ArtistNameLookup::item_id(self, name)
    }

    /// Start the artist search for `name` once (design D4a (b)): the core's
    /// `ArtistNameLookup` keeps the record, and this GUI half owns the search
    /// the record's map is keyed for.
    pub(in crate::app) fn request_gui_artist_name_lookup(&mut self, name: &str) {
        ArtistNameLookup::request(self, name);
    }
}
