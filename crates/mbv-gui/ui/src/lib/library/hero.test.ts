// The Hero header and block mapping (add-gui-library-panel design D15,
// invariant 30): the largest unit is the header's big title, plain facts ride
// its one line, and only the duration and progress rows sit beside the art.
// Every line is typed segments, because colour carries the data type
// (add-gui-settings-screen design D14).

import { expect, test } from 'vitest';
import { heroLayout, trackTotalFacts } from './hero';
import type { Segments } from './hero';

/** One line's text, without its types. */
function text(segments: Segments): string {
	return segments.map((segment) => segment.text).join('');
}

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

	expect(text(layout.header.kicker)).toBe('podcast');
	expect(layout.header.kicker).toEqual([{ text: 'podcast', kind: 'label' }]);
	expect(layout.header.title).toBe('Show B');
	expect(layout.header.facts).toEqual([{ text: '08 Oct', kind: 'date' }]);
	expect(layout.block.name).toBe('B New');
	expect(layout.block.meta).toEqual([
		{ segments: [{ text: '0 h 54 m', kind: 'time' }], role: 'duration' },
	]);
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

	expect(text(layout.header.kicker)).toBe('audiobook');
	expect(layout.header.title).toBe('The Caves of Steel');
	expect(text(layout.header.facts)).toBe('Isaac Asimov');
	expect(layout.block.name).toBeUndefined();
	expect(layout.block.meta.map((row) => row.role)).toEqual(['duration', 'progress']);
});

test('an item with no plain rows leaves the header facts empty', () => {
	const layout = heroLayout(
		{ title: 'a-new', parent: 'Feed A', metaRows: [], durationRow: null, progressRow: null },
		'feed',
	);

	expect(layout.header.facts).toEqual([]);
});

// The Music facts line joins a count and a time (design D14): the count paints
// the count token and the total paints the time token.
test('the Music facts line types its count as Count and its total as Time', () => {
	const facts = trackTotalFacts({ count: 13, duration: '44:25' });

	expect(facts).toEqual([
		{ text: '13', kind: 'count' },
		{ text: ' tracks · ', kind: 'plain' },
		{ text: '44:25', kind: 'time' },
	]);
	expect(text(facts)).toBe('13 tracks · 44:25');
});
