// gui-change-driven-sections row 5.4: the Hero's art box paints every `GuiArtRef`
// as an `<img>` whose `src` is its URL. A `loading` reference paints no image,
// and an `img` the webview fails to load shows the same loading fill. The
// fill is the box's placeholder text, so the test reads what the window paints.

import { fireEvent, render } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import type { GuiArtRef } from '$lib/bridge/art';
import LibraryHeroArt from './LibraryHeroArt.svelte';

const FILL = "[No art · the window's placeholder]";

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
	const { container, getByText } = renderArt({
		kind: 'image',
		src: 'mbv-art://localhost/card/k',
	});

	expect(image(container).getAttribute('src')).toBe('mbv-art://localhost/card/k');
	await fireEvent.error(image(container));

	expect(container.querySelector('img')).toBeNull();
	expect(getByText(FILL)).toBeTruthy();
});
