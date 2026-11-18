//! The GUI Library's one collection-command path (gui-collection-dropdown
//! design D5): the destination-neutral op and rejection vocabulary, the
//! per-destination site trait, and the shared apply and selector build the
//! four destinations' sites plug into. The sites live beside their
//! destination's store and side effects (`store.rs`, `podcasts.rs`,
//! `feeds.rs`, `emby.rs`); this module holds what they share.

use super::collection::{
    GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet, GuiCollectionTarget,
    GuiCollections, GuiDateOrder,
};
use super::{
    GuiEmbyFeedCollectionSite, GuiFeedsCollectionSite, GuiMusicCollectionSite,
    GuiPodcastsCollectionSite,
};
use crate::app::App;

/// The op one collection command carries (gui-collection-dropdown design D5):
/// the three dropdown commands, destination-neutral.
pub(in crate::app::gui_library) enum CollectionOp {
    /// Check or uncheck the collection `key` names.
    Toggle { key: String },
    /// Select All: the All row's press and the dropdown's Clear.
    All,
    /// Set the list's date order.
    Order(GuiDateOrder),
}

impl CollectionOp {
    /// The name the op's reject logs (design D5): the command's own name.
    fn name(&self) -> &'static str {
        match self {
            Self::Toggle { .. } => "collectionToggle",
            Self::All => "collectionAll",
            Self::Order(_) => "dateOrder",
        }
    }
}

/// Why a collection command changes nothing (design D5, D6): the log form the
/// destination's rule selects.
pub(in crate::app::gui_library) enum CollectionRejection {
    /// The command arrived, but the destination or its catalog rule rejects
    /// it: the ignored log names the reason.
    Ignored(String),
    /// A stale frame's key: the stale-item log names it.
    Stale(String),
}

/// One collection destination's command site (gui-collection-dropdown design
/// D5): the destination's own reject, staleness, catalog, and install steps
/// the one shared collection path calls. The Music, Podcasts, Feeds, and Emby
/// feed-group sites live beside their destination's store and side effects.
pub(in crate::app::gui_library) trait GuiCollectionSite {
    /// The noun the destination's selector reads (design D6).
    fn noun(&self) -> GuiCollectionNoun;

    /// Whether the destination stores a date order (design D4): Music does
    /// not, so an Order command there is a no-op.
    fn has_order(&self) -> bool;

    /// The catalog the retention retains against (design D1): `None` while
    /// the catalog has not landed — the list that would name a key stale —
    /// which keeps the stored set as it is.
    fn present(&self, app: &App) -> Option<Vec<String>>;

    /// The destination's scopes: each collection's stable key and label, in
    /// catalog order (design D6).
    fn scopes(&self, app: &App) -> Vec<(String, String)>;

    /// The destination's stored selection.
    fn selection(&self, app: &App) -> GuiCollectionSelection;

    /// Why the destination cannot serve a collection command at all (design
    /// D5, D6).
    fn reject_destination(&self, app: &App) -> Option<CollectionRejection>;

    /// Why the collection `key` cannot apply (design D5, D6): a stale frame's
    /// key, or a destination-level rule a toggle alone runs.
    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection>;

    /// The destination's own log for one toggle (design D5): Music logs the
    /// selected collection's change; the other destinations log none.
    fn log_toggle(&self, _key: &str) {}

    /// Write the set back, starting the destination's own loads (design D2,
    /// D5).
    fn install_set(&self, app: &mut App, set: GuiCollectionSet);

    /// Write the order back (design D4): called only when [`Self::has_order`].
    fn install_order(&self, app: &mut App, order: GuiDateOrder);

    /// The effective selection the destination paints (design D1): the stored
    /// set retained against the catalog.
    fn effective_set(&self, app: &App) -> GuiCollectionSet {
        self.selection(app)
            .set
            .effective(self.present(app).as_deref())
    }

    /// The selector's order (design D4): the stored order where the
    /// destination stores one, `None` where it does not (Music sorts by
    /// name).
    fn order(&self, app: &App) -> Option<GuiDateOrder> {
        self.has_order().then(|| self.selection(app).order)
    }
}

