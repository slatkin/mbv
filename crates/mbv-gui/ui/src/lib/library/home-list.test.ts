// add-gui-library-panel row 6.2: the Home list's row model (design D12, the
// USER-APPROVED GUI Home design) — the sections' heading rows and item rows
// at fixed heights over the spacer, the opaque row identity as a key, and the
// cursor's movement over the item rows only. The model is plain data, so the
// geometry is asserted without a layout.

import { expect, test } from 'vitest';
import type { GuiHomeRow } from '$lib/bridge/types';
import { buildHomeRows, homeRowKey, HOME_ITEM_ROW_HEIGHT, moveHomeCursor, HOME_HEADING_ROW_HEIGHT, HOME_SPACER_ROW_HEIGHT } from './home-list';
import { listWindow } from './list';

/** The `Continue watching` row the fixture carries: an Emby item with its
 * resume percent. */
const CONTINUED: GuiHomeRow = {
	row: 'item',
	id: { kind: 'continue', id: 'e1' },
	title: 'Better Call Saul',
	subtitle: 'S3E4 · Sabrosito',
	date: null,
	kind: 'media',
	state: { active: { progress: 42 } },
	art: null,
};

/** The second `Continue watching` row: a played item with its art. */
const WATCHED: GuiHomeRow = {
	row: 'item',
	id: { kind: 'continue', id: 'e2' },
	title: 'The Wire',
	subtitle: null,
	date: null,
	kind: 'media',
	state: 'played',
	art: { kind: 'image', src: 'mbv-art://localhost/card/x' },
};

/** One `Recently added` row: the Feed content id the key encodes. */
const ADDED: GuiHomeRow = {
	row: 'item',
	id: { kind: 'recent', id: { provider: 'Feed', value: 'f1' } },
	title: 'A Feed Entry',
	subtitle: 'The Feed',
	date: '12 May',
	kind: 'media',
	state: 'ordinary',
	art: null,
};

/** The carried rows the fixture paints: the two sections in order, with the
 * spacer between them, and the Audiobookshelf struct id on `Recently added`'s
 * last item. */
function rows(): GuiHomeRow[] {
	return [
		{ row: 'heading', text: 'Continue watching' },
		CONTINUED,
		WATCHED,
		{ row: 'spacer' },
		{ row: 'heading', text: 'Recently added' },
		ADDED,
	];
}

test('the sections paint theirs heading rows, items, and the spacer in order', () => {
	const painted = buildHomeRows(rows());

	expect(painted.map((row) => row.kind)).toEqual([
		'heading',
		'item',
		'item',
		'spacer',
		'heading',
		'item',
	]);
	expect(painted.map((row) => row.height)).toEqual([
		HOME_HEADING_ROW_HEIGHT,
		HOME_ITEM_ROW_HEIGHT,
		HOME_ITEM_ROW_HEIGHT,
		HOME_SPACER_ROW_HEIGHT,
		HOME_HEADING_ROW_HEIGHT,
		HOME_ITEM_ROW_HEIGHT,
	]);
	expect(painted.map((row) => row.top)).toEqual([0, 28, 76, 124, 144, 172]);
	expect((painted[0] as { text: string }).text).toBe('Continue watching');
	expect((painted[4] as { text: string }).text).toBe('Recently added');
});

// The stale-frame contract (design D12): a command must resolve the row the
// frame painted or nothing, so the key is a pure function of the opaque
// identity alone — equal identities key equal, distinct identities key
// distinct.
test('the row key is the opaque identity, distinct per section', () => {
	const continueKey = homeRowKey({ kind: 'continue', id: 'e1' });
	const recentKey = homeRowKey({ kind: 'recent', id: { provider: 'Feed', value: 'f1' } });
	const episodeKey = homeRowKey({
		kind: 'recent',
		id: { provider: 'Audiobookshelf', value: { library_item_id: 'li1', episode_id: 'ep1' } },
	});

	expect(continueKey).not.toBe(recentKey);
	expect(episodeKey).not.toBe(recentKey);
	expect(homeRowKey({ kind: 'recent', id: { provider: 'Feed', value: 'f1' } })).toBe(recentKey);
});

test('movement steps over the headings and the spacer', () => {
	const painted = buildHomeRows(rows());

	expect(moveHomeCursor(painted, homeRowKey(ADDED.id), 'up')).toBe(homeRowKey(WATCHED.id));
	expect(moveHomeCursor(painted, homeRowKey(ADDED.id), 'end')).toBe(homeRowKey(ADDED.id));
});

test('a first key movement lands the cursor on an item row', () => {
	const painted = buildHomeRows(rows());

	expect(moveHomeCursor(painted, null, 'down')).toBe(homeRowKey(CONTINUED.id));
	expect(moveHomeCursor(painted, null, 'up')).toBe(homeRowKey(ADDED.id));
});

test('the window paints only the rows in view', () => {
	const painted = buildHomeRows(rows());

	expect(listWindow(painted, 0, 80)).toEqual({ start: 0, end: 3 });
});
