//! Name-addressed Music artist art (gui-delineated-rows design D5). A
//! folder-based Music library groups its artist rows by name and holds no
//! Service artist id for them. Two sources name the Service artist item whose
//! `Primary` image the row shows: the level fills' tracks (an artist name paired
//! with the id the tracks carry), and, for a name no track names (untagged
//! files), Emby's artist search scored by the one shared fuzzy matcher. The id
//! found either way feeds the row's image source only: it never becomes an
//! `ArtistKey`, so artist keys, artist-track queries, and the Hero stay on
//! `ArtistItems` identity.
//!
//! The GUI half that owned the search has no caller that holds a `GuiCore`
//! (`decouple-client-core` design D4, task 7.7): the row art lookups and the
//! in-view fetch queue read the `App` they hold, so they enter the core's
//! `ArtistNameLookup` directly and this module holds no GUI entry.