impl App {
    /// Apply one collection command for `target` (gui-collection-dropdown
    /// design D5): the one shared path — reject, stale check, toggle/All/order
    /// over the effective set, and the destination's own install — that the
    /// four destinations' sites plug into, so the toggle, All, and order steps
    /// cannot drift. The match names each destination's site; everything else
    /// is shared.
    pub(in crate::app::gui_library) fn apply_collection(
        &mut self,
        target: &GuiCollectionTarget,
        op: CollectionOp,
    ) {
        match target {
            GuiCollectionTarget::Music(lib_idx) => {
                self.apply_collection_at(&GuiMusicCollectionSite(*lib_idx), op);
            }
            GuiCollectionTarget::Podcasts(lib_idx) => {
                self.apply_collection_at(&GuiPodcastsCollectionSite(*lib_idx), op);
            }
            GuiCollectionTarget::Feeds => self.apply_collection_at(&GuiFeedsCollectionSite, op),
            GuiCollectionTarget::EmbyFeed(lib_idx) => {
                self.apply_collection_at(&GuiEmbyFeedCollectionSite(*lib_idx), op);
            }
        }
    }

    /// The one collection path over one destination's site (design D5): the
    /// op's reject rule names the log line; a toggle reads the effective set
    /// and installs the change, All installs All, and an order writes the
    /// stored order where the destination stores one.
    fn apply_collection_at(&mut self, site: &impl GuiCollectionSite, op: CollectionOp) {
        let log_name = op.name();
        let rejection = match &op {
            CollectionOp::Toggle { key } => match site.reject_destination(self) {
                Some(rejection) => Some(rejection),
                None => site.reject_key(self, key),
            },
            CollectionOp::All | CollectionOp::Order(_) => site.reject_destination(self),
        };
        if let Some(rejection) = rejection {
            match rejection {
                CollectionRejection::Ignored(reason) => {
                    Self::log_ignored_library_command(log_name, format_args!("{reason}"));
                }
                CollectionRejection::Stale(key) => {
                    Self::log_stale_library_item(log_name, &key);
                }
            }
            return;
        }
        match op {
            CollectionOp::Toggle { key } => {
                site.log_toggle(&key);
                let next = site
                    .selection(self)
                    .set
                    .effective(site.present(self).as_deref())
                    .toggle(&key);
                site.install_set(self, next);
            }
            CollectionOp::All => site.install_set(self, GuiCollectionSet::All),
            CollectionOp::Order(order) => {
                if site.has_order() {
                    site.install_order(self, order);
                }
                // A destination that stores no order (Music sorts by name)
                // takes no Order command and logs none: its dropdown paints no
                // order toggle.
            }
        }
    }

    /// The destination's collection selector (gui-collection-dropdown design
    /// D6): the one build every projection reads, so the four frames cannot
    /// drift.
    pub(in crate::app::gui_library) fn gui_collections(
        &self,
        target: GuiCollectionTarget,
    ) -> GuiCollections {
        match &target {
            GuiCollectionTarget::Music(lib_idx) => {
                self.site_collections(&GuiMusicCollectionSite(*lib_idx), target)
            }
            GuiCollectionTarget::Podcasts(lib_idx) => {
                self.site_collections(&GuiPodcastsCollectionSite(*lib_idx), target)
            }
            GuiCollectionTarget::Feeds => self.site_collections(&GuiFeedsCollectionSite, target),
            GuiCollectionTarget::EmbyFeed(lib_idx) => {
                self.site_collections(&GuiEmbyFeedCollectionSite(*lib_idx), target)
            }
        }
    }

    /// The selector one site builds (design D6): the effective set over the
    /// destination's catalog, its noun, its scopes in catalog order, and its
    /// order.
    fn site_collections(
        &self,
        site: &impl GuiCollectionSite,
        target: GuiCollectionTarget,
    ) -> GuiCollections {
        let set = site.effective_set(self);
        GuiCollections::new(
            target,
            &set,
            site.noun(),
            site.scopes(self),
            site.order(self),
        )
    }
}
