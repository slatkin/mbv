// Artwork never paints above its own pixels (the GUI paints the server's
// bytes unchanged): the action caps the element at the natural size in device
// pixels.

import { afterEach, expect, test } from 'vitest';
import { capToNaturalSize } from './natural-size';

function image(naturalWidth: number, naturalHeight: number): HTMLImageElement {
	const img = document.createElement('img');
	Object.defineProperty(img, 'naturalWidth', { value: naturalWidth, configurable: true });
	Object.defineProperty(img, 'naturalHeight', { value: naturalHeight, configurable: true });
	return img;
}

afterEach(() => {
	Object.defineProperty(window, 'devicePixelRatio', { value: 1, configurable: true });
});

test('a loaded image is capped at its natural size in device pixels', () => {
	Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true });
	const img = image(400, 300);

	capToNaturalSize(img);

	expect(img.style.maxWidth).toBe('200px');
	expect(img.style.maxHeight).toBe('150px');
});

test('an image that has not loaded yet is capped when it loads', () => {
	const img = image(0, 0);
	capToNaturalSize(img);
	expect(img.style.maxWidth).toBe('');

	Object.defineProperty(img, 'naturalWidth', { value: 640, configurable: true });
	Object.defineProperty(img, 'naturalHeight', { value: 360, configurable: true });
	img.dispatchEvent(new Event('load'));

	expect(img.style.maxWidth).toBe('640px');
	expect(img.style.maxHeight).toBe('360px');
});
