// The `library` command (add-gui-library-panel design D2): the Library panel's
// window content sends one semantic request, and the bridge translates it into
// the core's neutral command. The wire shape is `GuiLibraryCommand` in
// bridge/library.rs; the core applies it through `App::apply_gui_library_command`
// and never reads a row position.

import { invoke } from '@tauri-apps/api/core';
import type { GuiLibraryCommand } from './types';

/** Sends one Library command through the `library` command (design D2). A
 * failed invoke — the ACL's denial of an ungranted command, or a broken
 * IPC — is logged to the webview console instead of vanishing: the live
 * finding that muted every Library command failed exactly here, silently,
 * because the callers fire the command and move on. The failure stays
 * non-throwing: the commands are fire-and-forget intents, and no caller
 * awaits the answer. */
export function sendLibraryCommand(
	command: GuiLibraryCommand,
): Promise<unknown> {
	return invoke('library', { command }).catch((error: unknown) => {
		console.error(
			`the library command ${command.command} failed`,
			command,
			error,
		);
	});
}
