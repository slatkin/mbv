// The pill row's multi-select rules (add-gui-library-panel design D6). The
// front end computes the new selected set from the gesture and the core's
// carried set, never from a local guess: the pills paint what the frame says,
// and every gesture reads that same set. The rules live here, apart from the
// stores and the command, so the arrangement of a gesture is plain data.

import type { GuiMusicPill } from '$lib/bridge/types';

/** One gesture the pill row turns into a selected set (design D6): a press on
 * a pill — Alt-pressed to solo it — or a press on the clear control. */
export type PillGesture =
	| { kind: 'press'; groupId: string; solo: boolean }
	| { kind: 'clear' };

/** The selected group ids, in the pills' order. */
export function selectedPillIds(pills: readonly GuiMusicPill[]): string[] {
	return pills.filter((pill) => pill.selected).map((pill) => pill.groupId);
}

/**
 * The set a gesture leaves selected, as group ids in pill order (design D6). A
 * press toggles a collection pill on or off, an Alt-press solos it, and a
 * single-select pill replaces the set. At least one pill stays selected, so
 * turning off the last one changes nothing; clear keeps the first selected
 * pill in pill order. A stale group id selects nothing.
 */
export function nextPillSelection(
	pills: readonly GuiMusicPill[],
	gesture: PillGesture,
): string[] {
	const selected = selectedPillIds(pills);
	if (gesture.kind === 'clear') {
		return selected.slice(0, 1);
	}
	const pill = pills.find((candidate) => candidate.groupId === gesture.groupId);
	if (pill === undefined) {
		return selected;
	}
	if (gesture.solo || !pill.collection) {
		return [pill.groupId];
	}
	const next = new Set(selected);
	if (next.has(pill.groupId)) {
		if (next.size === 1) {
			return selected;
		}
		next.delete(pill.groupId);
	} else {
		next.add(pill.groupId);
	}
	return pills
		.filter((candidate) => next.has(candidate.groupId))
		.map((candidate) => candidate.groupId);
}
