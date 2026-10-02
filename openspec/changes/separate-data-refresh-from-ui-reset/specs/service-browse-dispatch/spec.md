# Spec Delta

## MODIFIED Requirements

### Requirement: Refresh targets the selected browse destination

The global data-refresh action (F5 by default) SHALL invoke the refresh behavior belonging to the selected browse destination, regardless of Panel focus or whether the Library panel is visible. Home SHALL reload Home content. An Emby library SHALL reload that library's current browse scope and its existing destination-specific refresh sources. An Audiobookshelf library SHALL re-request its catalog and the detail/shelf data required by its current view. Feeds SHALL refetch subscribed feeds. F5 SHALL NOT dispatch a Queue refresh or initiate queue or playback mutations. Existing blocking-overlay input precedence SHALL remain in force.

Refresh SHALL preserve valid destination context: selected tab and main Selector pill, filters and sort, browse depth, tree expansion, selected item and Workspace item, local focus, scroll/viewport, multi-selection, open Inline Search and Library Hero overlay, Panel mode and resized widths. Unrelated destinations SHALL retain their UI state. Loading SHALL NOT publish an artificial empty catalog that resets otherwise valid context. Successful results SHALL update content in place by stable identity; when a refreshed identity really disappears, the existing destination-specific fallback/clamping rules SHALL apply. A failed fetch SHALL retain previously loaded content for the failed scope and report the failure. Refresh SHALL NOT clear saved presentation state, write launch state or reset Latest acknowledgements.

#### Scenario: User refreshes a browse destination
- **WHEN** Home, an Emby library, an Audiobookshelf library, or Feeds is selected and the user invokes F5 with Library focus
- **THEN** mbv SHALL refresh only that selected destination through its existing Service-specific data sources
- **AND** its still-valid browsing context SHALL remain intact

#### Scenario: User refreshes the queue panel
- **WHEN** Queue holds Panel focus and the user invokes F5
- **THEN** mbv SHALL refresh the selected browse destination rather than the visible queue
- **AND** Queue focus, content, source, selection and playback SHALL remain unchanged by the action

#### Scenario: Library is not painted
- **WHEN** the Library panel is hidden by the current Panel mode and the user invokes F5
- **THEN** the retained selected browse destination SHALL still be refreshed
- **AND** the action SHALL NOT reveal the Library or change Panel mode or focus

#### Scenario: Paged Audiobookshelf refresh retains a valid selection
- **WHEN** a selected Audiobookshelf book or podcast episode remains in the refreshed scope but arrives after the first catalog page or episode result
- **THEN** mbv SHALL retain the current selection and pill while the relevant refresh is loading
- **AND** it SHALL NOT treat a partial result as proof that the selected identity is gone

#### Scenario: Content disappears or the refresh fails
- **WHEN** a successful refresh establishes that the selected item no longer belongs to the current view
- **THEN** mbv SHALL apply that destination's existing valid-item or empty-state fallback without globally resetting the UI
- **WHEN** the fetch fails instead
- **THEN** mbv SHALL report the failure and keep the previously loaded content and context for that failed scope

#### Scenario: A blocking overlay is active
- **WHEN** a blocking overlay owns input and the user presses F5
- **THEN** the existing blocking-overlay policy SHALL suppress the refresh
- **AND** global F5 SHALL NOT bypass that overlay's input boundary
