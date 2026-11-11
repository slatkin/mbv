// The Hero header and block mapping (add-gui-library-panel design D15,
// invariant 30): the largest unit is the header's big title, plain facts ride
// its one line, and only the duration and progress rows sit beside the art.

import { expect, test } from 'vitest';
import { heroLayout } from './hero';

test('an item with a parent puts the parent in the header and the item in the block', () => {
	const layout = heroLayout(
		{
			title: 'B New',
			parent: 'Show B',
			metaRows: [[{ text: '0 h 54 m', kind: 'time' }], [{ text: '08 Oct', kind: 'date' }]],
			durationRow: 0,
			progressRow: null,
		},
		'podcast',
	);

	expect(layout.header).toEqual({ kicker: 'podcast', title: 'Show B', facts: '08 Oct' });
	expect(layout.block.name).toBe('B New');
	expect(layout.block.meta).toEqual([{ text: '0 h 54 m', role: 'duration' }]);
});

test('an item that is its own largest unit titles the header and leaves the block unnamed', () => {
	const layout = heroLayout(
		{
			title: 'The Caves of Steel',
			metaRows: [[{ text: 'Isaac Asimov', kind: 'plain' }], [{ text: '9 h 51 m', kind: 'time' }], [{ text: '17%', kind: 'plain' }]],
			durationRow: 1,
			progressRow: 2,
		},
		'audiobook',
	);

	expect(layout.header).toEqual({
		kicker: 'audiobook',
		title: 'The Caves of Steel',
		facts: 'Isaac Asimov',
	});
	expect(layout.block.name).toBeUndefined();
	expect(layout.block.meta.map((row) => row.role)).toEqual(['duration', 'progress']);
});

test('an item with no plain rows leaves the header facts empty', () => {
	const layout = heroLayout(
		{ title: 'a-new', parent: 'Feed A', metaRows: [], durationRow: null, progressRow: null },
		'feed',
	);

	expect(layout.header.facts).toBe('');
});
