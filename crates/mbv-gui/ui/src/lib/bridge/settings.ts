// The `settings` command (add-gui-settings-screen design D5): the Settings
// screen's window content sends one semantic request, and the bridge
// translates it into the core's neutral command. The wire shape is
// `GuiSettingsCommand` in bridge/settings.rs; the core applies it through
// `App::apply_gui_settings_command` and never reads a row position.

import { invoke } from '@tauri-apps/api/core';
import type { GuiSettingsCommand } from './types';

/** Sends one Settings command through the `settings` command (design D5). A
 * failed invoke — the ACL's denial of an ungranted command, or a broken
 * IPC — is logged to the webview console instead of vanishing, as the
 * `library` command does: the commands are fire-and-forget intents, and no
 * caller awaits the answer. */
export function sendSettingsCommand(
	command: GuiSettingsCommand,
): Promise<unknown> {
	return invoke('settings', { command }).catch((error: unknown) => {
		console.error(
			`the settings command ${command.command} failed`,
			command,
			error,
		);
	});
}
