## REMOVED Requirements

### Requirement: Enter opens a constituent-list modal on inline-hero surfaces

**Reason**: Non-Wide browsing no longer has Inline hero surfaces; Enter opens the Library Hero overlay containing the existing Workspace instead.

**Migration**: Open the Library Hero overlay and interact with its canonical Workspace media list.

### Requirement: The constituent-list modal supports item selection and cancellation

**Reason**: The Library Hero overlay's persistent Workspace replaces the one-shot constituent picker and remains open after child activation.

**Migration**: Preserve provider loading, empty, refresh, stable-target, and activation behavior in the existing Workspace owner presented by the Library Hero overlay.

### Requirement: The modal uses the shared modal-frame presentation

**Reason**: The replacement is a Library-local non-exclusive overlay, not a centered application modal, because Queue must remain independently operable.

**Migration**: Use the Library Hero overlay's Library-confined frame, dimming, focus, and hit geometry instead of the shared blocking modal frame.
