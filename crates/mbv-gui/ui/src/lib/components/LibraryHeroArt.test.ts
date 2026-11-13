// gui-change-driven-sections row 5.4: the Hero's art box paints every `GuiArtRef`
// as an `<img>` whose `src` is its URL. A `loading` reference paints no image,
// and an `img` the webview fails to load shows the same loading fill, which
// retries the same URL. The fill is the box's placeholder text, so the test
// reads what the window paints.

import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, expect, test, vi } from 'vitest';
import type { GuiArtRef } from '$lib/bridge/art';
import LibraryHeroArt from './LibraryHeroArt.svelte';

const FILL = "[No art · the window's placeholder]";
const ART_URL = 'mbv-art://localhost/card/k';
/** `ArtImage`'s first retry wait: the first failure of one URL. */
const FIRST_RETRY_MS = 1000;

afterEach(() => {
	vi.useRealTimers();
});

function renderArt(art: GuiArtRef | null) {
	return render(LibraryHeroArt, {
		props: { shape: 'square', art, title: 'Album', attr: '' },
	});
}

function image(container: HTMLElement): HTMLImageElement {
	const img = container.querySelector('img');
	if (!img) {
		throw new Error('the art box paints no img');
	}
	return img;
}

test('a loading reference paints no img and the loading fill', () => {
	const { container, getByText } = renderArt({ kind: 'loading' });

	expect(container.querySelector('img')).toBeNull();
	expect(getByText(FILL)).toBeTruthy();
});

test('an img error shows the loading fill in place of the image', async () => {
	const { container, getByText } = renderArt({ kind: 'image', src: ART_URL });

	expect(image(container).getAttribute('src')).toBe(ART_URL);
	await fireEvent.error(image(container));

	expect(container.querySelector('img')).toBeNull();
	expect(getByText(FILL)).toBeTruthy();
});

test('after an img error, the retry repaints the img with the same src', async () => {
	vi.useFakeTimers();
	const { container } = renderArt({ kind: 'image', src: ART_URL });

	await fireEvent.error(image(container));
	expect(container.querySelector('img')).toBeNull();

	vi.advanceTimersByTime(FIRST_RETRY_MS);
	await tick();
	expect(image(container).getAttribute('src')).toBe(ART_URL);
});
