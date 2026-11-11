// The Settings screen's key routing (add-gui-settings-screen design D11):
// `routeLibraryKey` hands every keydown here while the frame carries the
// Settings screen, and this module is the screen's one routing site — the
// components press through the store's functions, never a second router.
// The order is the design's: an active capture takes every key (Esc cancels,
// any other key is the candidate), a focused text field takes the key (Esc
// cancels, Enter submits, the rest types), an open pick menu takes its own
// keys (Up and Down walk it, Enter sets the value, Esc closes it), then the
// screen keys run — the movements in the focused pane, Tab and Shift+Tab
// between the panes, Enter opens or activates, Space activates a toggle or a
// choice mark, and Esc walks the ladder — and every other key goes to the
// core, so the playback chords keep working behind the screen.

import { domKey } from '$lib/bridge/input';
import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiSettingsFrame } from '$lib/bridge/types';
import { CURSOR_MOVES } from '$lib/library/keys-shared';
import {
	activateSettingsEntry,
	cancelKeysCapture,
	cancelSettingsForm,
	currentSettingsEntries,
	focusedSettingsEntry,
	moveSettingsCursor,
	selectSettingsSection,
	sendKeysCapture,
	settingsEscape,
	settingsUi,
	spaceActivates,
	submitSettingsForm,
	switchSettingsPane,
} from './state';
import {
	closePickMenu,
	movePickMenuCursor,
	pickMenu,
	selectPickValue,
} from './pick-menu';
import { get } from 'svelte/store';

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/**
 * Route one keydown while the Settings screen shows (design D11): the
 * window's overlay and prefix step ran first, so a key that reaches this
 * point is the screen's or the core router's.
 */
export function routeSettingsKey(event: KeyboardEvent, settings: GuiSettingsFrame): KeyOutcome {
	const ui = get(settingsUi);
	// (2) An active capture takes every key: Esc cancels, and any other key
	// is the candidate chord `keysCapture` carries. A sent capture swallows
	// the keys until the frame's answer ends it.
	if (ui.capture !== null) {
		if (!ui.capture.sent) {
			if (event.key === 'Escape' && bare(event)) {
				cancelKeysCapture();
			} else {
				sendKeysCapture(ui.capture.target, domKey(event));
			}
		}
		return 'handled';
	}
	// (3) A focused text field takes the key: Esc cancels the form, Enter
	// submits it, and every other key types into the field — the `text`
	// answer lets the browser's own default run and sends nothing to the
	// core. Tab's default walks the form's fields and buttons.
	if (ui.textField !== null) {
		if (event.key === 'Escape' && bare(event)) {
			cancelSettingsForm();
			return 'handled';
		}
		if (event.key === 'Enter' && bare(event)) {
			submitSettingsForm(settings.content);
			return 'handled';
		}
		return 'text';
	}
	// (4) An open pick menu takes its own keys (design D13): Up and Down walk
	// its values, Enter sends `setValue` for the cursor value, and Esc closes
	// it with no change. Any other key closes it and falls through, so the
	// screen's own keys keep acting and the menu never outlives them.
	const menu = get(pickMenu);
	if (menu !== null) {
		if (event.key === 'Escape' && bare(event)) {
			closePickMenu();
			return 'handled';
		}
		if (event.key === 'Enter' && bare(event)) {
			selectPickValue(menu.cursor);
			return 'handled';
		}
		const pickMove = CURSOR_MOVES[event.key];
		if ((pickMove === 'up' || pickMove === 'down') && bare(event)) {
			movePickMenuCursor(pickMove === 'up' ? -1 : 1);
			return 'handled';
		}
		closePickMenu();
	}
	// (5) The screen keys run.
	if (event.key === 'Escape' && bare(event)) {
		settingsEscape(settings);
		return 'handled';
	}
	if (event.key === 'Tab' && !event.ctrlKey && !event.metaKey && !event.altKey) {
		switchSettingsPane();
		return 'handled';
	}
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined && bare(event)) {
		const count =
			ui.pane === 'sidebar'
				? settings.sections.length
				: currentSettingsEntries(settings.content, ui).length;
		moveSettingsCursor(move, count);
		return 'handled';
	}
	if (event.key === 'Enter' && bare(event)) {
		if (ui.pane === 'sidebar') {
			const section = settings.sections[Math.min(ui.sidebarCursor, settings.sections.length - 1)];
			if (section !== undefined) {
				selectSettingsSection(section);
			}
			return 'handled';
		}
		const entry = focusedSettingsEntry(settings.content, ui);
		if (entry !== null) {
			activateSettingsEntry(entry);
		}
		return 'handled';
	}
	if (event.key === ' ' && bare(event)) {
		// Space activates a focused toggle or a choice mark; on the sidebar
		// and on every other entry it is the core's play and pause (the
		// spec's *Global keys keep working behind the screen*).
		if (ui.pane === 'content') {
			const entry = focusedSettingsEntry(settings.content, ui);
			if (spaceActivates(entry)) {
				if (entry !== null) {
					activateSettingsEntry(entry);
				}
				return 'handled';
			}
		}
		return 'core';
	}
	// (6) Every other key goes to the core's keyboard router.
	return 'core';
}
