//! Name-addressed Music artist art (gui-delineated-rows design D5). A
//! folder-based Music library groups its artist rows by name and holds no
//! Service artist id for them. Two sources name the Service artist item whose
//! `Primary` image the row shows: the level fills' tracks (an artist name paired
//! with the id the tracks carry), and, for a name no track names (untagged
//! files), Emby's artist search scored by the one shared fuzzy matcher. The id
//! found either way feeds the row's image source only: it never becomes an
//! `ArtistKey`, so artist keys, artist-track queries, and the Hero stay on
//! `ArtistItems` identity.

use mbv_text::fuzzy_match::{WordMatcher, word_match_score};

use crate::app::{App, LibEvent, MusicEvent};

/// Candidates one artist search asks the Service for.
const ARTIST_LOOKUP_LIMIT: usize = 10;

/// What a name's artist search knows while it has no resolved id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum GuiArtistNameLookup {
    /// The search is in flight.
    Loading,
    /// The Service answered and no artist name matched. Final.
    Unmatched,
}

/// A name with its case and punctuation removed and its leading article
/// dropped, so `AC/DC` and `ac dc`, or `The Smiths` and `Smiths`, meet.
fn normalized(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    match words.split_first() {
        Some((first, rest)) if first == "the" && !rest.is_empty() => rest.join(" "),
        _ => words.join(" "),
    }
}

/// The id of the candidate `(name, id)` that best matches `name`: an equal
/// normalized name first, else the best score of a match both ways, so a folder
/// `Yes` never takes `Yes Sir`.
fn best_artist_match<'a>(name: &str, candidates: &'a [(String, String)]) -> Option<&'a str> {
    let wanted = normalized(name);
    if let Some((_, id)) = candidates
        .iter()
        .find(|(candidate, _)| normalized(candidate) == wanted)
    {
        return Some(id);
    }
    let matcher = WordMatcher::new();
    candidates
        .iter()
        .filter_map(|(candidate, id)| {
            let forward = word_match_score(&matcher, candidate, name)?;
            let backward = word_match_score(&matcher, name, candidate)?;
            Some((forward + backward, id.as_str()))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}

/// The `(name, id)` artists of one `/Artists` reply.
fn artist_pairs(body: &serde_json::Value) -> Vec<(String, String)> {
    let items = body["Items"].as_array().map_or(&[][..], Vec::as_slice);
    items
        .iter()
        .filter_map(|item| Some((item["Name"].as_str()?, item["Id"].as_str()?)))
        .map(|(name, id)| (name.to_owned(), id.to_owned()))
        .collect()
}

impl App {
    /// The Service artist item id known for an artist name, if any.
    pub(in crate::app) fn gui_artist_item_id(&self, name: &str) -> Option<&str> {
        self.artist_name_item_ids
            .get(&normalized(name))
            .map(String::as_str)
    }

    /// Record the artist item id a track named for `name`. A name already known
    /// keeps its first id.
    pub(in crate::app) fn record_artist_item_id(&mut self, name: &str, id: String) {
        self.artist_name_item_ids
            .entry(normalized(name))
            .or_insert(id);
    }

    /// Start the artist search for `name` once: a name with an id, or already
    /// searched or in flight, starts nothing.
    pub(in crate::app) fn request_gui_artist_name_lookup(&mut self, name: &str) {
        let key = normalized(name);
        if key.is_empty()
            || self.artist_name_item_ids.contains_key(&key)
            || self.artist_name_lookups.contains_key(&key)
        {
            return;
        }
        let Some(client) = self.emby_client() else {
            return;
        };
        let (server_url, token, agent) = {
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone(), c.http_agent())
        };
        self.artist_name_lookups
            .insert(key, GuiArtistNameLookup::Loading);
        let tx = self.channels.lib_tx.clone();
        let name = name.to_owned();
        std::thread::spawn(move || {
            let url = format!(
                "{server_url}/Artists?SearchTerm={}&Limit={ARTIST_LOOKUP_LIMIT}&api_key={token}",
                mbv_net::encode_path_segment(&name)
            );
            let candidates = agent
                .get(&url)
                .call()
                .ok()
                .and_then(|response| response.into_body().read_json::<serde_json::Value>().ok())
                .map(|body| artist_pairs(&body));
            let _ = tx.send(LibEvent::Music(MusicEvent::ArtistNameSearched {
                name,
                candidates,
            }));
        });
    }

    /// Record one artist search's answer and queue the row fetches it unlocks.
    /// No answer (the request failed) forgets the name so a later pass retries:
    /// no answer is not no image (invariant 27).
    pub(in crate::app) fn handle_gui_artist_name_searched(
        &mut self,
        name: &str,
        candidates: Option<&[(String, String)]>,
    ) {
        let key = normalized(name);
        let Some(candidates) = candidates else {
            self.artist_name_lookups.remove(&key);
            return;
        };
        match best_artist_match(name, candidates) {
            Some(id) => {
                let id = id.to_owned();
                self.artist_name_lookups.remove(&key);
                self.record_artist_item_id(name, id);
            }
            None => {
                self.artist_name_lookups
                    .insert(key, GuiArtistNameLookup::Unmatched);
            }
        }
        self.gui_queue_in_view_row_fetches();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, id)| ((*name).to_owned(), (*id).to_owned()))
            .collect()
    }

    /// Contract: an equal name after case, punctuation, and article folding
    /// wins over a fuzzier candidate listed first.
    #[test]
    fn an_equal_normalized_name_wins() {
        let found = candidates(&[("Smiths Tribute Band", "1"), ("Smiths", "2")]);

        assert_eq!(best_artist_match("The Smiths", &found), Some("2"));
    }

    /// Contract: a fuzzy match must hold both ways, so a short folder name never
    /// takes a longer artist that merely contains it as a prefix word.
    #[test]
    fn a_name_that_matches_one_way_only_resolves_nothing() {
        let found = candidates(&[("Yes Sir", "1")]);

        assert_eq!(best_artist_match("Yes", &found), None);
    }
}
