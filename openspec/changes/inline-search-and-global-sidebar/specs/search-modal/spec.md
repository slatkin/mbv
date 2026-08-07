## REMOVED Requirements

### Requirement: Search renders as a modal, not as a filtered library list

**Reason**: The modal surface is being withdrawn. Fuzzy search returns to filtering the library list in place, and global search moves to a side panel; neither is a centered modal over a dimmed backdrop.

**Migration**: Replaced by `inline-library-search` — "The search key opens an inline input box above the library list" and "Results render as a flat list on every library type", and by `global-search-sidebar` — "The sidebar occupies the panel slot".

### Requirement: Modal contains no images

**Reason**: There is no modal. The no-images rule survives only for the global search results, which are now sidebar rows.

**Migration**: Replaced by `global-search-sidebar` — "Results render as plain single-row items", which forbids images and image fetches for result rows.

### Requirement: Two search modes over one state

**Reason**: The two searches are being split back into independent features with separate state, separate key bindings, and separate render surfaces. There is no shared mode enum left to switch between.

**Migration**: Fuzzy behavior moves to `inline-library-search`; server-side behavior moves to `global-search-sidebar` — "Queries are debounced and dispatched once they reach two characters".

### Requirement: Fuzzy search covers the whole library at any depth

**Reason**: Restated against the inline search surface rather than the modal.

**Migration**: Replaced by `inline-library-search` — "The corpus spans the whole library, not the visible page", which carries the same pagination, letter-filter, and loading guarantees.

### Requirement: Fuzzy search works on every library type

**Reason**: Restated against the inline search surface rather than the modal. The modal met this bar by not using the library list at all; the inline restore must meet it while sharing that list.

**Migration**: Replaced by `inline-library-search` — "Results render as a flat list on every library type", which keeps the grouped-music and letter-header scenarios verbatim.

### Requirement: A second search key press promotes fuzzy to global

**Reason**: The promotion gesture existed only because both searches shared one surface. With separate key bindings for separate surfaces, it has nothing to promote.

**Migration**: Use the dedicated global search chord instead — `global-search-sidebar` — "A dedicated key opens the global search sidebar from any tab". The inline search key is now always literal once search is open (`inline-library-search` — "Typing edits the query and re-filters the list in place").

### Requirement: Search from the home tab opens global search

**Reason**: The inline search key no longer has a global fallback to reach from the home tab.

**Migration**: The inline search key is now a no-op on home (`inline-library-search` — "The search key opens an inline input box above the library list"); global search is reached from home with the dedicated chord (`global-search-sidebar` — "A dedicated key opens the global search sidebar from any tab").

### Requirement: Results render as a flat list with an inline hero

**Reason**: The inline hero block was the modal's answer to having no detail pane. The sidebar deliberately drops it in favor of plain single-row results.

**Migration**: Replaced by `global-search-sidebar` — "Results render as plain single-row items". The flat-list part of this requirement also survives in `inline-library-search` — "Results render as a flat list on every library type".

### Requirement: Results are differentiated by item type

**Reason**: The per-type meta lines and the parent-item column belonged to the two-row modal row layout, which the single-row sidebar does not have.

**Migration**: The type badge and the name-only matching rule survive in `global-search-sidebar` — "Results render as plain single-row items". The per-type meta composition and the promotion-stability scenario are dropped with the row layout and the promotion gesture.

### Requirement: Type filtering is available in global mode only

**Reason**: There is no mode to gate on. Type filtering belongs to the sidebar unconditionally, and inline search has no filter chips at all.

**Migration**: Replaced by `global-search-sidebar` — "Results can be narrowed to a single item type".

### Requirement: Activating a result navigates to it

**Reason**: Restated against the sidebar. Inline search activates items in the library it is already filtering, which is not cross-library navigation.

**Migration**: Replaced by `global-search-sidebar` — "Activating a result navigates to it and closes the sidebar", and by `inline-library-search` — "Results are navigable and activatable without leaving search".

### Requirement: Modal styling matches the application palette

**Reason**: The modal's palette assignments described a surface that no longer exists. The sidebar takes its styling from the shared panel frame instead of specifying its own.

**Migration**: Replaced by `global-search-sidebar` — "The sidebar occupies the panel slot", which requires the standard panel frame, title row, and hint footer.

### Requirement: Dimmed backdrops render images in halfblocks

**Reason**: This requirement was never about search — it governs every overlay that dims its backdrop, and the behavior it describes is unchanged by this change. It only lived here because the search modal was the change that introduced it.

**Migration**: Relocated verbatim to the new `dimmed-backdrop-images` capability. No behavior change; the implementation is untouched.

### Requirement: Dismissing search restores the previous view

**Reason**: Restated separately for each surface, since dismissal now means two different things.

**Migration**: Replaced by `inline-library-search` — "Dismissing search restores the unfiltered list", and by `global-search-sidebar` — "Dismissing the sidebar leaves the underlying view untouched".
