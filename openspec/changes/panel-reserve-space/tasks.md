# Tasks

## 1. Config key

- [x] 1.1 Add `reserve: bool` to `PanelConfig` (default `true`), parse `[panel] reserve` in `crates/mbv-config/src/parse.rs` (malformed falls back to `true` with a logged warning), and write it in `save.rs`; verify with the existing config parse/save tests plus a malformed-value case.

## 2. pinwin argv

- [x] 2.1 In `src/pin.rs`, pick the `--reserve` value in `run_argv` from `config.reserve` (`push`/`cover`) and map `PinnedWidth::Collapsed` in `pinned_width_argv` to `Push` when `reserve` is true and `Cover` when false; verify `src/pin/tests.rs` covers `run` and collapsed `set-size` argv for both values.
- [x] 2.2 Add `PinnedSettingRequest::Reserve(bool)` with `edited_width` returning `None` and a `set_reserve_argv` builder (`pinwin set-reserve <panel> push|cover`), dispatched in `spawn_pinned_request`; verify the argv test asserts the `set-reserve` command line for both values. Whether the request is sent at all is dispatch's call (task 3.2).

## 3. F2 row and dispatch

- [x] 3.1 Add `SettingKey::PanelReserve` in `crates/mbv-ui-model/src/settings.rs`: label `Reserve space`, an on/off toggle inserted in the Panel row order list between `PanelWidthExpanded` and `PanelGutterTop`, value text from `cfg.panel.reserve`, and `changed_panel_config` flips it; verify the settings tests cover the row's label, value, and toggle.
- [x] 3.2 In `src/app/dispatch/settings.rs`, give `pinned_setting_request` the active width (`self.pinned_width`) and map `SettingKey::PanelReserve` to `Some(PinnedSettingRequest::Reserve(candidate.reserve))` only when collapsed — `None` while expanded, the same no-request path the accent rows take. In `apply_panel_setting_outcome`, flash a neutral toast for a saved disable while pinned (the held strip is released on the next `mbv --pin` launch) and for a saved enable while `self.pinned_width` is `Expanded` (the strip is reserved on the next collapse); verify the dispatch tests cover enable-collapsed, disable-with-toast, and expanded-no-request.

## 4. Docs

- [ ] 4.1 Rewrite the `Panel covering` entry in `CONTEXT.md` (its "The collapsed panel always pushes … There is no setting for either." sentence would contradict the new option): collapsed pushes and reserves the strip when `reserve` is on and covers when it is off, expanded always covers, a change while expanded applies on the next collapse, and a held strip releases only on the next `mbv --pin` launch; verify `openspec validate panel-reserve-space --strict` passes.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- When slatkin/pinwin#59 lands, file a follow-up change that swaps the disable path's argv to the release form.
