// The `library` command (add-gui-library-panel design D2): the Library panel's
// window content sends one semantic request, and the bridge translates it into
// the core's neutral command. The wire shape is `GuiLibraryCommand` in
// bridge/library.rs; the core applies it through `App::apply_gui_library_command`
// and never reads a row position.

import { invoke } from '@tauri-apps/api/core';
import type { GuiLibraryCommand } from './types';

/** Sends one Library command through the `library` command (design D2). */
export function sendLibraryCommand(
	command: GuiLibraryCommand,
): Promise<unknown> {
	return invoke('library', { command });
}
