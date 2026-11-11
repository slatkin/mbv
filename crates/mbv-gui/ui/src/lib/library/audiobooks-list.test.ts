// add-gui-library-panel row 9.1: the Audiobooks list's row model (design D12)
// — the letter headings over each run of the catalog's author sort keys, the
// cursor's movement over the book rows only, and the A–Z rail over the author
// sort key with the letters no row carries disabled. The model is plain data,
// so the geometry is asserted without a layout.

import { expect, test } from 'vitest';
import type { GuiAudiobooksRow } from '$lib/bridge/types';
import {
	AUIB_HEADING_ROW_HEIGHT,
	AUIB_ROW_HEIGHT,
	audiobooksLetterRail,
	buildAudiobooksRows,
	moveAudiobooksCursor,
} from './audiobooks-list';

/** One book row of the catalog: its rail letter is the heading it paints
 * under. */
function book(id: string, title: string, sortLetter: string): GuiAudiobooksRow {
	return { id, title, subtitle: null, duration: '', sortLetter };
}

/** The catalog fixture: two books under `A`, one under `B`, and one whose
 * author sort key leads with a digit, which paints under `#`. */
const CATALOG: GuiAudiobooksRow[] = [
	book('b1', 'Aftermath', 'A'),
	book('b2', 'Audition', 'A'),
	book('b3', 'Blindsight', 'B'),
	book('b4', '1984', '#'),
];

test('a letter heading paints over each run of author sort keys', () => {
	const painted = buildAudiobooksRows(CATALOG);

	expect(painted.map((row) => row.kind)).toEqual([
		'heading',
		'item',
		'item',
		'heading',
		'item',
		'heading',
		'item',
	]);
	expect(painted.map((row) => row.height)).toEqual([
		AUIB_HEADING_ROW_HEIGHT,
		AUIB_ROW_HEIGHT,
		AUIB_ROW_HEIGHT,
		AUIB_HEADING_ROW_HEIGHT,
		AUIB_ROW_HEIGHT,
		AUIB_HEADING_ROW_HEIGHT,
		AUIB_ROW_HEIGHT,
	]);
	expect(painted[0].top).toBe(0);
	expect(painted[1].top).toBe(AUIB_HEADING_ROW_HEIGHT);
	expect(painted.map((row) => (row.kind === 'heading' ? row.letter : null))).toEqual([
		'A',
		null,
		null,
		'B',
		null,
		'#',
		null,
	]);
});

test('an empty catalog paints no row', () => {
	expect(buildAudiobooksRows([])).toEqual([]);
});

test('movement steps over the letter headings', () => {
	const painted = buildAudiobooksRows(CATALOG);

	expect(moveAudiobooksCursor(painted, 'b2', 'down')).toBe('b3');
	expect(moveAudiobooksCursor(painted, 'b3', 'down')).toBe('b4');
	expect(moveAudiobooksCursor(painted, null, 'down')).toBe('b1');
	expect(moveAudiobooksCursor(painted, 'b1', 'end')).toBe('b4');
});

test('the rail holds the first book under each letter and disables the rest', () => {
	const rail = audiobooksLetterRail(buildAudiobooksRows(CATALOG));

	expect(rail.find((letter) => letter.letter === 'A')?.rowId).toBe('b1');
	expect(rail.find((letter) => letter.letter === 'B')?.rowId).toBe('b3');
	expect(rail.find((letter) => letter.letter === 'C')?.rowId).toBeNull();
});
