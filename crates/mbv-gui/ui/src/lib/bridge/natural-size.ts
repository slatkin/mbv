// Artwork never paints above its own pixels. The core hands the window the
// server's bytes unchanged, and the art boxes are sized by the layout, so a
// box larger than the image would stretch it. This action caps the image
// element at its natural size in device pixels (natural pixels divided by the
// display's pixel ratio): a smaller image paints 1:1, a larger one still
// scales down to its box.

import type { Action } from 'svelte/action';

/** Cap `img`'s rendered size at its natural size, and keep the cap current
 * as the image source or the display's pixel ratio changes. */
export const capToNaturalSize: Action<HTMLImageElement> = (img) => {
	const cap = () => {
		if (img.naturalWidth === 0 || img.naturalHeight === 0) {
			return;
		}
		const ratio = window.devicePixelRatio || 1;
		img.style.maxWidth = `${img.naturalWidth / ratio}px`;
		img.style.maxHeight = `${img.naturalHeight / ratio}px`;
	};
	img.addEventListener('load', cap);
	window.addEventListener('resize', cap);
	cap();
	return {
		destroy() {
			img.removeEventListener('load', cap);
			window.removeEventListener('resize', cap);
		}
	};
};
