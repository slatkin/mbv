# Remove Settings/Services Interaction-State Mirror

## Why

`SettingsComponent` already owns `cursor`, `services_cursor`, and `scroll` as
its own local interaction state, computed from its own key handling. But
`App` still carries the same three values (`settings_cursor`, `services_cursor`,
`settings_scroll`) and the round trip is fully closed: the component emits its
local cursor in a request, the shell writes it into `App`, and an `App`-side
handler re-reads that same `App` field a moment later to do the very thing the
component already knew when it emitted the request. `App` is not supposed to
be a second interaction-state store per the `interactive-component-framework`
spec's "shell Model retains runtime authority" requirement, and this mirror is
the last place Settings/Services violates it. It is issue #615, slice 1 of 4
for #611 ("Remove two-way interaction-state mirrors"), picked first because it
is the smallest and most self-contained of the four — proving the pattern the
TV workspace (#616), Queue (#617), and Emby browser (#618) slices then follow.

## What Changes

- `ServiceRequest::ActivateService(cursor)` handling in
  `Model::handle_service_request` no longer writes `self.app.services_cursor`
  before calling `App::activate_service_entry()`. `activate_service_entry`
  takes the resolved `ServiceEntry` (or the cursor as a plain parameter) instead
  of re-reading `self.services_cursor`.
- `SettingsIntent::Activate(cursor)` handling in `Model::handle_settings_intent`
  no longer writes `self.app.settings_cursor` before calling
  `App::handle_settings_activate()`. `handle_settings_activate` takes the
  resolved `SettingKey` instead of re-reading `self.settings_cursor`.
- `SettingsIntent::Back` no longer resets `self.app.services_cursor = 0`; the
  services cursor reset becomes purely component-local, driven by the
  destination change the component already observes in `set_content()`.
- The legacy `App::render_settings_panel` / `App::render_services_panel`
  painter in `src/app/render/components/settings.rs` — reachable only from
  `src/app/render/tests_settings.rs`, with no production caller — is deleted
  along with that test module, removing the last reader that could have
  blocked field deletion.
- `App::settings_cursor`, `App::services_cursor`, and `App::settings_scroll`
  are deleted from `app_struct.rs`. `SettingsSnapshot`'s outbound cursor/scroll
  fields in `shell_settings.rs` are dropped from the push (the snapshot keeps
  pushing validated *content* — service rows, setting values, setup drafts —
  which is sanctioned and unaffected).

**Not changing:** the shell→component content push (service list, setting
values, setup draft) stays exactly as it is; only the two-way cursor/scroll
pin is removed. No daemon, ctrl, Service, or playback behavior changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `interactive-component-framework`: clarifies the existing "shell Model
  retains runtime authority" requirement with an explicit rule that a
  component's own cursor/scroll/selection MUST NOT be written into a shell
  field for the sole purpose of being read back by a shell-side handler, and
  removes the Settings/Services instance of that pattern as the concrete
  scenario.

## Impact

- `src/app/shell_settings.rs` — `handle_service_request`,
  `handle_settings_intent`, `settings_snapshot`.
- `src/app/services_settings.rs` — `activate_service_entry` (and its caller),
  `open_services_settings` if it also needs the resolved-value shape.
- `src/app/render/components/settings.rs` — deletion of the legacy
  `render_settings_panel` / `render_services_panel` painter and
  `settings_cursor_to_key` call site within it (the free function itself, used
  by the reshaped `handle_settings_activate`, is retained).
- `src/app/render/tests_settings.rs` and its `#[path]` registration in
  `src/app/render/mod.rs` — deleted.
- `src/app/app_struct.rs` — three field deletions.
- `src/app/components/settings.rs` — `SettingsIntent::Back` component-local
  handling (if not already covered by the existing `destination_changed`
  reset path; confirmed during design).
- No wire, persistence, or ctrl protocol changes.
