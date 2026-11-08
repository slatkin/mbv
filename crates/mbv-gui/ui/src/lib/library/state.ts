// The Library panel's own state (add-gui-library-panel design D1): what the
// window content owns and the core never reads. Two stores keep the two kinds
// apart — the content the frame carries, kept from the last frame that carried
// it, and the interaction state the panel's keys and controls move.
//
// The core owns the data selection (which collections are selected, the picked
// item), so nothing here mirrors it: the pill toggle the keys request is an
// intent the pill row turns into the set-collections command (design D6).

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type {
	GuiLibraryContent,
	GuiLibraryFrame,
	GuiMusicPill,
} from '$lib/bridge/types';
import { nextPillSelection } from './pills';
import type { PillGesture } from './pills';

/** The panes Tab moves focus across (design D3, step 3). The Queue panel is
 * the pane past the Hero, but it takes Panel focus through the core's focus
 * stack, which no key command reaches (design D4). */
export type LibraryPane = 'pills' | 'list' | 'hero';

/** The order Tab and Shift+Tab walk the panes in (design D3, step 3). */
const PANES: readonly LibraryPane[] = ['pills', 'list', 'hero'];

/** The Library panel's interaction state. */
export interface LibraryUiState {
	/** The pane the window content focuses. */
	pane: LibraryPane;
	/** Whether a text field of the panel holds the keyboard (design D3,
	 * step 2). */
	filterOpen: boolean;
	/** The filter's text: the `/` field types it, and it narrows the list
	 * without case (design D7). */
	filter: string;
	/** The pill Left and Right move the focus to, or `null` while no pill has
	 * it (design D6). */
	focusedPill: string | null;
	/** The pill toggle the keys asked for: Space toggles and Alt+Space solos
	 * the focused pill (design D6). The pill row turns it into the
	 * set-collections command. */
	pillToggle: { groupId: string; solo: boolean } | null;
}

/** The state a freshly mounted window content starts with. */
export function initialLibraryUi(): LibraryUiState {
	return {
		pane: 'pills',
		filterOpen: false,
		filter: '',
		focusedPill: null,
		pillToggle: null,
	};
}

/** The Library panel's interaction state. */
export const libraryUi = writable<LibraryUiState>(initialLibraryUi());

/** The last Library content the core carried (design D2). A frame re-carries
 * it only when its revision changes, so the window keeps what the last
 * carrying frame held. */
export const libraryContent = writable<GuiLibraryContent | null>(null);

/** Keep the content of a frame that carries it (design D2). */
export function carryLibraryContent(frame: GuiLibraryFrame | null): void {
	if (frame?.content) {
		libraryContent.set(frame.content);
	}
}

/** Open the list's filter: `/` with Library Panel focus (design D3, step 4). */
export function openLibraryFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: true }));
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). */
export function closeLibraryFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false, filter: '' }));
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptLibraryFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
}

/** Focus a pane: the window content of Tab and Shift+Tab (design D3, step 3). */
export function focusLibraryPane(pane: LibraryPane): void {
	libraryUi.update((ui) => ({ ...ui, pane }));
}

/** Move the pane focus `step` panes along, wrapping at the ends. */
export function moveLibraryPane(step: number): void {
	libraryUi.update((ui) => {
		const at = PANES.indexOf(ui.pane);
		const next = (at + step + PANES.length) % PANES.length;
		return { ...ui, pane: PANES[next] };
	});
}

/** Focus a pill, or no pill while `groupId` is `null`. */
export function focusLibraryPill(groupId: string | null): void {
	libraryUi.update((ui) => ({ ...ui, focusedPill: groupId }));
}

/** Ask for the focused pill to toggle, or to solo with Alt+Space (design D6). */
export function requestLibraryPillToggle(groupId: string, solo: boolean): void {
	libraryUi.update((ui) => ({ ...ui, pillToggle: { groupId, solo } }));
}

/** Send the set a pill gesture leaves selected (design D6): the core stores it
 * per destination and loads the new groups. The set reads the core's carried
 * selection, so the pills and the command cannot disagree. */
export function selectLibraryPills(
	libIdx: number,
	pills: readonly GuiMusicPill[],
	gesture: PillGesture,
): void {
	void sendLibraryCommand({
		command: 'setCollections',
		libIdx,
		groupIds: nextPillSelection(pills, gesture),
	});
}

/** Turn the keys' pill toggle into the set-collections command a press on the
 * pill sends (design D6): Space toggles the focused pill and Alt+Space solos
 * it. The intent is consumed here, so one key sends one command. */
export function applyLibraryPillToggle(
	libIdx: number,
	pills: readonly GuiMusicPill[],
): void {
	const { pillToggle } = get(libraryUi);
	if (pillToggle === null) {
		return;
	}
	libraryUi.update((ui) => ({ ...ui, pillToggle: null }));
	selectLibraryPills(libIdx, pills, {
		kind: 'press',
		groupId: pillToggle.groupId,
		solo: pillToggle.solo,
	});
}

/** The pill the pill row's focus holds, resolved against the carried content
 * so a stale id focuses nothing. */
export function focusedLibraryPill(
	pills: readonly GuiMusicPill[],
	ui: LibraryUiState,
): GuiMusicPill | null {
	if (ui.pane !== 'pills' || ui.focusedPill === null) {
		return null;
	}
	return pills.find((pill) => pill.groupId === ui.focusedPill) ?? null;
}
