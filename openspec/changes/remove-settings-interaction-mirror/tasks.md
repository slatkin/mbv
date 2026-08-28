## 1. Reshape the App-side handlers to take resolved values

- [x] 1.1 Change `App::activate_service_entry` (`src/app/services_settings.rs`)
  to take the resolved `ServiceEntry` (or the cursor as a plain parameter,
  doing the `SERVICE_ENTRIES.get()` lookup itself) instead of reading
  `self.services_cursor`. Update its one call site in
  `Model::handle_service_request`'s `ServiceRequest::ActivateService(cursor)`
  arm (`src/app/shell_settings.rs:168-172`) to resolve the entry and pass it
  in directly, deleting the `self.app.services_cursor = cursor;` write.
  Verify: `rtk cargo check -p mbv` passes with the new signature and its
  single call site.
- [x] 1.2 Change `App::handle_settings_activate` (in the `impl App` block
  currently in `src/app/render/components/settings.rs`, alongside the legacy
  painter it will move out of at task 3.1) to take the resolved `SettingKey`
  instead of reading `self.settings_cursor`. Update its one call site in
  `Model::handle_settings_intent`'s `SettingsIntent::Activate(cursor)` arm
  (`src/app/shell_settings.rs:236-239`) to call `settings_cursor_to_key(cursor)`
  at the call site and pass the resolved key in, deleting the
  `self.app.settings_cursor = cursor;` write. Verify: `rtk cargo check -p mbv`
  passes.

## 2. Move the services-cursor Back-reset into the component

- [x] 2.1 In `SettingsComponent` (`src/app/components/settings.rs`), reset the
  component's own `services_cursor = 0` at the point the component emits
  `Msg::Settings(SettingsIntent::Back)`, so the reset survives once
  `shell_settings.rs`'s `self.app.services_cursor = 0` (in the
  `SettingsIntent::Back` arm) is deleted in task 4. Verify: a component-level
  test (or an existing test extended) exercises leaving the Services
  destination via Back and asserts the component's `services_cursor` reads 0
  on the next Services entry.

## 3. Delete the legacy Settings/Services painter

- [x] 3.1 Delete `App::render_settings_panel` and `App::render_services_panel`
  (and their private helpers `render_emby_setup_panel` /
  `render_audiobookshelf_setup_panel` if they have no other caller — check
  before deleting) from `src/app/render/components/settings.rs`, keeping the
  `SettingsRenderModel` struct, `render_settings_content` function, and the
  free function `settings_cursor_to_key` (moved/kept as needed by task 1.2)
  which the live component render path and the reshaped
  `handle_settings_activate` still use. Verify: `rtk cargo check -p mbv`
  reports no more references to the deleted functions.
- [x] 3.2 Delete `src/app/render/tests_settings.rs` and its
  `#[path = "tests_settings.rs"]` registration at
  `src/app/render/mod.rs:205`. Verify: `rtk cargo check -p mbv` and
  `rtk cargo nextest run` both pass with the module gone.

## 4. Delete the three App fields and the outbound push

- [ ] 4.1 Delete `settings_cursor`, `services_cursor`, `settings_scroll` from
  `App` in `src/app/app_struct.rs` (currently lines 213, 215, 216). Delete the
  `self.app.services_cursor = 0` reset from `SettingsIntent::Back`
  (`src/app/shell_settings.rs:217-219`, superseded by task 2.1). Remove the
  `cursor: self.app.settings_cursor`, `services_cursor:
  self.app.services_cursor`, `scroll: self.app.settings_scroll` lines from
  `settings_snapshot()`'s `SettingsSnapshot` construction
  (`src/app/shell_settings.rs:121-123`) and the corresponding fields from
  `SettingsSnapshot` in `src/app/components/settings.rs` if they become
  unused by the component's own initialization (the component still needs
  *some* initial cursor/scroll value on first content push — confirm whether
  `SettingsSnapshot` should drop these fields entirely, defaulting the
  component to 0/0 on first `set_content()`, or keep them as component-only
  hints; resolve by reading `set_content()`'s `!self.initialized` branch and
  choosing the option that requires no `App` field). Verify: `rtk cargo check
  -p mbv` passes with the fields gone and no dangling references.
- [ ] 4.2 Open `App::open_services_settings`
  (`src/app/services_settings.rs:51`) and remove the now-defunct
  `self.services_cursor = self.services_cursor.min(SERVICE_ENTRIES.len() - 1)`
  clamp (the field no longer exists after task 4.1; the component clamps its
  own cursor against the pushed `services.len()` in `set_content()`). Verify:
  `rtk cargo check -p mbv` passes.

## 5. Verification gate

- [ ] 5.1 Run `rtk cargo check -p mbv`, `rtk cargo nextest run`,
  `rtk cargo clippy --workspace --all-targets`, and `rtk ast-grep scan`; all
  four must be green. Confirm via `rtk grep` that `settings_cursor`,
  `services_cursor`, and `settings_scroll` no longer appear anywhere under
  `src/app/app_struct.rs` or as `self.app.` reads/writes.
- [ ] 5.2 Update `docs/architecture/interactive-surface-ledger.md`'s Settings
  row (currently line 75) if its existing "App-free render seam" note needs
  amending now that the interaction-state fields are actually gone (it may
  already read correctly; check before editing).
