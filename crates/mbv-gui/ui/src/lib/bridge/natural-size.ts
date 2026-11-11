// Artwork never paints above its own pixels. The core hands the window the
// server's bytes unchanged, and the art boxes are sized by the layout, so a
// box larger than the image would stretch it. This action caps the image
// element at its natural size in device pixels (natural pixels divided by the
// display's pixel ratio): a smaller image paints 1:1, a larger one still
// scales down to its box.

import type { Action } from 'svelte/action';

/** An optional box in CSS pixels the image also stays inside. */
export interface NaturalSizeBounds {
	width?: number;
	height?: number;
}

/** Cap `img`'s rendered size at its natural size, and keep the cap current
 * as the image source or the display's pixel ratio changes. With `bounds`,
 * the image also stays inside that box, so its parent can hug the image
 * instead of holding a fixed box around it. */
export const capToNaturalSize: Action<HTMLImageElement, NaturalSizeBounds | undefined> = (
	img,
	bounds
) => {
	const cap = () => {
		if (img.naturalWidth === 0 || img.naturalHeight === 0) {
			return;
		}
		const ratio = window.devicePixelRatio || 1;
		const width = img.naturalWidth / ratio;
		const height = img.naturalHeight / ratio;
		img.style.maxWidth = `${Math.min(width, bounds?.width ?? width)}px`;
		img.style.maxHeight = `${Math.min(height, bounds?.height ?? height)}px`;
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
