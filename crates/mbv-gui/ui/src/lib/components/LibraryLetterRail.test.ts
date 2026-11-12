// add-gui-library-panel row 4.6: the letter rail (design D7) — A to Z beside the
// name-sorted list, a letter with no painted rows disabled so a press changes
// nothing, and a press on an enabled letter scrolling to its first row and
// moving the cursor there. The rail reads the painted rows the list hands it and
// jumps through the window content's state; the A–Z model itself is asserted in
// `$lib/library/list.test.ts`.

import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test } from 'vitest';
import type { GuiMusicArtist, GuiMusicList } from '$lib/bridge/types';
import { buildLibraryRows, letterRail } from '$lib/library/list';
import { carryLibrary } from '$lib/bridge/test-library';
import {
	initialLibraryUi,
	libraryUi,
	measureLibraryList,
} from '$lib/library/state';
import LibraryLetterRail from './LibraryLetterRail.svelte';

beforeEach(() => {
	libraryUi.set(initialLibraryUi());
	carryLibrary(null);
});

/** The Beastie Boys artist: one album whose tracks the content does not
 * hold. */
const BEASTIE: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a1' },
	name: 'Beastie Boys',
	sortLetter: 'B',
	albums: [{ albumId: 'al1', name: 'Licensed To Ill', year: '1986', tracks: null, total: null }],
};

/** The Brand Nubian artist: one album, closed under the artist. */
const BRAND: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a2' },
	name: 'Brand Nubian',
	sortLetter: 'B',
	albums: [{ albumId: 'al2', name: 'One For All', year: '1990', tracks: null, total: null }],
};

/** The carried Music content: both artists, none of them open. */
function music(): GuiMusicList {
	return {
		libIdx: 0,
		pills: [],
		collectionNames: [],
		loading: false,
		failure: null,
		artists: [BEASTIE, BRAND],
	};
}

/** The rail's letters: the A–Z entries over the painted rows. */
function letters() {
	return letterRail(
		buildLibraryRows({ artists: music().artists, open: [], closed: [], filter: '' }),
	);
}

test('a letter with no painted rows is disabled and a press changes nothing', async () => {
	const { container } = render(LibraryLetterRail, { letters: letters(), marked: null });
	const empty = container.querySelector('[data-library-rail-letter="C"]');

	expect(empty?.hasAttribute('disabled')).toBe(true);
	await fireEvent.click(empty as HTMLElement);
	expect(get(libraryUi).cursor).toBeNull();
});

test('a press on a letter moves the cursor to its first row and shows that row', async () => {
	carryLibrary({
		list: { tabs: [], currentTab: null, destination: { kind: 'music', ...music() } },
		detail: { kind: 'music', hero: null, art: null, playingTrackId: null },
	});
	measureLibraryList(40);
	const { container } = render(LibraryLetterRail, { letters: letters(), marked: null });

	await fireEvent.click(container.querySelector('[data-library-rail-letter="B"]') as HTMLElement);

	expect(get(libraryUi).cursor).toBe('artist:service:a1');
	expect(get(libraryUi).scrollTop).toBe(14);
});
