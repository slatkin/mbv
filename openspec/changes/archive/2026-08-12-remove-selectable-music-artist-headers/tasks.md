## 1. Remove Artist-Header State And Action Scope

- [x] 1.1 Delete artist-header focus state and selection types, then remove reset and synchronization paths that only maintained that state.
- [x] 1.2 Simplify current-item, playback, queue, and context-menu resolution so grouped music targets the focused track or selected album only.

## 2. Remove Artist-Header Interaction

- [x] 2.1 Exclude artist headers from grouped music navigation targets while preserving their display rows and album movement across group boundaries.
- [x] 2.2 Remove Ctrl+PageUp/PageDown artist-jump dispatch and helpers without changing unmodified paging.
- [x] 2.3 Make artist-header mouse rows inert while preserving album click and double-click behavior.

## 3. Simplify Rendering

- [x] 3.1 Remove selected-header styling and action hints while retaining artist headers as stable visual grouping labels.
- [x] 3.2 Remove dead header-focus branches and data flow from grouped album display planning, scrolling, and rendering.

## 4. Tests And Verification

- [x] 4.1 Delete tests that only assert selectable artist-header behavior and adjust existing grouped navigation, scope, and mouse coverage where needed to protect album selection across headers.
- [x] 4.2 Run formatting checks and the narrowest relevant application tests for grouped music navigation and action scope.
- [x] 4.3 Run `cargo clippy --workspace --all-targets` and `make check-code-file-lines`.
