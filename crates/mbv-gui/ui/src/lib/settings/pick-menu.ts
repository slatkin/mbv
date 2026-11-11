// The Settings pick menu (add-gui-settings-screen design D13): a pick row's
// value list, open over its pick button until the user chooses a value, presses
// outside, or presses Esc. The open menu is window content, not a core overlay,
// so the window holds it here and the Settings key router (./keys) reads it:
// Up and Down move its cursor, Enter sends `setValue`, and Esc closes it with
// no send. One menu is open at a time — a second pick button's press replaces
// the first menu — because the cursor and the settings name live in one fact.

import { get, writable } from 'svelte/store';
import { sendSettingsCommand } from '$lib/bridge/settings';

/** The open pick menu: the row's setting, its values, and the cursor. */
export interface PickMenuState {
	/** The row's setting name, which `setValue` carries back. */
	setting: string;
	/** The row's values, in the frame's order. */
	values: string[];
	/** The menu cursor, an index into `values`. */
	cursor: number;
}

/** The open pick menu, or `null` while every menu is closed. */
export const pickMenu = writable<PickMenuState | null>(null);

/** The painted text of one value: the empty value is the automatic choice. */
export function pickValueLabel(value: string): string {
	return value === '' ? 'Any' : value;
}

/** Open the row's value menu with the cursor on the value it shows now. */
export function openPickMenu(setting: string, value: string, values: string[]): void {
	const current = values.indexOf(value);
	pickMenu.set({ setting, values, cursor: current === -1 ? 0 : current });
}

/** Close the open menu with no change (Esc, a press outside, a second press on
 * the pick button). */
export function closePickMenu(): void {
	pickMenu.set(null);
}

/** Move the menu cursor one value, inside the list's ends. */
export function movePickMenuCursor(step: -1 | 1): void {
	pickMenu.update((menu) => {
		if (menu === null) {
			return menu;
		}
		const cursor = Math.min(Math.max(menu.cursor + step, 0), menu.values.length - 1);
		return { ...menu, cursor };
	});
}

/** Choose the value at `index`: the `setValue` command carries the row's
 * setting and that value, and the menu closes. An index outside the list
 * sends nothing. */
export function selectPickValue(index: number): void {
	const menu = get(pickMenu);
	const value = menu?.values[index];
	if (menu === null || value === undefined) {
		return;
	}
	void sendSettingsCommand({ command: 'setValue', setting: menu.setting, value });
	pickMenu.set(null);
}
