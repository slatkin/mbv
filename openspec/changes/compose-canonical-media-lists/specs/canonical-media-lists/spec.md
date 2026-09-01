## Purpose

Defines the shared fixed-row Wide media list and selected-row-replacement Inline media browser so each named primary destination receives the same list behavior while retaining provider-specific content and effects.

## ADDED Requirements

### Requirement: Wide media lists use one fixed-row control

Every hero-bearing primary media browser SHALL use `WideMediaList` when it appears in a Hero-on-left right rail. The control SHALL remain one column regardless of available rail width and SHALL own its live cursor, scroll offset, viewport, previous/next/page/home/end movement, selection visibility clamp, row placement, scrollbar, semantic selected/focused/active treatment, trailing metadata placement, truncation, and render-derived item geometry.

`WideMediaList` SHALL NOT govern non-hero catalog browsers whose existing contract permits two columns.

#### Scenario: A Wide rail becomes very wide

- **WHEN** a Hero-on-left right rail grows beyond the ordinary multi-column threshold
- **THEN** its `WideMediaList` remains one column
- **AND** its headings, items, selection indicator, trailing metadata, and scrollbar retain the shared placement

#### Scenario: A non-hero browser becomes wide

- **WHEN** a non-hero catalog browser covered by the existing two-column contract reaches its column breakpoint
- **THEN** that browser may retain its existing two-column presentation
- **AND** this capability does not require it to compose `WideMediaList`

#### Scenario: Movement reaches a new item

- **WHEN** the user moves, pages, goes Home, or goes End in a `WideMediaList`
- **THEN** the control resolves the destination against its selectable rows
- **AND** it updates its live cursor and scroll so the selected item remains visible
- **AND** any cross-boundary request carries the resolved target rather than the original movement delta

#### Scenario: A Wide list is unfocused

- **WHEN** its parent destination or another Hero-on-left pane owns focus
- **THEN** the list retains its selected target and scroll
- **AND** it uses the shared unfocused semantic treatment without changing row geometry

### Requirement: Inline media browsers own selected-row replacement

Every named hero-bearing primary media browser that does not meet the shared Wide geometry conditions SHALL use `InlineMediaBrowser`. The term is distinct from Inline Search. The control SHALL remain one column and replace the selected item's ordinary row with one variable-height Inline hero at the same flow position. It SHALL budget replacement height once, keep the replacement visible through row-based scrolling, and restore the ordinary row when detail cannot fit.

#### Scenario: Selected detail fits

- **WHEN** the selected item's Inline hero fits in the available browser viewport
- **THEN** it replaces that item's ordinary row at the same flow position
- **AND** subsequent rows continue after the replacement without a duplicate selected row
- **AND** the scrollbar accounts for the replacement height exactly once

#### Scenario: Selected detail does not fit

- **WHEN** the selected item's Inline hero cannot fit in the available viewport
- **THEN** `InlineMediaBrowser` renders the ordinary selected row instead
- **AND** it does not reserve blank hero space or paint a partial replacement

#### Scenario: Inline detail has structured children

- **WHEN** a selected TV, Music, Podcast, or Audiobookshelf item exposes seasons, episodes, tracks, or chapters
- **THEN** the Inline hero uses the shared title, metadata, overview, and optional-image shape
- **AND** structured children remain outside the Inline hero and are opened through the shared selection modal

### Requirement: Media-list rows use a closed provider-neutral vocabulary

The canonical media-list presentation SHALL contain selectable item rows and non-selectable heading and spacer rows. An item SHALL carry an opaque stable target, display text, optional trailing metadata, and semantic item state. Provider identity, Service clients, effects, source URLs, renderer callbacks, arbitrary style values, and destination-specific geometry SHALL NOT be part of the row contract.

The active Queue state MAY carry a prepared `progress_percent` payload bounded to 0 through 100. This value is presentation data only; the Queue parent and shell retain playback and Player authority.

#### Scenario: A grouped list is displayed

- **WHEN** Music artist groups or Feed date groups are projected into a canonical media list
- **THEN** group labels use heading rows and visual separation uses spacer rows
- **AND** movement and pointer resolution skip those structural rows

