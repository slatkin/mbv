// The typed text painter (add-gui-settings-screen design D14): colour carries
// the data type, so each segment names one `--canvas-type-*` token, and a time
// or a date repeated down a list paints at the repeat opacity. The test
// asserts the token each kind maps to, never a rendered colour: jsdom holds no
// palette.

import { render } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import type { GuiMetaKind } from '$lib/bridge/library-types';
import TypedText from './TypedText.svelte';

const TOKENS: [GuiMetaKind, string][] = [
	['plain', 'inherit'],
	['time', 'var(--canvas-type-time)'],
	['date', 'var(--canvas-type-date)'],
	['count', 'var(--canvas-type-count)'],
	['label', 'var(--canvas-type-label)'],
	['secondary', 'var(--canvas-type-secondary)'],
	['role', 'var(--canvas-type-role)'],
];

test.each(TOKENS)('a %s segment paints its type token', (kind, token) => {
	const { container } = render(TypedText, { segments: [{ text: 'x', kind }] });

	expect((container.querySelector('[data-meta-kind]') as HTMLElement).style.color).toBe(token);
});

test.each([
	['time', 'var(--canvas-type-repeat-opacity)'],
	['date', 'var(--canvas-type-repeat-opacity)'],
	['count', ''],
	['plain', ''],
])('a repeated %s segment paints the repeat opacity: %s', (kind, opacity) => {
	const { container } = render(TypedText, {
		segments: [{ text: 'x', kind: kind as GuiMetaKind }],
		repeat: true,
	});

	expect((container.querySelector('[data-meta-kind]') as HTMLElement).style.opacity).toBe(opacity);
});
