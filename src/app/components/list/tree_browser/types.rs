use crate::app::components::media_list::MediaSemanticState;
use ratatui::layout::Position;
/// Provider-neutral trailing metadata in a tree row's right gutter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeTrailing {
    pub text: String,
}

impl TreeTrailing {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Closed per-node marking policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeMarkPolicy {
    /// The node itself can be included in the ordered mark set.
    #[default]
    Direct,
    /// The node displays an aggregate over directly markable descendants.
    Aggregate,
    /// The node is excluded from mark membership and aggregation.
    Excluded,
}

/// Plain destination data for one tree row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode<Target> {
    pub target: Target,
    pub parent: Option<Target>,
    pub title: String,
    pub search_text: String,
    pub trailing: Option<TreeTrailing>,
    pub semantic_state: MediaSemanticState,
    pub mark_policy: TreeMarkPolicy,
}

impl<Target> TreeNode<Target> {
    pub fn new(
        target: Target,
        parent: Option<Target>,
        title: impl Into<String>,
        search_text: impl Into<String>,
        semantic_state: MediaSemanticState,
        mark_policy: TreeMarkPolicy,
    ) -> Self {
        Self {
            target,
            parent,
            title: title.into(),
            search_text: search_text.into(),
            trailing: None,
            semantic_state,
            mark_policy,
        }
    }

    pub fn with_trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = Some(TreeTrailing::new(trailing));
        self
    }
}

/// The result disposition shared by semantic tree operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeConsumed {
    Unhandled,
    Consumed,
}

/// Stable-target external intent emitted by a tree operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeExternalIntent<Target> {
    Activate(Target),
    Context(Target),
    ContextSelection(Vec<Target>),
}

/// Stable selection change reported by a tree operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeSelectionChange<Target> {
    pub previous: Option<Target>,
    pub current: Option<Target>,
}

/// Provider-neutral summary of ordered marks and aggregate presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeMarkSummary {
    pub marked_count: usize,
    pub has_partial_aggregate: bool,
}

/// Closed semantic operation vocabulary.  Behaviour is implemented by the
/// owner in the operations unit; keeping the vocabulary here prevents a
/// destination from inventing a second mutation surface.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeOperation<Target> {
    Move(i64),
    Page(i64),
    First,
    Last,
    Parent,
    Child,
    ToggleExpansion,
    /// Toggle one node's persistent expansion without changing selection.
    /// A childless target is an explicit `Unhandled` result.
    ToggleExpansionTarget(Target),
    /// Restore a persisted position: select `target`, reveal its ancestor
    /// path, and anchor the viewport at the persisted settled-flow
    /// `flow_offset`. A target the owner does not hold is an explicit
    /// `Unhandled` result.
    AnchorSelection {
        target: Target,
        flow_offset: usize,
    },
    Select(Target),
    ToggleMark,
    ToggleMarkTarget(Target),
    PointerSelect(Position),
    PointerToggleMark(Position),
    Activate,
    ActivateTarget(Target),
    Context,
    ContextTarget(Target),
    EditFilter(String),
    ClearFilter,
    /// Clear the ordered mark set.
    ClearMarks,
}

/// One complete operation result.  The independent fields let a caller
/// observe selection, marks, and external intent without replaying owner logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeTransition<Target> {
    pub disposition: TreeConsumed,
    pub selected_target: Option<Target>,
    pub selected_target_change: Option<TreeSelectionChange<Target>>,
    pub mark_summary: Option<TreeMarkSummary>,
    pub mark_summary_change: Option<TreeMarkSummary>,
    pub external_intent: Option<TreeExternalIntent<Target>>,
}