#### Scenario: Queue displays its active item

- **WHEN** Queue composes the canonical fixed-row control and one slot is currently playing
- **THEN** the slot is an ordinary item row with active semantic state and an optional bounded progress percentage
- **AND** Queue does not require a separate implementation of shared cursor, scroll, truncation, selection, or scrollbar behavior
- **AND** playback authority remains outside the reusable control

#### Scenario: Provider content changes

- **WHEN** a parent destination refreshes or replaces the rows projected into a canonical control
- **THEN** the control preserves its selected stable target when that target remains present
- **AND** otherwise it clamps or resets its own cursor without adopting a mirrored shell cursor

### Requirement: Parent destinations retain provider and workspace authority

A destination parent SHALL provide canonical row and Inline hero presentation data, map opaque targets to provider-specific requests, and own its pills, hero or detail workspace, provider-local filters, image preparation, and focus policy. The shell SHALL retain Service, Player, persistence, navigation, image-fetch, and external-effect authority. The reusable controls SHALL perform no external effect while handling input or painting.

#### Scenario: A selected row is activated

- **WHEN** the user activates a selectable canonical row
- **THEN** the reusable control returns the resolved opaque target to its destination parent
- **AND** the parent emits the provider-specific typed request
- **AND** the shell performs the effect without recomputing the list movement or reading a mirrored cursor

#### Scenario: A hero image is required

- **WHEN** the selected row's Wide or Inline detail requires an image
- **THEN** the parent supplies owned presentation data and an image key or prepared paint request
- **AND** the reusable control neither fetches the image nor receives provider or cache authority

### Requirement: Presentation changes preserve one live list selection

Crossing between Wide and Inline presentations SHALL preserve one live selected target and a defined viewport anchor. The viewport anchor is the zero-based screen-row offset from the top of the list viewport to the top of the selected item's ordinary row before replacement. A destination SHALL NOT maintain independent Wide and Narrow live cursors that can diverge, paint both list presentations in one frame, or copy paint-derived scroll into shell state on every frame.

#### Scenario: A destination crosses the shared breakpoint

- **WHEN** a destination changes from Hero-on-left to Inline or back
- **THEN** the same selected target remains active when it still exists
- **AND** the new control keeps the selected ordinary row at the prior viewport-row offset when available, otherwise clamps that offset into its viewport
- **AND** exactly one canonical list presentation paints the destination browser
- **AND** the new presentation does not duplicate the selected detail

### Requirement: Named primary destinations compose the canonical controls

The following surfaces SHALL compose the applicable canonical control: Home; the hero-bearing generic Emby library catalog browser; Movies; TV Series browsing; grouped Music album browsing; the Emby homevideos feed view; the Emby podcast channel list; Audiobookshelf Podcast show browsing; Audiobookshelf Book browsing; Feeds; and Queue's fixed-row list.

Queue SHALL compose fixed-row behavior only. Hero-on-left, `InlineMediaBrowser`, Inline hero, and responsive Wide/Inline handoff requirements are not applicable to Queue. A destination-specific list implementation SHALL exist only as a named bespoke exception with a documented structural reason and focused verification.

#### Scenario: Wide Feeds is rendered

- **WHEN** Feeds meets the shared Wide geometry conditions
- **THEN** its selected entry hero renders on the left
- **AND** its group and watched selectors precede one canonical single-column list in the right rail

#### Scenario: Wide Audiobookshelf Books is rendered

- **WHEN** Audiobookshelf Books meets the shared Wide geometry conditions
- **THEN** its persistent book detail workspace renders on the left
- **AND** the right rail contains ordinary fixed-height book rows
- **AND** the selected row is not replaced by a second Inline hero

#### Scenario: Queue is rendered

- **WHEN** Queue displays its Local or Remote fixed-row list at any panel width
- **THEN** it composes the shared fixed-row behavior
- **AND** it does not enter Hero-on-left or selected-row-replacement presentation

#### Scenario: A destination requests an exception

- **WHEN** a named destination cannot express its list through item, heading, spacer, and semantic item state
- **THEN** the difference is recorded as a named bespoke surface with its reason and verification
- **AND** the exception does not duplicate canonical behavior that its rows can express
